//! Logs on to an account for the first time with its name, password and Steam Guard, and saves the refresh token
//! Steam issues; later runs log on with the saved token, without the password. The other account examples
//! (`chat_bot`, `trade_notifications`, `web_session_bot`) read the token this saves.
//!
//! ```sh
//! STEAM_ACCOUNT=<account name> RUST_LOG=steam_client=info cargo run -p steam-client-lib --example login
//! ```
//!
//! Reads:
//! - `STEAM_ACCOUNT`: the account name.
//! - `STEAM_PASSWORD_FILE` (`~/.steam-test/password`): the password, only when there is no saved token.
//! - `STEAM_MAFILE` (optional): a maFile, whose `shared_secret` answers Steam Guard with a code of the mobile
//!   authenticator. Without one, the code Steam emailed, or the one the Steam mobile app shows, is read from stdin.
//! - `STEAM_REFRESH_TOKEN_FILE` (`~/.steam-test/refresh_token`): the saved refresh token. Without one, the example
//!   authenticates with the password and saves the token Steam issues there (readable by the owner only). Delete it to
//!   authenticate again, such as once Steam revoked it.
//!
//! Prints the SteamID and the token's expiry, never the password, a code or a token. The first logon adds a
//! "SteamHelper-rs" device to the account's authorized devices.
//!
//! Not tried live yet: it is step L1 of the live test plan (`workingcontext.md`), without steam-mobile.

use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use steam_client::auth::AuthSession;
use steam_client::auth::Confirmation;
use steam_client::auth::RefreshToken;
use steam_client::client::SteamClient;
use steam_totp::Secret;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `variable`, or `~/.steam-test/<file>`.
fn path(variable: &str, file: &str) -> Result<PathBuf> {
    match std::env::var_os(variable) {
        Some(path) => Ok(path.into()),
        None => {
            let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
            Ok(Path::new(&home).join(".steam-test").join(file))
        }
    }
}

/// How long until `time`, for printing expiries.
fn until(time: SystemTime) -> String {
    match time.duration_since(SystemTime::now()) {
        Ok(left) => format!("in {}d{:02}h", left.as_secs() / 86400, left.as_secs() / 3600 % 24),
        Err(_) => "already".to_string(),
    }
}

/// Saves the refresh token, readable by the owner only.
fn save(path: &Path, token: &RefreshToken) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(token.expose_secret().as_bytes())?;
    Ok(())
}

/// A line from stdin, after `prompt`.
fn read_line(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

/// The maFile's shared secret. Only that field is read: the maFile is plain JSON.
fn shared_secret(mafile: &Path) -> Result<Secret> {
    let mafile: serde_json::Value = serde_json::from_str(&fs::read_to_string(mafile)?)?;
    let secret = mafile["shared_secret"]
        .as_str()
        .ok_or("the maFile has no shared_secret")?;
    Ok(Secret::from_b64(secret)?)
}

/// Answers Steam Guard: a device code from the maFile or stdin, or an email code from stdin. Anything else, such as
/// approving in the mobile app, needs nothing submitted: polling waits for it.
async fn confirm(session: &mut AuthSession<'_>) -> Result<()> {
    let confirmations = session.confirmations().to_vec();
    println!("Steam Guard accepts: {:?}", confirmations);

    if confirmations.contains(&Confirmation::DeviceCode) {
        match std::env::var_os("STEAM_MAFILE") {
            Some(mafile) => session.submit_shared_secret(shared_secret(mafile.as_ref())?).await?,
            None => {
                let code = read_line("Code the Steam mobile app shows: ")?;
                session.submit_device_code(&code).await?
            }
        }
    } else if let Some(Confirmation::EmailCode { email_domain }) = confirmations
        .iter()
        .find(|confirmation| matches!(confirmation, Confirmation::EmailCode { .. }))
    {
        let code = read_line(&format!("Code Steam emailed to your address at {}: ", email_domain))?;
        session.submit_email_code(&code).await?;
    } else {
        println!("Nothing to submit; approve the logon in the Steam mobile app or by email if Steam asks");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let account = std::env::var("STEAM_ACCOUNT").map_err(|_| "set STEAM_ACCOUNT to the account name")?;
    let token_path = path("STEAM_REFRESH_TOKEN_FILE", "refresh_token")?;

    let mut client = SteamClient::connect().await?;
    println!("Connected to {}", client.endpoint());

    let client = if token_path.exists() {
        let token = RefreshToken::new(fs::read_to_string(&token_path)?.trim().to_string())?;
        println!("Logging on with the saved refresh token");
        client.login_with_refresh_token(&account, token).await?
    } else {
        let password = fs::read_to_string(path("STEAM_PASSWORD_FILE", "password")?)?;
        let password = password.trim_end_matches(['\r', '\n']);

        let mut session = client.begin_auth_session(&account, password).await?;
        confirm(&mut session).await?;
        // Steam's spelling of the account name, which the token goes with
        let (account, token) = session.poll().await?.into_parts();
        save(&token_path, &token)?;
        println!("Saved the refresh token to {}", token_path.display());

        client.login_with_refresh_token(&account, token).await?
    };

    println!(
        "Logged on as {} ({}); the refresh token expires {}",
        client.steam_id().to_steam64(),
        client.account_name(),
        until(client.refresh_token().expires_at())
    );

    client.log_off().await?;
    println!("Logged off");
    Ok(())
}
