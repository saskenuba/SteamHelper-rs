//! This module contains the [SteamUser] needed for [crate::SteamAuthenticator] to work.

use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::path::PathBuf;

use derive_more::Deref;
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

/// State where the user has a MaFile.
#[derive(Debug, Clone, Deref)]
pub struct PresentMaFile(MobileAuthFile);

/// State where the user doesn't has a MaFile.
#[derive(Debug, Copy, Clone)]
pub struct AbsentMaFile;

/// State where the user has the MaFile of an authenticator that was added, but not finalized yet.
///
/// The authenticator isn't active until finalized, so logging in doesn't use it.
#[derive(Debug, Clone)]
pub struct PendingMaFile(MobileAuthFile);

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::AbsentMaFile {}
    impl Sealed for super::PendingMaFile {}
    impl Sealed for super::PresentMaFile {}
}

/// The MaFile states a [`SteamUser`] can be in: [`AbsentMaFile`], [`PendingMaFile`] or [`PresentMaFile`].
///
/// Sealed, so it can't be implemented outside this crate.
///
/// Each state only offers what makes sense in it. An account without a MaFile can begin adding an authenticator,
/// which moves it to [`PendingMaFile`], and finalizing moves it to [`PresentMaFile`]:
///
/// ```no_run
/// # use steam_mobile::{Authenticated, SteamAuthenticator, user::AbsentMaFile};
/// # async fn f(authenticator: SteamAuthenticator<Authenticated, AbsentMaFile>) {
/// let (pending, _code_delivery) = authenticator.begin_add_authenticator().await.unwrap();
/// let present = pending.finalize_pending("ABC12").await.map_err(|(_, e)| e).unwrap();
/// present.fetch_confirmations().await.unwrap();
/// # }
/// ```
///
/// An account that already has a MaFile can't begin adding another authenticator:
///
/// ```compile_fail
/// # use steam_mobile::{Authenticated, SteamAuthenticator, user::PresentMaFile};
/// # async fn f(authenticator: SteamAuthenticator<Authenticated, PresentMaFile>) {
/// authenticator.begin_add_authenticator().await;
/// # }
/// ```
///
/// A pending authenticator can't fetch confirmations before it is finalized:
///
/// ```compile_fail
/// # use steam_mobile::{Authenticated, SteamAuthenticator, user::PendingMaFile};
/// # async fn f(pending: SteamAuthenticator<Authenticated, PendingMaFile>) {
/// pending.fetch_confirmations().await;
/// # }
/// ```
///
/// And only a pending authenticator can be finalized:
///
/// ```compile_fail
/// # use steam_mobile::{Authenticated, SteamAuthenticator, user::AbsentMaFile};
/// # async fn f(authenticator: SteamAuthenticator<Authenticated, AbsentMaFile>) {
/// authenticator.finalize_pending("ABC12").await;
/// # }
/// ```
pub trait MaFileState: sealed::Sealed + 'static + Send + Sync + Clone {
    /// The MaFile of the account's active authenticator, which login uses for its codes and must complete with the
    /// device ID confirmations need. A pending MaFile isn't active yet.
    #[doc(hidden)]
    fn active_mafile(&self) -> Option<&MobileAuthFile> {
        None
    }

    #[doc(hidden)]
    fn active_mafile_mut(&mut self) -> Option<&mut MobileAuthFile> {
        None
    }
}

impl MaFileState for AbsentMaFile {}

impl MaFileState for PendingMaFile {}

impl MaFileState for PresentMaFile {
    fn active_mafile(&self) -> Option<&MobileAuthFile> {
        Some(&self.0)
    }

    fn active_mafile_mut(&mut self) -> Option<&mut MobileAuthFile> {
        Some(&mut self.0)
    }
}

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
        self.with_mafile_state(PresentMaFile(ma_file))
    }

    /// Uses the MaFile of an authenticator that was added but not finalized yet, such as one saved by an interrupted
    /// run, so it can be finalized.
    #[must_use]
    pub fn with_pending_mafile(self, ma_file: MobileAuthFile) -> SteamUser<PendingMaFile> {
        self.with_mafile_state(PendingMaFile(ma_file))
    }
}

impl<M> SteamUser<M> {
    fn with_mafile_state<N>(self, mafile: N) -> SteamUser<N> {
        SteamUser {
            username: self.username,
            password: self.password,
            parental_code: self.parental_code,
            mafile,
        }
    }
}

impl<M: MaFileState> SteamUser<M> {
    pub(crate) fn active_mafile(&self) -> Option<&MobileAuthFile> {
        self.mafile.active_mafile()
    }

    pub(crate) fn active_mafile_mut(&mut self) -> Option<&mut MobileAuthFile> {
        self.mafile.active_mafile_mut()
    }
}

impl SteamUser<PendingMaFile> {
    pub(crate) fn mafile(&self) -> &MobileAuthFile {
        &self.mafile.0
    }

    /// The authenticator was finalized, so its MaFile is now the account's.
    pub(crate) fn into_present(self) -> SteamUser<PresentMaFile> {
        SteamUser {
            username: self.username,
            password: self.password,
            parental_code: self.parental_code,
            mafile: PresentMaFile(self.mafile.0),
        }
    }
}

impl SteamUser<PresentMaFile> {
    pub(crate) fn identity_secret(&self) -> Secret {
        Secret::from_b64(&self.mafile.identity_secret).unwrap()
    }

    /// Only called once authenticated: login fetches a missing device ID from Steam, and fails if it can't.
    pub(crate) fn device_id(&self) -> &str {
        self.mafile
            .device_id()
            .expect("Login recovers the device ID, so an authenticated MaFile always has one.")
    }

    pub(crate) fn mafile(&self) -> &MobileAuthFile {
        &self.mafile
    }
}
