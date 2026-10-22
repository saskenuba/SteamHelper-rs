use std::fmt;
use std::str::FromStr;

use lazy_static::lazy_static;
use regex::Regex;
use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use steamid_parser::SteamID;

use crate::errors::TradeLinkError;

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
fn parse(trade_link: &str) -> Result<(u32, &str), TradeLinkError> {
    let captures = TRADE_LINK_REGEX
        .captures(trade_link.trim())
        .ok_or(TradeLinkError::Invalid)?;

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
        return Err(TradeLinkError::Invalid);
    }

    let partner = captures
        .name("partner")
        .and_then(|partner| u32::from_str(partner.as_str()).ok())
        .ok_or(TradeLinkError::Invalid)?;
    let token = captures.name("token").ok_or(TradeLinkError::Invalid)?.as_str();

    Ok((partner, token))
}

/// A trade link, as Steam generates it on the trade offer privacy page: whom to send an offer to, and the token that
/// lets anyone send them one.
///
/// A link is accepted as Steam generates it (`https://steamcommunity.com/tradeoffer/new/?partner=N&token=T`), with
/// surrounding whitespace, and with more parameters or a fragment after the token. Anything else around it, other
/// text included, makes it invalid. [`TradeLink::url`] is always the canonical form, without the extras.
///
/// Shown as its URL; in JSON (serde) the URL string, parsed again when read.
///
/// # Notes
/// A friend's link needs no token, but Steam always generates one with it, so a link without one is invalid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TradeLink {
    url: String,
    partner: SteamID,
    token: String,
}

impl TradeLink {
    /// Reads `link`. Steam can tell whether its token is still the partner's; this can't.
    pub fn parse(link: &str) -> Result<TradeLink, TradeLinkError> {
        let (partner, token) = parse(link)?;

        Ok(Self {
            partner: SteamID::from_steam3(partner, None, None),
            token: token.to_string(),
            url: format!("{}{}&token={}", CANONICAL_PREFIX, partner, token),
        })
    }

    /// Whom offers to this link go to.
    pub fn partner(&self) -> SteamID {
        self.partner
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// The link in its canonical form.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Whether the link is `steam_id`'s.
    pub fn belongs_to(&self, steam_id: SteamID) -> bool {
        self.partner == steam_id
    }
}

impl FromStr for TradeLink {
    type Err = TradeLinkError;

    fn from_str(link: &str) -> Result<Self, Self::Err> {
        Self::parse(link)
    }
}

impl fmt::Display for TradeLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.url)
    }
}

impl Serialize for TradeLink {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.url)
    }
}

impl<'de> Deserialize<'de> for TradeLink {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let link = String::deserialize(deserializer)?;
        Self::parse(&link).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `link` reads as a trade link.
    fn validate(link: &str) -> Result<(), TradeLinkError> {
        TradeLink::parse(link).map(|_| ())
    }

    /// Whether `link` reads as a trade link, and is `steam64`'s.
    fn validate_with_steam64(link: &str, steam64: u64) -> Result<bool, TradeLinkError> {
        TradeLink::parse(link).map(|link| link.belongs_to(SteamID::from_steam64(steam64)))
    }

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
        let result = validate_with_steam64(get_valid_tradelink(), valid_steamid());
        assert_eq!(result, Ok(true))
    }

    #[test]
    fn validated_with_invalid_steamid() {
        let result = validate_with_steam64(get_valid_tradelink(), invalid_steamid());
        assert_eq!(result, Ok(false))
    }

    #[test]
    fn valid_tradelink() {
        let result = validate(get_valid_tradelink());
        assert_eq!(result, Ok(()))
    }

    #[test]
    fn wrong_token_is_not_detected() {
        let result = validate(get_wrong_token_tradelink());
        assert_eq!(result, Ok(()))
    }

    #[test]
    fn invalid_tradelink_missing_token() {
        let result = validate("https://steamcommunity.com/tradeoffer/new/?partner=24569668");
        assert_eq!(result, Err(TradeLinkError::Invalid))
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
            assert_eq!(validate(link), Ok(()), "{}", link);
            assert!(TradeLink::parse(link).is_ok(), "{}", link);
        }
    }

    #[test]
    fn new_trims_the_link() {
        let tradelink =
            TradeLink::parse("\t https://steamcommunity.com/tradeoffer/new/?partner=24569668&token=vnFisKdN \r\n")
                .unwrap();
        assert_eq!(tradelink.url(), get_valid_tradelink());
        assert_eq!(tradelink.token(), "vnFisKdN");
        assert_eq!(tradelink.partner().to_steam64(), valid_steamid());
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
            assert_eq!(validate(link), Ok(()), "{}", link);
            assert_eq!(validate_with_steam64(link, valid_steamid()), Ok(true), "{}", link);
            let tradelink = TradeLink::parse(link).unwrap();
            assert_eq!(tradelink.url(), get_valid_tradelink(), "{}", link);
            assert_eq!(tradelink.token(), "vnFisKdN");
            assert_eq!(tradelink.partner().to_steam64(), valid_steamid());
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
            assert_eq!(validate(link), Err(TradeLinkError::Invalid), "{}", link);
            assert_eq!(TradeLink::parse(link), Err(TradeLinkError::Invalid), "{}", link);
            assert_eq!(
                validate_with_steam64(link, valid_steamid()),
                Err(TradeLinkError::Invalid),
                "{}",
                link
            );
        }
    }

    #[test]
    fn partner_above_u32_is_invalid() {
        let link = "https://steamcommunity.com/tradeoffer/new/?partner=4294967296&token=abcdefgh";
        assert_eq!(TradeLink::parse(link), Err(TradeLinkError::Invalid));
        assert_eq!(
            validate_with_steam64(link, valid_steamid()),
            Err(TradeLinkError::Invalid)
        );
        let link = "https://steamcommunity.com/tradeoffer/new/?partner=99999999999999999999999&token=abcdefgh";
        assert_eq!(
            validate_with_steam64(link, valid_steamid()),
            Err(TradeLinkError::Invalid)
        );
    }

    #[test]
    fn invalid_tradelink() {
        let result = validate(get_google());
        assert_eq!(result, Err(TradeLinkError::Invalid))
    }

    #[test]
    fn url_string_both_ways() {
        let link = TradeLink::parse(get_valid_tradelink()).unwrap();
        assert_eq!(link.to_string(), get_valid_tradelink());
        assert_eq!(get_valid_tradelink().parse::<TradeLink>().unwrap(), link);

        let json = serde_json::to_string(&link).unwrap();
        assert_eq!(json, format!("\"{}\"", get_valid_tradelink()));
        assert_eq!(serde_json::from_str::<TradeLink>(&json).unwrap(), link);
        // read as any link: the canonical form
        let extras = format!("\"{}#trade\"", get_valid_tradelink());
        assert_eq!(serde_json::from_str::<TradeLink>(&extras).unwrap(), link);
        assert!(serde_json::from_str::<TradeLink>("\"http://google.com\"").is_err());
    }
}
