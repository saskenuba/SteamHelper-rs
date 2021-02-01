#![cfg(feature = "cli")]

use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::{App, Arg};
use dialoguer::{Confirm, Input};
use steam_auth::client::SteamAuthenticator;
use steam_auth::errors::{AuthError, LoginError};
use steam_auth::{format_captcha_url, AddAuthenticatorStep, MobileAuthFile, User};
use steam_totp::{generate_auth_code_async, Secret};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new("steam-auth")
        .version("0.1")
        .author("Martin Mariano <contato@martinmariano.com>")
        .subcommand(
            App::new("2fa").about("Generates 2fa codes from shared secrets.").arg(
                Arg::new("shared_secret")
                    .short('s')
                    .long("secret")
                    .required(true)
                    .takes_value(true),
            ),
        )
        .subcommand(
            App::new("auth")
                .about("Authenticator related operations.")
                .subcommand(
                    App::new("add")
                        .long_about(
                            "Adds an authenticator to the account.\n\nDuring the execution of this program, you will \
                             be asked to perform some other operations interactively, such as confirming your email, \
                             or retrieving your SMS code from the number you have provided.",
                        )
                        .about("Add an authenticator to the account.")
                        .args(&[
                            Arg::new("phone_number")
                                .about("Phone number in E.164 format. E.g: +551112345678")
                                .short('n')
                                .long("number")
                                .required(true)
                                .takes_value(true),
                            Arg::new("save_path")
                                .about(
                                    "Recommended. Path where your Mobile Auth(MA) file will be saved. If none is \
                                     provided, file will be printed on stdout.",
                                )
                                .short('p')
                                .long("path")
                                .required(false)
                                .takes_value(true),
                        ]),
                )
                .subcommand(App::new("remove").about("Remove an authenticator from the account."))
                .args(&[
                    Arg::new("account")
                        .about("Steam account name.")
                        .required(true)
                        .takes_value(true),
                    Arg::new("password")
                        .about("Steam account password.")
                        .required(true)
                        .takes_value(true),
                    Arg::new("parental_code")
                        .about("Steam account parental code if any.")
                        .required(false)
                        .takes_value(true),
                ]),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("2fa") {
        let shared_secret = matches.value_of("shared_secret").unwrap();
        let secret = Secret::from_b64(shared_secret).unwrap();

        let auth_code = generate_auth_code_async(secret).await.unwrap();
        println!("{}", auth_code);
    }

    if let Some(matches) = matches.subcommand_matches("auth") {
        let account = matches.value_of("account").unwrap();
        let password = matches.value_of("password").unwrap();
        let parental_code = matches.value_of("parental_code");

        if let Some(add_subcommand) = matches.subcommand_matches("add") {
            let phone_number = add_subcommand.value_of("phone_number").unwrap();
            let save_path = add_subcommand.value_of("save_path").as_deref().map(PathBuf::from);

            let user = User::build().username(account).password(password);
            let authenticator = SteamAuthenticator::new(user);

            match authenticator.login(None).await {
                Ok(_) => (),
                Err(auth_error) => match auth_error {
                    AuthError::Login(login_error) => match login_error {
                        LoginError::CaptchaRequired { captcha_guid } => {
                            println!(
                                "A captcha is required. Open the link to check it: {}",
                                format_captcha_url(&*captcha_guid)
                            );
                            let captcha: String =
                                Input::new().with_prompt("Please enter the captcha:").interact_text()?;
                        }
                        // other LoginErrors
                        _ => panic!(),
                    },
                    // other AuthErrors
                    _ => panic!(),
                },
            }
            println!("Successfully logged in.");

            let mut auth_step = AddAuthenticatorStep::InitialStep;
            let mobile_auth_file;
            loop {
                match authenticator.add_authenticator(auth_step.clone(), phone_number).await {
                    Ok(AddAuthenticatorStep::EmailConfirmation) => {
                        println!(
                            "Phone number was added successfully, A Steam email was sent to your registered inbox to \
                             allow a phone\nnumber to be registered. Please confirm it now.\n"
                        );
                        Confirm::new()
                            .with_prompt("Have you confirmed your email?")
                            .wait_for_newline(true)
                            .interact()?;
                        auth_step = AddAuthenticatorStep::EmailConfirmation;
                    }
                    Ok(AddAuthenticatorStep::MobileAuth(mafile)) => {
                        println!("--- OUTPUT ----");
                        println!("\nThis is your MobileAuth file. Save it!\n");
                        println!("{}", serde_json::to_string_pretty(&mafile).unwrap());
                        println!("\n--- END OF OUTPUT ----\n");

                        if save_path.is_some() {
                            let filename = mafile
                                .account_name
                                .as_ref()
                                .cloned()
                                .unwrap_or_else(|| account.to_string());

                            mobile_auth_file = mafile.clone();
                            tokio::task::spawn_blocking(move || {
                                save_file_to_path(&mafile, &*filename, save_path.unwrap());
                            })
                            .await?;
                            break;
                        }
                    }
                    Err(e) => eprintln!("{:?}", e),
                    _ => println!("wat"),
                }
            }

            let sms_code: String = Input::new()
                .with_prompt("Please write the SMS code you have received on your mobile phone")
                .interact_text()?;
            println!("sms code entered: {}", sms_code);

            authenticator
                .finalize_authenticator(&mobile_auth_file, &*sms_code)
                .await?;

            println!(
                "\nSuccess! Your account has now SteamGuard enabled, on number: {}",
                phone_number
            );
        }

        if let Some(_remove_subcommand) = matches.subcommand_matches("remove") {}
    }

    Ok(())
}

fn save_file_to_path(mafile: &MobileAuthFile, filename: &str, mut path: PathBuf) -> Result<()> {
    path.set_file_name(filename.to_owned() + ".maFile");

    println!("Saving maFile to {:?}\n", &path);

    let file = OpenOptions::new().append(false).write(true).create(true).open(path)?;

    let mut buf_reader = BufWriter::new(file);
    buf_reader
        .write_all(&*serde_json::to_vec_pretty(mafile).unwrap())
        .map_err(|e| e.into())
}
