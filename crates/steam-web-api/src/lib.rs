#![allow(non_snake_case)]
#![allow(unused_imports)]

#[macro_use]
mod macros;

pub mod endpoints;
mod helpers;
pub mod response_types;

#[cfg(feature = "async")]
use async_trait::async_trait;

#[cfg(feature = "blocking")]
pub mod blocking {
    use serde::de::DeserializeOwned;

    pub trait Executor {
        /// Requests the endpoint and returns the raw response.
        fn execute(self) -> reqwest::Result<reqwest::blocking::Response>;
    }

    pub trait ExecutorResponse<T: DeserializeOwned> {
        /// Requests the endpoint and returns the proper deserialized response.
        /// Response types are exposed on `steam_web_api::response_types`.
        fn execute_with_response(self) -> reqwest::Result<T>;
    }

    #[derive(Debug)]
    pub struct SteamAPI {
        pub(crate) client: reqwest::blocking::Client,
        /// Mandatory for some operations
        pub(crate) key: String,
    }

    impl SteamAPI {
        /// Creates a new SteamAPI Client with an API Key.
        pub fn new<T: ToString>(api_key: T) -> SteamAPI {
            Self {
                client: Default::default(),
                key: api_key.to_string(),
            }
        }

        pub fn get(&self) -> GetQueryBuilder {
            self.into()
        }
    }

    new_type!(GetQueryBuilder);
    from!(@GetQueryBuilder => GET);
}

#[cfg(feature = "async")]
mod async_client {

    use async_trait::async_trait;
    use serde::de::DeserializeOwned;

    #[async_trait]
    pub trait Executor {
        async fn execute(self) -> reqwest::Result<reqwest::Response>;
    }

    #[async_trait]
    pub trait ExecutorResponse<T: DeserializeOwned> {
        /// Requests the endpoint and returns the proper deserialized response.
        /// Response types are exposed on `steam_web_api::response_types`.
        async fn execute_with_response(self) -> reqwest::Result<T>;
    }

    #[derive(Debug)]
    pub struct SteamAPI {
        pub(crate) client: reqwest::Client,
        /// Mandatory for some operations
        pub(crate) key: String,
    }

    impl SteamAPI {
        /// Creates a new SteamAPI Client with an API Key.
        pub fn new<T: ToString>(api_key: T) -> SteamAPI {
            Self {
                client: Default::default(),
                key: api_key.to_string(),
            }
        }

        pub fn get(&self) -> GetQueryBuilder {
            self.into()
        }
    }

    new_type!(GetQueryBuilder);
    from!(@GetQueryBuilder => GET);
}

#[cfg(feature = "async")]
pub use async_client::*;
