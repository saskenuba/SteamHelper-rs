//! Application configuration and configuration parameter retrieval.
//!
//!

use std::path::PathBuf;

const STEAM_USER: &str = "STEAM_USER";
const STEAM_PASS: &str = "STEAM_PASS";


#[derive(Debug)]
pub struct SteamConfiguration {
    pub username: String,
    pub password: String,
}

impl SteamConfiguration {
    /// this requires a file in the following format
    /// ```[details]```
    /// ```user = xxxx```
    /// ```pass = xxxx```
    fn from_file(_path: PathBuf) {}

    /// this requires the environment variables
    /// STEAM_USER and STEAM_PASS
    fn from_env() {}
}
