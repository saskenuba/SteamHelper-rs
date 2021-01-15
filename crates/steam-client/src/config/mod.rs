//! Application configuration and configuration parameter retrieval.
//!
//!

use std::path::PathBuf;

const STEAM_USER: &str = "STEAM_USER";
const STEAM_PASS: &str = "STEAM_PASS";


#[derive(Debug)]
/// Huge explanation about SteamConfiguration
pub struct SteamConfiguration {
    username: String,
    password: String,
    api_key: Option<String>,
}

impl SteamConfiguration {
    /// Builds a basic configuration file
    pub fn builder() -> Self {
        Self {
            username: "".to_string(),
            password: "".to_string(),
            api_key: None,
        }
    }

    pub fn set_apikey<T: Into<String>>(&mut self, api_key: T) {
        self.api_key = Some(api_key.into())
    }

    /// this requires a file in the following format
    /// ``` [details]
    /// user = xxxx
    /// pass = xxxx
    ///```
    fn from_file(_path: PathBuf) {}

    /// this requires the environment variables
    /// STEAM_USER and STEAM_PASS
    fn from_env() {}
}
