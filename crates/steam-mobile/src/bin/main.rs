#![cfg(feature = "cli")]

use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Result;
use clap::Arg;
use clap::ArgMatches;
use clap::Command;
use dialoguer::Confirm;
use dialoguer::Input;
use futures_util::TryFutureExt;
use steam_mobile::user::PresentMaFile;
use steam_mobile::user::SteamUser;
use steam_mobile::AddAuthenticatorStep;
use steam_mobile::Authenticated;
use steam_mobile::ConfirmationAction;
use steam_mobile::MobileAuthFile;
use steam_mobile::SteamAuthenticator;
use steam_totp::generate_auth_code_async;
use steam_totp::Secret;
use strum_macros::AsRefStr;
use strum_macros::EnumString;
use strum_macros::IntoStaticStr;

#[derive(EnumString, IntoStaticStr, AsRefStr)]
enum MainCommands {
    #[strum(serialize = "confirmation")]
    Confirmations,
    #[strum(serialize = "2fa")]
    GenCodes,
    #[strum(serialize = "auth")]
    Authenticator,
}

fn cli() -> Command<'static> {
    let shared_secret_arg = Arg::new("shared_secret")
        .short('s')
        .long("secret")
        .required(true)
        .takes_value(true);

    let trading_args = &[
        Arg::new("all")
            .short('a')
            .help("Act on all trade requests. Be careful with this!"),
        Arg::new("tradeoffer_id")
            .short('i')
            .help("Act on a single tradeoffer_id")
            .required_unless_present("all")
            .conflicts_with("all")
            .takes_value(true),
    ];
    Command::new("steam-mobile")
        .version("0.1")
        .author("Martin Mariano <contato@martinmariano.com>")
        .subcommand(
            Command::new(MainCommands::GenCodes.as_ref())
                .about("Generates 2fa codes from shared secrets.")
                .arg(shared_secret_arg.clone()),
        )
        .subcommand(
            Command::new(MainCommands::Confirmations.as_ref())
                .about("Accept and deny trade requests.")
                .args(&[
                    Arg::new("account")
                        .help("Steam account name.")
                        .required(true)
                        .takes_value(true),
                    Arg::new("password")
                        .help("Steam account password.")
                        .required(true)
                        .takes_value(true),
                    Arg::new("ma_file_path")
                        .help("Path to MaFile (MobileAuth File)")
                        .required(true)
                        .takes_value(true),
                    Arg::new("parental_code")
                        .help("Steam account parental code if any.")
                        .required(false)
                        .takes_value(true),
                ])
                .subcommands(vec![
                    Command::new("accept").args(trading_args.clone()),
                    Command::new("deny").args(trading_args),
                ]),
        )
        .subcommand(
            Command::new(MainCommands::Authenticator.as_ref())
                .about("Authenticator related operations.")
                .subcommand(
                    Command::new("add")
                        .long_about(
                            "Adds an authenticator to the account.\n\nDuring the execution of this program, you will \
                             be asked to perform some other operations interactively, such as confirming your email, \
                             or retrieving your SMS code from the number you have provided.",
                        )
                        .about("Add an authenticator to the account.")
                        .args(&[
                            Arg::new("phone_number")
                                .help("Phone number in E.164 format. E.g: +551112345678")
                                .short('n')
                                .long("number")
                                .required(true)
                                .takes_value(true),
                            Arg::new("save_path")
                                .help(
                                    "Recommended. Path where your Mobile Auth(MA) file will be saved. If none is \
                                     provided, file will be printed on stdout.",
                                )
                                .short('p')
                                .long("path")
                                .required(false)
                                .takes_value(true),
                        ]),
                )
                .subcommand(Command::new("remove").about("Remove an authenticator from the account."))
                .args(&[
                    Arg::new("account")
                        .help("Steam account name.")
                        .required(true)
                        .takes_value(true),
                    Arg::new("password")
                        .help("Steam account password.")
                        .required(true)
                        .takes_value(true),
                    Arg::new("parental_code")
                        .help("Steam account parental code if any.")
                        .required(false)
                        .takes_value(true),
                ]),
        )
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = cli().get_matches();

    let subcommand = matches
        .subcommand()
        .map(|(subcommand, tail)| (MainCommands::from_str(subcommand).unwrap(), tail));

    match subcommand {
        Some((MainCommands::GenCodes, matches)) => {
            let shared_secret = matches.value_of("shared_secret").unwrap();
            let secret = Secret::from_b64(shared_secret).unwrap();

            let auth_code = generate_auth_code_async(secret).await.unwrap();
            println!("{}", auth_code);

            return Ok(());
        }

        Some((MainCommands::Confirmations, matches)) => {
            process_confirmations(matches).await?;
        }

        Some((MainCommands::Authenticator, matches)) => {
            let account = matches.value_of("account").unwrap();
            let password = matches.value_of("password").unwrap();
            let _parental_code = matches.value_of("parental_code");

            if let Some(add_subcommand) = matches.subcommand_matches("add") {
                let phone_number = add_subcommand.value_of("phone_number").unwrap();
                let save_path = add_subcommand.value_of("save_path").as_deref().map(PathBuf::from);

                let authenticator = handle_login(account, password, None).await?;

                let mut auth_step = AddAuthenticatorStep::InitialStep;
                let mobile_auth_file;
                loop {
                    match authenticator.add_authenticator(auth_step.clone(), phone_number).await {
                        Ok(AddAuthenticatorStep::EmailConfirmation) => {
                            println!(
                                "Phone number was added successfully, A Steam email was sent to your registered inbox \
                                 to allow a phone\nnumber to be registered. Please confirm it now.\n"
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
                                    save_file_to_path(&mafile, &*filename, save_path.unwrap()).unwrap();
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

                return Ok(());
            }

            if let Some(_remove_subcommand) = matches.subcommand_matches("remove") {
                return Ok(());
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

// TODO: Respect --all flag
// TODO: Respect -i flag to process only desired tradeoffer_id
async fn process_confirmations(subcomm_args: &ArgMatches) -> Result<()> {
    let account = subcomm_args.value_of("account").unwrap();
    let password = subcomm_args.value_of("password").unwrap();
    let ma_file = subcomm_args
        .value_of("ma_file_path")
        .map(MobileAuthFile::from_disk)
        .and_then(Result::ok)
        .expect("MaFile needs to be exist in order to send confirmations.");

    let confirmation_method = match subcomm_args.subcommand() {
        Some(("accept", _)) => ConfirmationAction::Accept,
        Some(("deny", _)) => ConfirmationAction::Deny,
        _ => unreachable!(),
    };

    let authenticator = handle_login(account, password, Some(ma_file)).await?;

    println!("Please wait while we fetch your pending confirmations...");
    let confirmations = authenticator.fetch_confirmations().await?;

    if confirmations.is_empty() {
        println!(
            "Couldn't find any confirmations. If you did any operation that requires a confirmation, it may take a \
             while to receive them."
        );
        return Ok(());
    }

    let total_confirmations = confirmations.len();
    let process_results = authenticator
        .process_confirmations(confirmation_method, confirmations)
        .await;

    if process_results.is_ok() {
        println!("Success! {total_confirmations} confirmations were processed.");
    } else {
        println!("Error processing {total_confirmations} confirmations.");
    }

    Ok(())
}

async fn handle_login(
    account: &str,
    password: &str,
    shared_secret: Option<MobileAuthFile>,
) -> Result<SteamAuthenticator<Authenticated, PresentMaFile>> {
    let user = shared_secret
        .map(|ma_file| SteamUser::new(account.to_string(), password.to_string()).with_mafile(ma_file))
        .expect("Safe.");

    let authenticator = SteamAuthenticator::new(user);
    authenticator.login().err_into().await
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
