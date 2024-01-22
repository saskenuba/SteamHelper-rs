//! This module contains the [SteamUser] needed for [crate::SteamAuthenticator] to work.

use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::path::PathBuf;

use derive_more::Deref;
use downcast_rs::DowncastSync;
use steam_totp::Secret;

use crate::errors::AuthError;
use crate::MobileAuthFile;

/// A steam user needed for the authenticator to work.
/// Using a MaFile or not will dictate available methods on [`crate::SteamAuthenticator`]
///
/// A simple implementation that has everything required to work properly below.
/// ```no_run
/// # use steam_mobile::user::SteamUser;
/// SteamUser::new("test_username".to_string(), "password".to_string())
///     .parental_code("1111") // Only needed if the is a parental code, otherwise skip
///     .with_mafile_from_disk("assets/my.maFile")
///     .expect("Failed to find mafile on disk.");
/// ```
#[derive(Clone)]
pub struct SteamUser<MaFileState> {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) parental_code: Option<String>,
    mafile: MaFileState,
}

pub(crate) trait IsUser: DowncastSync {
    fn username(&self) -> &str;
    fn password(&self) -> &str;
}
downcast_rs::impl_downcast!(sync IsUser);

impl<T> IsUser for SteamUser<T>
where
    T: 'static + Send + Sync,
{
    fn username(&self) -> &str {
        &self.username
    }

    fn password(&self) -> &str {
        &self.password
    }
}

impl<'a: 'static, T> IsUser for &'a SteamUser<T>
where
    T: 'static + Send + Sync,
{
    fn username(&self) -> &str {
        &self.username
    }

    fn password(&self) -> &str {
        &self.password
    }
}

/// State where the user has a MaFile.
#[derive(Debug, Clone, Deref)]
pub struct PresentMaFile(MobileAuthFile);

/// State where the user doesn't has a MaFile.
#[derive(Debug, Copy, Clone)]
pub struct AbsentMaFile;

impl<T> Display for SteamUser<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Steam User {}", self.username)
    }
}

impl<T> Debug for SteamUser<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("User")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

impl SteamUser<AbsentMaFile> {
    /// Creates a new valid `User` with the bare minimum credentials.
    #[must_use]
    pub fn new(username: String, password: String) -> SteamUser<AbsentMaFile> {
        SteamUser {
            username,
            password,
            parental_code: None,
            mafile: AbsentMaFile,
        }
    }
}
impl<T> SteamUser<T> {
    /// Sets the account username, mandatory
    #[must_use]
    pub fn username<S>(mut self, username: S) -> Self
    where
        S: ToString,
    {
        self.username = username.to_string();
        self
    }

    /// Sets the account password, mandatory
    #[must_use]
    pub fn password<S>(mut self, password: S) -> Self
    where
        S: ToString,
    {
        self.password = password.to_string();
        self
    }

    /// Sets the parental code, if any.
    #[must_use]
    pub fn parental_code<S>(mut self, parental_code: S) -> Self
    where
        S: ToString,
    {
        self.parental_code = Some(parental_code.to_string());
        self
    }
}

impl SteamUser<AbsentMaFile> {
    /// Convenience function that imports the file from disk
    pub fn with_mafile_from_disk<P>(self, path: P) -> Result<SteamUser<PresentMaFile>, AuthError>
    where
        P: Into<PathBuf>,
    {
        Ok(SteamUser {
            username: self.username,
            password: self.password,
            parental_code: self.parental_code,
            mafile: PresentMaFile(MobileAuthFile::from_disk(path)?),
        })
    }

    #[allow(missing_docs)]
    #[must_use]
    pub fn with_mafile(self, ma_file: MobileAuthFile) -> SteamUser<PresentMaFile> {
        SteamUser {
            username: self.username,
            password: self.password,
            parental_code: self.parental_code,
            mafile: PresentMaFile(ma_file),
        }
    }
}

impl SteamUser<PresentMaFile> {
    pub(crate) fn shared_secret(&self) -> Secret {
        Secret::from_b64(&self.mafile.shared_secret).unwrap()
    }

    pub(crate) fn identity_secret(&self) -> Secret {
        Secret::from_b64(&self.mafile.identity_secret).unwrap()
    }

    pub(crate) fn device_id(&self) -> &str {
        self.mafile.device_id.as_deref().clone().unwrap()
    }
}
