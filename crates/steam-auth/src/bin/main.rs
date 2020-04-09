#![cfg(feature = "cli")]

use clap::{App, Arg};

use steam_totp::{generate_auth_code_async, Secret};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("steam-auth")
        .version("0.1")
        .author("Martin Mariano <contato@martinmariano.com>")
        .subcommand(
            App::new("2fa").about("Generates 2fa codes from shared secrets.").arg(
                Arg::with_name("shared_secret")
                    .short('s')
                    .long("secret")
                    .required(true)
                    .takes_value(true),
            ),
        )
        .get_matches();

    if let Some(ref matches) = matches.subcommand_matches("2fa") {
        let shared_secret = matches.value_of("shared_secret").unwrap();
        let secret = Secret::from_b64(shared_secret).unwrap();

        let auth_code = generate_auth_code_async(secret).await.unwrap();
        println!("{}", auth_code);
    }

    Ok(())
}
