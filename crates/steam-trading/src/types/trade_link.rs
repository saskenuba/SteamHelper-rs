use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::errors::TradelinkError;
use std::convert::TryFrom;
use std::str::FromStr;
use steamid_parser::SteamID;

lazy_static! {
    static ref TRADE_LINK_REGEX: Regex = Regex::new(
        r#"https://steamcommunity\.com/tradeoffer/new/\?partner=(?P<partner>[\d]+)&token=(?P<token>[\w-]+)"#
    )
    .unwrap();
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Tradelink {
    pub link: String,
    pub partner_id: SteamID,
    pub token: String,
}

impl TryFrom<String> for Tradelink {
    type Error = TradelinkError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Tradelink {
    pub fn validate_with_steam64(trade_link: &str, steamid: u64) -> Result<bool, TradelinkError> {
        let captures = TRADE_LINK_REGEX.captures(&trade_link);

        if captures.is_none() {
            return Err(TradelinkError::Invalid);
        }

        let captures = captures.unwrap();
        let steamid = SteamID::from_steam64(steamid);

        let partner_id = captures
            .name("partner")
            .map(|partner_id_raw| u32::from_str(partner_id_raw.as_str()).unwrap())
            .map(|partner_id| SteamID::from_steam3(partner_id, None, None))
            .map(|steamid_from_tradelink| steamid_from_tradelink == steamid)
            .unwrap();

        if partner_id {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // Optionally takes a Steam64, and check if the partner_id matches with it, to be indeed the same account.
    pub fn validate(trade_link: &str) -> Result<(), TradelinkError> {
        let captures = TRADE_LINK_REGEX.captures(&trade_link);

        if captures.is_none() {
            return Err(TradelinkError::Invalid);
        }

        Ok(())
    }

    pub fn new(trade_link: String) -> Result<Self, TradelinkError> {
        Self::validate(&*trade_link)?;

        let captures = TRADE_LINK_REGEX.captures(&trade_link).unwrap();
        let partner_id = captures
            .name("partner")
            .map(|partner_id_raw| u32::from_str(partner_id_raw.as_str()).unwrap())
            .map(|partner_id| SteamID::from_steam3(partner_id, None, None))
            .unwrap();
        let token = captures.name("token").unwrap().as_str();

        Ok(Self {
            partner_id,
            token: token.to_string(),
            link: trade_link,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_google() -> &'static str {
        "http://google.com"
    }

    fn get_invalid_tradelink_no_token() -> &'static str {
        "https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=11223aa"
    }

    fn get_valid_tradelink() -> &'static str {
        "https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN"
    }

    fn valid_steamid() -> u64 {
        76561197984835396
    }

    #[test]
    fn validated_with_steamid() {
        let result = Tradelink::validate_with_steam64(get_valid_tradelink(), valid_steamid());
        assert_eq!(result, Ok(true))
    }

    #[test]
    fn valid_tradelink() {
        let result = Tradelink::validate(get_valid_tradelink());
        assert_eq!(result, Ok(()))
    }

    #[test]
    fn invalid_tradelink_missing_token() {
        let result = Tradelink::validate(get_invalid_tradelink_no_token());
        assert_eq!(result, Err(TradelinkError::Invalid))
    }

    #[test]
    fn invalid_tradelink() {
        let result = Tradelink::validate(get_google());
        assert_eq!(result, Err(TradelinkError::Invalid))
    }
}
