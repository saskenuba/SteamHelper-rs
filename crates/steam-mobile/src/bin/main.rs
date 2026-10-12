#![cfg(feature = "cli")]

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use dialoguer::Confirm;
use dialoguer::Input;
use dialoguer::Password;
use futures_util::TryFutureExt;
use steam_mobile::errors::AuthError;
use steam_mobile::errors::LinkerError;
use steam_mobile::user::PresentMaFile;
use steam_mobile::user::SteamUser;
use steam_mobile::AddAuthenticatorStep;
use steam_mobile::Authenticated;
use steam_mobile::Confirmation;
use steam_mobile::ConfirmationAction;
use steam_mobile::EConfirmationType;
use steam_mobile::MobileAuthFile;
use steam_mobile::SteamAuthenticator;

/// Environment variable read for the account password. If unset, the password is prompted for.
///
/// The password is never taken as an argument, since arguments end up in the shell history and the process list.
const PASSWORD_ENV: &str = "STEAM_PASSWORD";

/// Steam Guard from the command line: login codes, mobile confirmations and authenticators.
#[derive(Parser)]
#[command(name = "steam-mobile", version)]
struct Cli {
    /// Path to the account's maFile (MobileAuth file).
    #[arg(long, global = true, env = "STEAM_MAFILE", value_name = "PATH")]
    mafile: Option<PathBuf>,

    /// Steam account name. Defaults to the one stored in the maFile.
    #[arg(long, global = true, env = "STEAM_ACCOUNT", value_name = "NAME")]
    account: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the current Steam Guard login code.
    Code,
    /// List, accept or deny mobile confirmations.
    #[command(subcommand)]
    Confirmations(ConfirmationsCommand),
    /// Manage the account's Steam Guard authenticator.
    #[command(subcommand)]
    Authenticator(AuthenticatorCommand),
}

#[derive(Subcommand)]
enum ConfirmationsCommand {
    /// List pending confirmations.
    List,
    /// Accept pending confirmations.
    Accept(Selector),
    /// Deny pending confirmations.
    Deny(Selector),
}

/// Which pending confirmations to act on. Exactly one way of selecting them must be given.
#[derive(Args)]
#[group(required = true, multiple = false)]
struct Selector {
    /// Confirmation ids, as shown by `confirmations list`.
    ids: Vec<String>,

    /// The confirmation for this trade offer.
    #[arg(long, value_name = "ID")]
    trade_offer: Option<u64>,

    /// Every confirmation of this kind. Can be repeated.
    #[arg(long, value_enum)]
    kind: Vec<Kind>,

    /// Every pending confirmation, of every kind. Be careful with this!
    #[arg(long)]
    all: bool,
}

#[derive(Subcommand)]
enum AuthenticatorCommand {
    /// Add an authenticator to the account.
    ///
    /// You will be asked to do some steps interactively along the way, such as confirming your email and entering
    /// the SMS code sent to your phone.
    Add {
        /// Phone number in E.164 format. E.g: +551112345678
        #[arg(long)]
        phone: String,

        /// Recommended. Directory where your maFile will be saved, as <account>.maFile. An existing file is never
        /// overwritten. If none is provided, the file is printed on stdout.
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
    },
}

/// Confirmation kinds, as named on the command line.
#[derive(ValueEnum, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Trade,
    MarketListing,
    MarketPurchase,
    /// Both family-invite and family-join.
    Family,
    FamilyInvite,
    FamilyJoin,
    AddUser,
    ApiKey,
    PhoneChange,
    AccountRecovery,
    FeatureOptOut,
    Refund,
}

impl Kind {
    fn types(self) -> &'static [EConfirmationType] {
        match self {
            Self::Trade => &[EConfirmationType::Trade],
            Self::MarketListing => &[EConfirmationType::Market],
            Self::MarketPurchase => &[EConfirmationType::MarketPurchase],
            Self::Family => &[
                EConfirmationType::InviteToFamilyGroup,
                EConfirmationType::JoinFamilyGroup,
            ],
            Self::FamilyInvite => &[EConfirmationType::InviteToFamilyGroup],
            Self::FamilyJoin => &[EConfirmationType::JoinFamilyGroup],
            Self::AddUser => &[EConfirmationType::AddUser],
            Self::ApiKey => &[EConfirmationType::APIKey],
            Self::PhoneChange => &[EConfirmationType::PhoneNumberChange],
            Self::AccountRecovery => &[EConfirmationType::AccountRecovery],
            Self::FeatureOptOut => &[EConfirmationType::FeatureOptOut],
            Self::Refund => &[EConfirmationType::RequestRefund],
        }
    }

    /// The command line name of `kind`, as accepted by `--kind`.
    fn name_of(kind: EConfirmationType) -> String {
        Self::value_variants()
            .iter()
            .find(|k| k.types() == [kind])
            .and_then(|k| k.to_possible_value())
            .map_or_else(|| "unknown".to_string(), |v| v.get_name().to_string())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Code => {
            let mafile = read_mafile(cli.mafile.as_deref())?;
            println!("{}", mafile.auth_code().await?);
        }
        Command::Confirmations(command) => {
            let mafile = read_mafile(cli.mafile.as_deref())?;
            let account = account_name(cli.account, &mafile)?;
            run_confirmations(command, account, mafile).await?;
        }
        Command::Authenticator(AuthenticatorCommand::Add { phone, out }) => {
            let account = cli
                .account
                .context("Pass the account to add the authenticator to with --account or STEAM_ACCOUNT.")?;

            let password = read_password(&account)?;
            let authenticator = login(SteamUser::new(account.clone(), password)).await?;

            let mobile_auth_file = add_authenticator(&authenticator, &phone).await?;
            persist_mafile(&mobile_auth_file, &account, out).await?;

            let sms_code: String = Input::new()
                .with_prompt("Please write the SMS code you have received on your mobile phone")
                .interact_text()?;

            authenticator
                .finalize_authenticator(&mobile_auth_file, &sms_code)
                .await?;

            println!("\nSuccess! Your account has now SteamGuard enabled, on number: {phone}");
        }
    }

    Ok(())
}

fn read_mafile(path: Option<&Path>) -> Result<MobileAuthFile> {
    let path = path.context("Pass the maFile with --mafile or STEAM_MAFILE.")?;
    MobileAuthFile::from_disk(path).with_context(|| format!("Failed to read the maFile at {}", path.display()))
}

fn account_name(account: Option<String>, mafile: &MobileAuthFile) -> Result<String> {
    account
        .or_else(|| mafile.account_name.clone())
        .context("The maFile doesn't name its account. Pass it with --account or STEAM_ACCOUNT.")
}

async fn run_confirmations(command: ConfirmationsCommand, account: String, mafile: MobileAuthFile) -> Result<()> {
    let password = read_password(&account)?;
    let authenticator = login(SteamUser::new(account, password).with_mafile(mafile)).await?;

    println!("Fetching pending confirmations...");
    let confirmations: Vec<Confirmation> = authenticator.fetch_confirmations().await?.into_iter().collect();

    let (action, selector) = match command {
        ConfirmationsCommand::List => {
            print_confirmations(&confirmations);
            return Ok(());
        }
        ConfirmationsCommand::Accept(selector) => (ConfirmationAction::Accept, selector),
        ConfirmationsCommand::Deny(selector) => (ConfirmationAction::Deny, selector),
    };

    let selected = select(confirmations, &selector)?;
    if selected.is_empty() {
        println!("No pending confirmation matches.");
        return Ok(());
    }

    print_confirmations(&selected);
    process(&authenticator, action, selected).await
}

/// Picks the confirmations `selector` asks for, failing if any explicitly requested one isn't pending.
fn select(confirmations: Vec<Confirmation>, selector: &Selector) -> Result<Vec<Confirmation>> {
    if let Some(offer_id) = selector.trade_offer {
        let selected: Vec<_> = confirmations
            .into_iter()
            .filter(|c| c.has_trade_offer_id(offer_id))
            .collect();
        if selected.is_empty() {
            bail!("Couldn't find a pending confirmation for trade offer {offer_id}.");
        }
        return Ok(selected);
    }

    if !selector.ids.is_empty() {
        let missing: Vec<&str> = selector
            .ids
            .iter()
            .filter(|id| !confirmations.iter().any(|c| &c.id == *id))
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            bail!("No pending confirmation with id {}.", missing.join(", "));
        }
        return Ok(confirmations
            .into_iter()
            .filter(|c| selector.ids.contains(&c.id))
            .collect());
    }

    if !selector.kind.is_empty() {
        return Ok(confirmations
            .into_iter()
            .filter(|c| selector.kind.iter().any(|k| k.types().contains(&c.kind)))
            .collect());
    }

    Ok(confirmations)
}

async fn process(
    authenticator: &SteamAuthenticator<Authenticated, PresentMaFile>,
    action: ConfirmationAction,
    confirmations: Vec<Confirmation>,
) -> Result<()> {
    let total = confirmations.len();
    authenticator
        .process_confirmations(action, confirmations)
        .await
        .with_context(|| format!("Error processing {total} confirmations."))?;

    let verb = if action == ConfirmationAction::Accept {
        "Accepted"
    } else {
        "Denied"
    };
    println!("{verb} {total} confirmations.");
    Ok(())
}

fn print_confirmations(confirmations: &[Confirmation]) {
    if confirmations.is_empty() {
        println!("No pending confirmations. If you just did something that needs one, it may take a while to show up.");
        return;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);

    println!(
        "{:<12} {:<17} {:<12} {:>8}  DESCRIPTION",
        "ID", "KIND", "CREATOR", "AGE"
    );
    for c in confirmations {
        println!(
            "{:<12} {:<17} {:<12} {:>8}  {}",
            c.id,
            Kind::name_of(c.kind),
            c.creator_id,
            format_age(now - c.creation_time),
            c.type_name
        );
    }
}

fn format_age(seconds: i64) -> String {
    match seconds.max(0) {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s if s < 86400 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86400),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn confirmation(id: &str, kind: EConfirmationType, creator_id: &str) -> Confirmation {
        Confirmation {
            id: id.to_string(),
            key: String::new(),
            kind,
            creation_time: 0,
            creator_id: creator_id.to_string(),
            type_name: String::new(),
        }
    }

    fn pending() -> Vec<Confirmation> {
        vec![
            confirmation("1", EConfirmationType::Trade, "100"),
            confirmation("2", EConfirmationType::JoinFamilyGroup, "200"),
            confirmation("3", EConfirmationType::InviteToFamilyGroup, "300"),
            confirmation("4", EConfirmationType::AccountRecovery, "400"),
        ]
    }

    fn selector(args: &[&str]) -> Selector {
        let mut argv = vec!["steam-mobile", "confirmations", "accept"];
        argv.extend_from_slice(args);
        match Cli::try_parse_from(argv).unwrap().command {
            Command::Confirmations(ConfirmationsCommand::Accept(selector)) => selector,
            _ => unreachable!(),
        }
    }

    fn ids(confirmations: Vec<Confirmation>) -> Vec<String> {
        confirmations.into_iter().map(|c| c.id).collect()
    }

    #[test]
    fn selects_by_kind() {
        assert_eq!(
            ids(select(pending(), &selector(&["--kind", "family"])).unwrap()),
            ["2", "3"]
        );
        assert_eq!(
            ids(select(pending(), &selector(&["--kind", "family-join", "--kind", "trade"])).unwrap()),
            ["1", "2"]
        );
    }

    #[test]
    fn selects_by_id_and_rejects_unknown_ids() {
        assert_eq!(ids(select(pending(), &selector(&["4", "1"])).unwrap()), ["1", "4"]);
        assert!(select(pending(), &selector(&["1", "9"])).is_err());
    }

    #[test]
    fn selects_by_trade_offer_only_for_trades() {
        assert_eq!(
            ids(select(pending(), &selector(&["--trade-offer", "100"])).unwrap()),
            ["1"]
        );
        assert!(select(pending(), &selector(&["--trade-offer", "200"])).is_err());
    }

    #[test]
    fn all_selects_everything() {
        assert_eq!(
            ids(select(pending(), &selector(&["--all"])).unwrap()),
            ["1", "2", "3", "4"]
        );
    }

    #[test]
    fn kind_names_round_trip() {
        assert_eq!(Kind::name_of(EConfirmationType::JoinFamilyGroup), "family-join");
        assert_eq!(Kind::name_of(EConfirmationType::Unknown), "unknown");
    }
}
