//!
//! Provides a wrapper around documented and some undocumented Steam API endpoints.
//!
//! The `tappet` crate provides a practical way to query the Steam Web API using strongly typed
//! methods.
//!
//! # Endpoints
//!
//! You can check the available interfaces for querying from the `QueryBuilder` structs.
//!
//! * [`GetQueryBuilder`](struct.GetQueryBuilder.html): has all available interfaces for the GET method.
//! * [`PostQueryBuilder`](struct.PostQueryBuilder.html): has all available interfaces for the POST method.
//!
//! Each time you "select" a interface, such as the `GetQueryBuilder`, a new struct is created where all methods
//! are endpoints available.
//!
//! # Usage
//!
//! ```no_run
//! use tappet::{SteamAPI, Executor};
//! use tappet::response_types;
//! use anyhow::Result;
//! # use tokio;
//!
//! # #[tokio::main]
//! async fn main() -> Result<()> {
//!     let client = SteamAPI::new(std::env!("STEAM_API"));
//!     let generic_response = client
//!         .get()
//!         .ISteamUser()
//!         .GetPlayerSummaries(vec!["789451224515".to_string()])
//!         .execute()
//!         .await?;
//!     Ok(())
//! }
//! ```
//!
//! # Access tokens
//!
//! Instead of an API key, requests can carry a web session access token (`access_token=` instead of `key=`), see
//! [`Credential`]. Tokens expire, so build a `SteamAPI` per call from the current token and share one HTTP client:
//!
//! ```no_run
//! use tappet::{Credential, SteamAPI, Executor};
//! use anyhow::Result;
//! # use tokio;
//!
//! # #[tokio::main]
//! async fn main() -> Result<()> {
//!     let http_client = reqwest::Client::new();
//!     # let current_access_token = String::new();
//!     let client = SteamAPI::with_client(http_client.clone(), Credential::AccessToken(current_access_token));
//!     let summary = client.get().IEconService().GetTradeOffersSummary(None).execute().await?;
//!     Ok(())
//! }
//! ```
//!
//! # Reuse
//!
//! There are some endpoints that only returns information for the account vinculated with the api key that you are using at the moment.
//! `tappet` has a convenience function that circunvents this, allow the user to inject a custom api key before the request is made.
//!
//! This can be useful if you don't want to keep instantiating new clients every time you want to call with a different api keys,
//! but still need to to call agnostic methods that doesn't return api specific information.
//!
//! But a "master" api key is still needed to instantiate `SteamAPI` in order to avoid panics.
//!
//! ```no_run
//! use tappet::{SteamAPI, Executor};
//! use tappet::response_types;
//! use anyhow::Result;
//! # use tokio;
//!
//! # #[tokio::main]
//! async fn main() -> Result<()> {
//!     let client = SteamAPI::new(std::env!("STEAM_API"));
//!     client
//!         .get()
//!         .ISteamUser()
//!         .GetPlayerSummaries(vec!["789451224515".to_string()])
//!         .inject_custom_key("C704578DF5E380C5F8A89B8F8A0814B8")
//!         .execute()
//!         .await?;
//!     Ok(())
//! }
//! ```

#![allow(non_snake_case)]
#![allow(unused_imports)]

#[cfg(feature = "async")]
use async_trait::async_trait;

#[cfg(feature = "async")]
pub use async_client::*;

use crate::errors::SteamAPIError;

#[macro_use]
mod macros;

pub mod endpoints;
pub mod errors;
mod helpers;
pub mod response_types;

#[cfg(feature = "trading")]
mod trading_types;

#[cfg(test)]
mod request_tests;

pub type Result<T> = std::result::Result<T, SteamAPIError>;

/// How requests authenticate to the Web API.
///
/// Its `Debug` output never shows the secret.
#[derive(Clone, PartialEq, Eq)]
pub enum Credential {
    /// A Web API key, sent as `key=`.
    ApiKey(String),
    /// A web session access token, sent as `access_token=` instead of `key=`, as node-steam-tradeoffer-manager does
    /// (`lib/webapi.js`). It expires (the session layer renews it about daily), so build a [`SteamAPI`] per call from
    /// the current token with `SteamAPI::with_client`.
    AccessToken(String),
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Credential::ApiKey(_) => f.write_str("ApiKey(<redacted>)"),
            Credential::AccessToken(_) => f.write_str("AccessToken(<redacted>)"),
        }
    }
}

#[cfg(feature = "blocking")]
pub mod blocking {
    use serde::de::DeserializeOwned;

    use crate::{Credential, Result};

    /// Requests the endpoint and returns the raw response.
    pub trait Executor {
        fn execute(self) -> Result<String>;
    }

    /// Requests the endpoint and returns the proper deserialized response.
    /// Response types are exposed on `tappet::response_types`.
    pub trait ExecutorResponse<T: DeserializeOwned> {
        fn execute_with_response(self) -> Result<T>;
    }

    pub struct SteamAPI {
        pub(crate) client: reqwest::blocking::Client,
        /// Mandatory for some operations
        pub(crate) credential: Credential,
    }

    impl std::fmt::Debug for SteamAPI {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("SteamAPI")
                .field("client", &self.client)
                .field("credential", &self.credential)
                .finish()
        }
    }

    impl SteamAPI {
        /// Creates a new SteamAPI Client with an API Key.
        pub fn new<T: ToString>(api_key: T) -> SteamAPI {
            Self::with_client(Default::default(), Credential::ApiKey(api_key.to_string()))
        }

        /// Creates a new SteamAPI Client that sends `access_token=` instead of `key=`.
        pub fn with_access_token<T: ToString>(access_token: T) -> SteamAPI {
            Self::with_client(Default::default(), Credential::AccessToken(access_token.to_string()))
        }

        /// Creates a SteamAPI Client on top of an existing HTTP client.
        ///
        /// Cloning a `reqwest` client is cheap and shares its connection pool, so this is the way to build a
        /// `SteamAPI` per call, e.g. with a renewed access token.
        pub fn with_client(client: reqwest::blocking::Client, credential: Credential) -> SteamAPI {
            Self { client, credential }
        }

        pub fn set_api_key<T: ToString>(&mut self, api_key: T) {
            self.credential = Credential::ApiKey(api_key.to_string());
        }

        pub fn set_credential(&mut self, credential: Credential) {
            self.credential = credential;
        }

        pub fn get(&self) -> GetQueryBuilder {
            self.into()
        }
    }

    new_type!(GetQueryBuilder);
    new_type!(PostQueryBuilder);

    from!(@GetQueryBuilder => GET);
    from!(@PostQueryBuilder => POST);
}

#[cfg(feature = "async")]
mod async_client {
    use async_trait::async_trait;
    use serde::de::DeserializeOwned;

    use crate::{Credential, Result};

    #[async_trait]
    /// Requests the endpoint and returns the raw response.
    pub trait Executor {
        async fn execute(self) -> Result<String>;
    }

    #[async_trait]
    /// Requests the endpoint and returns the proper deserialized response.
    /// Response types are exposed on `tappet::response_types`.
    pub trait ExecutorResponse<T: DeserializeOwned> {
        async fn execute_with_response(self) -> Result<T>;
    }

    pub struct SteamAPI {
        pub(crate) client: reqwest::Client,
        /// Mandatory for some operations
        pub(crate) credential: Credential,
    }

    impl std::fmt::Debug for SteamAPI {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("SteamAPI")
                .field("client", &self.client)
                .field("credential", &self.credential)
                .finish()
        }
    }

    impl SteamAPI {
        /// Creates a new SteamAPI Client with an API Key.
        pub fn new<T: ToString>(api_key: T) -> SteamAPI {
            Self::with_client(Default::default(), Credential::ApiKey(api_key.to_string()))
        }

        /// Creates a new SteamAPI Client that sends `access_token=` instead of `key=`.
        pub fn with_access_token<T: ToString>(access_token: T) -> SteamAPI {
            Self::with_client(Default::default(), Credential::AccessToken(access_token.to_string()))
        }

        /// Creates a SteamAPI Client on top of an existing HTTP client.
        ///
        /// Cloning a `reqwest` client is cheap and shares its connection pool, so this is the way to build a
        /// `SteamAPI` per call, e.g. with a renewed access token.
        pub fn with_client(client: reqwest::Client, credential: Credential) -> SteamAPI {
            Self { client, credential }
        }

        pub fn set_api_key<T: ToString>(&mut self, api_key: T) {
            self.credential = Credential::ApiKey(api_key.to_string());
        }

        pub fn set_credential(&mut self, credential: Credential) {
            self.credential = credential;
        }

        pub fn get(&self) -> GetQueryBuilder {
            self.into()
        }

        pub fn post(&self) -> PostQueryBuilder {
            self.into()
        }
    }

    new_type!(GetQueryBuilder);
    new_type!(PostQueryBuilder);

    from!(@GetQueryBuilder => GET);
    from!(@PostQueryBuilder => POST);
}
