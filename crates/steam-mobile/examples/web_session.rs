//! Logs on to an account over CM with a token, logs steam-mobile in to the Steam websites through that client
//! (`login_with_client`), and reads the account's mobile confirmations (read-only). The steps of the live test plan,
//! L1 to L9, are flags.
//!
//! ```sh
//! RUST_LOG=steam_client=debug,steam_mobile=info cargo run -p steam-mobile --features cm --example web_session -- \
//!     [--stop-after logon|token|web] [--refresh] [--renew-tokens] [--after-log-off]
//! ```
//!
//! Reads, from these paths or the environment variables named:
//! - `STEAM_MAFILE` (`~/.steam-test/account.maFile`): the maFile, for the account name, the Steam Guard codes and the
//!   confirmations.
//! - `STEAM_PASSWORD_FILE` (`~/.steam-test/password`): the password. steam-mobile's user takes it; the CM logon only
//!   uses it when there is no saved token.
//! - `STEAM_REFRESH_TOKEN_FILE` (`~/.steam-test/refresh_token`): a saved refresh token. Without one, logs on with the
//!   password, and saves the token Steam issues there (readable by the owner only).
//!
//! Prints no secret: tokens show as their times only (issued, expires, asked for).
//!
//! - `--stop-after logon`: L1, logs on (and spawns the client) and off.
//! - `--stop-after token`: and L2, generates a web access token.
//! - `--stop-after web`: and L3, logs steam-mobile in through the client, which generates its own token.
//! - no `--stop-after`: and L4, fetches the confirmations.
//! - `--refresh`: L7, sets the authenticator's renewal margin to 25 hours, past the token's whole lifetime, so that the
//!   token is due again 30 seconds after it was issued (the least time between two renewals); waits 40 seconds, then
//!   fetches them again: the authenticator renews the token through the client first, by itself, swapping it
//!   mid-session, and the fetch goes with the new one. Takes about a minute.
//! - `--renew-tokens`: L8, lets Steam replace the refresh token, and saves the new one.
//! - `--after-log-off`: L9, logs off, then fetches them again.

use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;

use steam_client::auth::AccessToken;
use steam_client::auth::Confirmation;
use steam_client::auth::RefreshToken;
use steam_client::client::LoggedOn;
use steam_client::client::SteamClient;
use steam_client::client::User;
use steam_mobile::user::PresentMaFile;
use steam_mobile::user::SteamUser;
use steam_mobile::Authenticated;
use steam_mobile::MobileAuthFile;
use steam_mobile::SteamAuthenticator;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `--refresh`'s renewal margin: past the token's lifetime, a day and a few minutes, so that it is due as soon as
/// steam-mobile renews at all, 30 seconds after the last renewal.
const RENEW_BEFORE: Duration = Duration::from_secs(25 * 3600);

/// How long `--refresh` waits before fetching again: past those 30 seconds.
const RENEWAL_WAIT: Duration = Duration::from_secs(40);

#[derive(Debug, Default)]
struct Steps {
    stop_after: Option<String>,
    refresh: bool,
    renew_tokens: bool,
    after_log_off: bool,
}

impl Steps {
    fn parse() -> Result<Self> {
        let mut steps = Self::default();
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--stop-after" => match args.next().as_deref() {
                    Some(step @ ("logon" | "token" | "web")) => steps.stop_after = Some(step.to_string()),
                    _ => return Err("--stop-after takes logon, token or web".into()),
                },
                "--refresh" => steps.refresh = true,
                "--renew-tokens" => steps.renew_tokens = true,
                "--after-log-off" => steps.after_log_off = true,
                other => return Err(format!("unknown argument {}", other).into()),
            }
        }
        Ok(steps)
    }

    fn stops_after(&self, step: &str) -> bool {
        self.stop_after.as_deref() == Some(step)
    }
}

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
        Ok(left) => format!("in {}h{:02}m", left.as_secs() / 3600, left.as_secs() / 60 % 60),
        Err(_) => "already".to_string(),
    }
}

/// `time` in UTC, to the second.
fn utc(time: SystemTime) -> String {
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    // days since 1970-01-01 to a date: Howard Hinnant's `civil_from_days`
    let days = (secs / 86400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month + 2) / 5 + 1;
    let month = if month < 10 { month + 3 } else { month - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year,
        month,
        day,
        secs / 3600 % 24,
        secs / 60 % 60,
        secs % 60
    )
}

/// Seconds from `earlier` to `later`, negative if `later` comes first.
fn seconds(earlier: SystemTime, later: SystemTime) -> i64 {
    match later.duration_since(earlier) {
        Ok(after) => after.as_secs() as i64,
        Err(before) => -(before.duration().as_secs() as i64),
    }
}

/// When Steam issued and expires `token` (its clock), and how that compares with when this machine asked for it: a
/// fresh token is issued as it is asked for, a cached one before.
fn print_times(what: &str, token: &AccessToken, asked: SystemTime) {
    let expires = token.expires_at();
    match token.issued_at() {
        Some(issued) => println!(
            "{}: asked {}, issued {} ({}s after the ask), expires {}, lasts {}s",
            what,
            utc(asked),
            utc(issued),
            seconds(asked, issued),
            utc(expires),
            seconds(issued, expires)
        ),
        None => println!("{}: asked {}, no iat, expires {}", what, utc(asked), utc(expires)),
    }
}

/// Saves the refresh token, readable by the owner only.
fn save(path: &Path, token: &RefreshToken) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(token.expose_secret().as_bytes())?;
    println!(
        "Saved the refresh token to {} (expires {})",
        path.display(),
        until(token.expires_at())
    );
    Ok(())
}

/// Logs on with the saved refresh token, or authenticates with the password and the maFile's codes.
async fn log_on(
    account: &str,
    password: &str,
    mafile: &MobileAuthFile,
    token_path: &Path,
) -> Result<SteamClient<LoggedOn<User>>> {
    let mut client = SteamClient::connect().await?;
    println!("Connected to {}", client.endpoint());

    if token_path.exists() {
        let token = RefreshToken::new(fs::read_to_string(token_path)?.trim().to_string())?;
        println!(
            "Logging on with the saved refresh token (expires {})",
            until(token.expires_at())
        );
        return Ok(client.login_with_refresh_token(account, token).await?);
    }

    let mut session = client.begin_auth_session(account, password).await?;
    println!("Steam Guard accepts: {:?}", session.confirmations());
    if session.confirmations().contains(&Confirmation::DeviceCode) {
        session.submit_device_code(&mafile.auth_code().await?).await?;
    } else if let Some(Confirmation::EmailCode { email_domain }) = session
        .confirmations()
        .iter()
        .find(|confirmation| matches!(confirmation, Confirmation::EmailCode { .. }))
    {
        print!("Code Steam emailed to your address at {}: ", email_domain);
        std::io::stdout().flush()?;
        let mut code = String::new();
        std::io::stdin().read_line(&mut code)?;
        session.submit_email_code(code.trim()).await?;
    }
    let (account, token) = session.poll().await?.into_parts();
    save(token_path, &token)?;

    Ok(client.login_with_refresh_token(&account, token).await?)
}

async fn print_confirmations(authenticator: &SteamAuthenticator<Authenticated, PresentMaFile>) -> Result<()> {
    let confirmations = authenticator.fetch_confirmations().await?;
    let kinds: Vec<_> = confirmations.0.iter().map(|confirmation| &confirmation.kind).collect();
    println!("{} confirmations pending: {:?}", kinds.len(), kinds);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let steps = Steps::parse()?;

    let mafile = MobileAuthFile::from_disk(path("STEAM_MAFILE", "account.maFile")?)?;
    let account = mafile.account_name.clone().ok_or("the maFile has no account_name")?;
    let password = fs::read_to_string(path("STEAM_PASSWORD_FILE", "password")?)?;
    let password = password.trim_end_matches(['\r', '\n']).to_string();
    let token_path = path("STEAM_REFRESH_TOKEN_FILE", "refresh_token")?;

    // L1
    let client = log_on(&account, &password, &mafile, &token_path).await?;
    println!(
        "Logged on as {} ({})",
        client.steam_id().to_steam64(),
        client.account_name()
    );
    // the authenticator keeps a handle, to renew its token through the client: a spawned one
    let (handle, _events, task) = client.spawn();
    if steps.stops_after("logon") {
        handle.log_off().await?;
        println!("Stopped: {:?}", task.await);
        return Ok(());
    }

    // L2
    let asked = SystemTime::now();
    let access_token = handle.web_access_token().await?;
    println!("Web access token expires {}", until(access_token.expires_at()));
    print_times("Web access token", &access_token, asked);
    if steps.stops_after("token") {
        handle.log_off().await?;
        println!("Stopped: {:?}", task.await);
        return Ok(());
    }

    // L3
    let web_login_asked = SystemTime::now();
    let user = SteamUser::new(account, password).with_mafile(mafile);
    let authenticator = SteamAuthenticator::new(user).login_with_client(handle.clone()).await?;
    let authenticator = if steps.refresh {
        authenticator.renew_before(RENEW_BEFORE)
    } else {
        authenticator
    };
    println!("Logged in to the Steam websites");
    if steps.stops_after("web") {
        handle.log_off().await?;
        println!("Stopped: {:?}", task.await);
        return Ok(());
    }

    // L4
    print_confirmations(&authenticator).await?;

    // L7
    if steps.refresh {
        // compared, never printed; getting it may renew it already, if L4 came 30 seconds after the login
        let before = authenticator.access_token().await?;
        // parsed as steam-client's token, for its claims
        print_times(
            "Session token before",
            &AccessToken::new(before.expose_secret().to_string())?,
            web_login_asked,
        );
        println!(
            "Waiting {}s for the session's token to be due again",
            RENEWAL_WAIT.as_secs()
        );
        tokio::time::sleep(RENEWAL_WAIT).await;
        let renewal_asked = SystemTime::now();
        print_confirmations(&authenticator).await?;
        // within 30 seconds of the renewal: not renewed again
        let after = authenticator.access_token().await?;
        print_times(
            "Session token after",
            &AccessToken::new(after.expose_secret().to_string())?,
            renewal_asked,
        );
        println!("Tokens differ: {}", after.expose_secret() != before.expose_secret());
        if after.expose_secret() == before.expose_secret() {
            return Err("the session's token wasn't renewed".into());
        }
        println!("Fetched them with a renewed token");
    }

    // L8
    if steps.renew_tokens {
        let renewed = handle.renew_tokens().await?;
        println!("New access token expires {}", until(renewed.access_token.expires_at()));
        match renewed.refresh_token {
            Some(token) => save(&token_path, &token)?,
            None => println!("Steam kept the refresh token"),
        }
    }

    handle.log_off().await?;
    println!("Logged off; stopped: {:?}", task.await);

    // L9: the session's token is still valid, so the request goes with it; with `--refresh` its renewal may be due,
    // and fails, as the client stopped (a warning in the log)
    if steps.after_log_off {
        print_confirmations(&authenticator).await?;
    }
    Ok(())
}
