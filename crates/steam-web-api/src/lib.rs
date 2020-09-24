#![allow(non_snake_case)]
#![allow(unused_imports)]

use serde::de::DeserializeOwned;

#[macro_use]
mod macros;

mod helpers;
pub mod endpoints;
pub mod response_types;

// #[async_trait]
// trait ExecutorAsync {
//     async fn execute(self) -> reqwest::Result<reqwest::Response>;
// }

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
    client: reqwest::blocking::Client,
    /// Mandatory for some operations
    key: String,
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