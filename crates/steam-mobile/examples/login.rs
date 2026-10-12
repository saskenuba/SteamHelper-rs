//! Logs into a Steam account using its maFile, and nothing else.
//!
//! The password is read from `STEAM_PASSWORD`, so it stays out of the shell history and the process list.
//!
//! ```sh
//! STEAM_PASSWORD=... cargo run -p steam-mobile --example login -- path/to/account.maFile
//! ```

use steam_mobile::user::SteamUser;
use steam_mobile::MobileAuthFile;
use steam_mobile::SteamAuthenticator;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "steam_mobile=info".into()),
        )
        .init();

    let path = std::env::args().nth(1).ok_or("usage: login <path to maFile>")?;
    let password = std::env::var("STEAM_PASSWORD").map_err(|_| "STEAM_PASSWORD is not set")?;

    let mafile = MobileAuthFile::from_disk(&path)?;
    let account = mafile.account_name.clone().ok_or("maFile has no account_name")?;

    let user = SteamUser::new(account.clone(), password).with_mafile(mafile);
    let authenticator = SteamAuthenticator::new(user).login().await?;

    println!("Logged in as {account}.");
    println!(
        "steamLoginSecure cookie present: {}",
        authenticator
            .dump_cookie("steamcommunity.com", "steamLoginSecure")
            .is_some()
    );
    match authenticator.api_key() {
        Some(key) => println!("API key: {key}"),
        None => println!("API key: <none cached>"),
    }
    Ok(())
}
