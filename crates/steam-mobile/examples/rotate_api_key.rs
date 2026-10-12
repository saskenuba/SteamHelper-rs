//! Rotates an account's Steam Web API key end to end: log in, revoke the current key, then register a fresh one.
//!
//! Registering a key requires accepting a mobile confirmation, which the authenticator does automatically using the
//! maFile's identity secret.
//!
//! The password is read from `STEAM_PASSWORD`, so it stays out of the shell history and the process list.
//!
//! ```sh
//! STEAM_PASSWORD=... cargo run -p steam-mobile --example rotate_api_key -- path/to/account.maFile
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

    let path = std::env::args()
        .nth(1)
        .ok_or("usage: rotate_api_key <path to maFile>")?;
    let password = std::env::var("STEAM_PASSWORD").map_err(|_| "STEAM_PASSWORD is not set")?;

    let mafile = MobileAuthFile::from_disk(&path)?;
    let account = mafile.account_name.clone().ok_or("maFile has no account_name")?;

    let user = SteamUser::new(account.clone(), password).with_mafile(mafile);
    let mut authenticator = SteamAuthenticator::new(user).login().await?;
    println!("Logged in as {account}.");

    match authenticator.api_key() {
        Some(key) => println!("Key before rotation: {key}"),
        None => println!("Key before rotation: <none registered>"),
    }

    println!("Revoking the current key...");
    authenticator.revoke_api_key().await?;
    println!("Revoked. Cached key is now: {:?}", authenticator.api_key());

    println!("Registering a new key (auto-accepting the mobile confirmation)...");
    let new_key = authenticator.register_api_key().await?;
    println!("Key after rotation:  {new_key}");

    Ok(())
}
