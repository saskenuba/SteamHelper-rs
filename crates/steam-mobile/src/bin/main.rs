#![cfg(feature = "cli")]

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use clap::Arg;
use clap::ArgMatches;
use clap::Command;
use dialoguer::Confirm;
use dialoguer::Input;
use dialoguer::Password;
use futures_util::TryFutureExt;
use steam_mobile::errors::AuthError;
use steam_mobile::errors::LinkerError;
use steam_mobile::user::SteamUser;
use steam_mobile::AddAuthenticatorStep;
use steam_mobile::Authenticated;
use steam_mobile::Confirmation;
use steam_mobile::ConfirmationAction;
use steam_mobile::MobileAuthFile;
use steam_mobile::SteamAuthenticator;
use steam_totp::generate_auth_code_async;
use steam_totp::Secret;
use strum_macros::AsRefStr;
use strum_macros::EnumString;
use strum_macros::IntoStaticStr;

/// Environment variable read for the account password. If unset, the password is prompted for.
///
/// The password is never taken as an argument, since arguments end up in the shell history and the process list.
const PASSWORD_ENV: &str = "STEAM_PASSWORD";

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
        .subcommand_required(true)
        .arg_required_else_help(true)
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
                                    "Recommended. Directory where your Mobile Auth(MA) file will be saved, as \
                                     <account>.maFile. An existing file is never overwritten. If none is provided, \
                                     the file will be printed on stdout.",
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
            let _parental_code = matches.value_of("parental_code");

            if let Some(add_subcommand) = matches.subcommand_matches("add") {
                let phone_number = add_subcommand.value_of("phone_number").unwrap();
                let save_path = add_subcommand.value_of("save_path").map(PathBuf::from);

                let password = read_password(account)?;
                let authenticator = login(SteamUser::new(account.to_string(), password)).await?;

                let mobile_auth_file = add_authenticator(&authenticator, phone_number).await?;
                persist_mafile(&mobile_auth_file, account, save_path).await?;

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

/// Drives the add authenticator steps until Steam hands us the maFile.
async fn add_authenticator<M>(
    authenticator: &SteamAuthenticator<Authenticated, M>,
    phone_number: &str,
) -> Result<MobileAuthFile>
where
    M: Send + Sync,
{
    let mut auth_step = AddAuthenticatorStep::InitialStep;
    loop {
        match authenticator.add_authenticator(auth_step.clone(), phone_number).await {
            Ok(AddAuthenticatorStep::EmailConfirmation) => {
                println!(
                    "Phone number was added successfully, A Steam email was sent to your registered inbox to allow a \
                     phone\nnumber to be registered. Please confirm it now.\n"
                );
                wait_for_email_confirmation()?;
                auth_step = AddAuthenticatorStep::EmailConfirmation;
            }
            Ok(AddAuthenticatorStep::MobileAuth(mafile)) => return Ok(mafile),
            Ok(AddAuthenticatorStep::InitialStep) => bail!("Unexpected step returned while adding the authenticator."),
            Err(AuthError::AuthenticatorError(LinkerError::EmailNotConfirmed)) => {
                println!("Steam reports the email was not confirmed yet.");
                wait_for_email_confirmation()?;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn wait_for_email_confirmation() -> Result<()> {
    let confirmed = Confirm::new()
        .with_prompt("Have you confirmed your email?")
        .wait_for_newline(true)
        .interact()?;
    if !confirmed {
        bail!("Email was not confirmed. Aborting.");
    }
    Ok(())
}

/// Saves the maFile to `save_dir` or prints it, making sure the user has it before the authenticator is finalized.
async fn persist_mafile(mafile: &MobileAuthFile, account: &str, save_dir: Option<PathBuf>) -> Result<()> {
    if let Some(save_dir) = save_dir {
        let filename = mafile.account_name.clone().unwrap_or_else(|| account.to_string());
        let to_save = mafile.clone();
        let saved = tokio::task::spawn_blocking(move || save_file_to_path(&to_save, &filename, &save_dir)).await?;

        match saved {
            Ok(path) => {
                println!("Your MobileAuth file was saved to {}", path.display());
                return Ok(());
            }
            // The secrets cannot be fetched again, so they are printed instead of being lost.
            Err(e) => eprintln!("Failed to save your MobileAuth file: {e:#}"),
        }
    }

    println!("--- OUTPUT ----");
    println!("\nThis is your MobileAuth file. Save it!\n");
    println!("{}", serde_json::to_string_pretty(mafile)?);
    println!("\n--- END OF OUTPUT ----\n");

    let saved = Confirm::new()
        .with_prompt("Have you saved your MobileAuth file? Without it you WILL lose access to your account")
        .wait_for_newline(true)
        .interact()?;
    if !saved {
        bail!("MobileAuth file was not saved. The authenticator will not be finalized.");
    }
    Ok(())
}

async fn process_confirmations(subcomm_args: &ArgMatches) -> Result<()> {
    let account = subcomm_args.value_of("account").unwrap();
    let ma_file_path = subcomm_args.value_of("ma_file_path").unwrap();
    let ma_file = MobileAuthFile::from_disk(ma_file_path)
        .with_context(|| format!("Failed to read the MaFile at {ma_file_path}"))?;

    let (confirmation_method, action_args) = match subcomm_args.subcommand() {
        Some(("accept", args)) => (ConfirmationAction::Accept, args),
        Some(("deny", args)) => (ConfirmationAction::Deny, args),
        _ => bail!("Choose to either `accept` or `deny` confirmations."),
    };

    let trade_offer_id = if action_args.is_present("all") {
        None
    } else {
        let id = action_args.value_of("tradeoffer_id").unwrap();
        Some(
            id.parse::<u64>()
                .with_context(|| format!("Invalid trade offer id: {id}"))?,
        )
    };

    let password = read_password(account)?;
    let user = SteamUser::new(account.to_string(), password).with_mafile(ma_file);
    let authenticator = login(user).await?;

    println!("Please wait while we fetch your pending confirmations...");
    let confirmations = authenticator.fetch_confirmations().await?;

    if confirmations.is_empty() {
        println!(
            "Couldn't find any confirmations. If you did any operation that requires a confirmation, it may take a \
             while to receive them."
        );
        return Ok(());
    }

    let selected: Vec<Confirmation> = match trade_offer_id {
        None => confirmations.into_iter().collect(),
        Some(id) => confirmations.into_iter().filter(|c| c.has_trade_offer_id(id)).collect(),
    };

    if let (Some(id), true) = (trade_offer_id, selected.is_empty()) {
        bail!("Couldn't find a pending confirmation for trade offer {id}.");
    }

    let total_confirmations = selected.len();
    authenticator
        .process_confirmations(confirmation_method, selected)
        .await
        .with_context(|| format!("Error processing {total_confirmations} confirmations."))?;

    println!("Success! {total_confirmations} confirmations were processed.");
    Ok(())
}

fn read_password(account: &str) -> Result<String> {
    if let Ok(password) = std::env::var(PASSWORD_ENV) {
        return Ok(password);
    }
    Password::new()
        .with_prompt(format!("Steam password for {account}"))
        .interact()
        .map_err(Into::into)
}

async fn login<M>(user: SteamUser<M>) -> Result<SteamAuthenticator<Authenticated, M>>
where
    M: 'static + Send + Sync + Clone,
{
    SteamAuthenticator::new(user).login().err_into().await
}

/// Writes the maFile as `<filename>.maFile` inside `dir`, readable only by the owner.
///
/// Never overwrites an existing file, since it may hold the secrets of another authenticator.
fn save_file_to_path(mafile: &MobileAuthFile, filename: &str, dir: &Path) -> Result<PathBuf> {
    let path = dir.join(format!("{filename}.maFile"));

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options
        .open(&path)
        .with_context(|| format!("Failed to create {}", path.display()))?;
    file.write_all(&serde_json::to_vec_pretty(mafile)?)?;
    file.sync_all()?;

    Ok(path)
}
