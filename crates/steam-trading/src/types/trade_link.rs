use std::convert::TryFrom;
use std::str::FromStr;

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use steamid_parser::SteamID;

use crate::errors::TradelinkError;

const CANONICAL_PREFIX: &str = "https://steamcommunity.com/tradeoffer/new/?partner=";

lazy_static! {
    /// The whole link, as Steam generates it, maybe followed by more parameters or a fragment, as when it was copied
    /// from a browser. Anchored, so a link that only contains a trade link doesn't pass (e.g. as a parameter of
    /// another site's link), and ASCII only: `\d` and `\w` would accept any Unicode digit or letter.
    static ref TRADE_LINK_REGEX: Regex = Regex::new(
        r#"^https://steamcommunity\.com/tradeoffer/new/\?partner=(?P<partner>[0-9]+)&token=(?P<token>[A-Za-z0-9_-]+)(?P<rest>[&#]\S*)?$"#
    )
    .unwrap();
}

/// The partner's account id and the token of `trade_link`, surrounding whitespace ignored.
fn parse(trade_link: &str) -> Result<(u32, &str), TradelinkError> {
    let captures = TRADE_LINK_REGEX
        .captures(trade_link.trim())
        .ok_or(TradelinkError::Invalid)?;

    // A second `partner` or `token` makes the link ambiguous: Steam may read the other one. Names that are
    // percent-encoded (`%70artner`) or arrays (`partner[]`) could be one too; no trade link has them.
    let rest = captures.name("rest").map_or("", |rest| rest.as_str());
    let query = rest.split('#').next().unwrap_or_default();
    let repeated = query
        .split('&')
        .map(|parameter| parameter.split('=').next().unwrap_or_default())
        .any(|name| {
            name.contains('%')
                || ["partner", "token"]
                    .iter()
                    .any(|ours| name.split('[').next() == Some(*ours))
        });
    if repeated {
        return Err(TradelinkError::Invalid);
    }

    let partner = captures
        .name("partner")
        .and_then(|partner| u32::from_str(partner.as_str()).ok())
        .ok_or(TradelinkError::Invalid)?;
    let token = captures.name("token").ok_or(TradelinkError::Invalid)?.as_str();

    Ok((partner, token))
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
/// A tradelink generated from Steam Trade Offer page.
///
/// A link is accepted as Steam generates it (`https://steamcommunity.com/tradeoffer/new/?partner=N&token=T`), with
/// surrounding whitespace, and with more parameters or a fragment after the token. Anything else around it, other
/// text included, makes it invalid. `link` is always the canonical form, without the extras.
///
/// # Notes
/// A tradelink does not need a token if is from a friend.
/// But Steam always generates one with a token, and it is also kind of pointless to remove the token, so they are standardized to have it.
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
    /// Whether the link is valid and its partner is `steamid`.
    pub fn validate_with_steam64(trade_link: &str, steamid: u64) -> Result<bool, TradelinkError> {
        let (partner, _) = parse(trade_link)?;
        Ok(SteamID::from_steam3(partner, None, None) == SteamID::from_steam64(steamid))
    }

    pub fn validate(trade_link: &str) -> Result<(), TradelinkError> {
        parse(trade_link).map(|_| ())
    }

    /// `link` is the canonical form of `trade_link`: for a link as Steam generates it, the link itself.
    pub fn new(trade_link: String) -> Result<Self, TradelinkError> {
        let (partner, token) = parse(&trade_link)?;

        Ok(Self {
            partner_id: SteamID::from_steam3(partner, None, None),
            token: token.to_string(),
            link: format!("{}{}&token={}", CANONICAL_PREFIX, partner, token),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_google() -> &'static str {
        "http://google.com"
    }

    /// Well formed, but the token is not this account's: only Steam can tell.
    fn get_wrong_token_tradelink() -> &'static str {
        "https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vuaidfos"
    }

    fn get_valid_tradelink() -> &'static str {
        "https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN"
    }

    fn valid_steamid() -> u64 {
        76561197984835396
    }

    fn invalid_steamid() -> u64 {
        76561197984835395
    }

    #[test]
    fn validated_with_steamid() {
        let result = Tradelink::validate_with_steam64(get_valid_tradelink(), valid_steamid());
        assert_eq!(result, Ok(true))
    }

    #[test]
    fn validated_with_invalid_steamid() {
        let result = Tradelink::validate_with_steam64(get_valid_tradelink(), invalid_steamid());
        assert_eq!(result, Ok(false))
    }

    #[test]
    fn valid_tradelink() {
        let result = Tradelink::validate(get_valid_tradelink());
        assert_eq!(result, Ok(()))
    }

    #[test]
    fn wrong_token_is_not_detected() {
        let result = Tradelink::validate(get_wrong_token_tradelink());
        assert_eq!(result, Ok(()))
    }

    #[test]
    fn invalid_tradelink_missing_token() {
        let result = Tradelink::validate("https://steamcommunity.com/tradeoffer/new/?partner=24569668");
        assert_eq!(result, Err(TradelinkError::Invalid))
    }

    #[test]
    fn valid_tradelinks() {
        for link in &[
            get_valid_tradelink(),
            "https://steamcommunity.com/tradeoffer/new/?partner=79925588&token=Ob27qXzn",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=a-b_c",
            "https://steamcommunity.com/tradeoffer/new/?partner=4294967295&token=AbCdEf09",
            " https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN\n",
        ] {
            assert_eq!(Tradelink::validate(link), Ok(()), "{}", link);
            assert!(Tradelink::new(link.to_string()).is_ok(), "{}", link);
        }
    }

    #[test]
    fn new_trims_the_link() {
        let tradelink = Tradelink::new(
            "\t https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN \r\n".to_string(),
        )
        .unwrap();
        assert_eq!(tradelink.link, get_valid_tradelink());
        assert_eq!(tradelink.token, "vnFisKdN");
        assert_eq!(tradelink.partner_id.to_steam64(), valid_steamid());
    }

    #[test]
    fn extra_parameters_and_fragments_are_dropped() {
        for link in &[
            "https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN&utm_source=x",
            "https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN&a=1&b",
            "https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN&redirect=https://evil.tld",
            "https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN#trade",
            "https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN&a=1#x&partner=2",
            "https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN&",
            " https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN&a=é\n",
        ] {
            assert_eq!(Tradelink::validate(link), Ok(()), "{}", link);
            assert_eq!(
                Tradelink::validate_with_steam64(link, valid_steamid()),
                Ok(true),
                "{}",
                link
            );
            let tradelink = Tradelink::new(link.to_string()).unwrap();
            assert_eq!(tradelink.link, get_valid_tradelink(), "{}", link);
            assert_eq!(tradelink.token, "vnFisKdN");
            assert_eq!(tradelink.partner_id.to_steam64(), valid_steamid());
        }
    }

    #[test]
    fn evil_tradelinks() {
        for link in &[
            "https://evil.tld/?r=https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh",
            "https://evil.tld/?r=https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh&x=1",
            "https://evil.tld/#https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh.evil.tld",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh/evil",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh&partner=2",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh&token=other",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh&a=1&token=other#x",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh&%70artner=2",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh&t%6Fken=x",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh&partner[]=2",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcd<script>",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=tökénéé",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abc\u{0430}def",
            "https://steamcommunity.com/tradeoffer/new/?partner=\u{0661}\u{0662}&token=abcdefgh",
            "https://steamcommunity.com/tradeoffer/new/?partner=&token=abcdefgh",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=",
            "https://steamcommunity.com/tradeoffer/new/?token=abcdefgh&partner=1",
            "http://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh",
            "https://steamcommunity.com.evil.tld/tradeoffer/new/?partner=1&token=abcdefgh",
            "https://steamcommunityXcom/tradeoffer/new/?partner=1&token=abcdefgh",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abc defgh",
            // surrounding text: the caller extracts the link
            "my trade link: https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh thanks",
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh&a=1 thanks",
            "<https://steamcommunity.com/tradeoffer/new/?partner=1&token=abcdefgh>",
        ] {
            assert_eq!(Tradelink::validate(link), Err(TradelinkError::Invalid), "{}", link);
            assert_eq!(
                Tradelink::new(link.to_string()),
                Err(TradelinkError::Invalid),
                "{}",
                link
            );
            assert_eq!(
                Tradelink::validate_with_steam64(link, valid_steamid()),
                Err(TradelinkError::Invalid),
                "{}",
                link
            );
        }
    }

    #[test]
    fn partner_above_u32_is_invalid() {
        let link = "https://steamcommunity.com/tradeoffer/new/?partner=4294967296&token=abcdefgh";
        assert_eq!(Tradelink::new(link.to_string()), Err(TradelinkError::Invalid));
        assert_eq!(
            Tradelink::validate_with_steam64(link, valid_steamid()),
            Err(TradelinkError::Invalid)
        );
        let link = "https://steamcommunity.com/tradeoffer/new/?partner=99999999999999999999999&token=abcdefgh";
        assert_eq!(
            Tradelink::validate_with_steam64(link, valid_steamid()),
            Err(TradelinkError::Invalid)
        );
    }

    #[test]
    fn invalid_tradelink() {
        let result = Tradelink::validate(get_google());
        assert_eq!(result, Err(TradelinkError::Invalid))
    }
}
