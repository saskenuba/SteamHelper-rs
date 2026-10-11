use std::str::FromStr;

#[cfg(feature = "serialize")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use steam_language_gen::generated::enums::{EAccountType, EUniverse};

// Bit layout of a Steam64 ID, from most to least significant:
// universe (8) | account type (4) | account instance (20) | account number (31) | account id (1)
const UNIVERSE_SHIFT: u32 = 56;
const ACCOUNT_TYPE_SHIFT: u32 = 52;
const ACCOUNT_TYPE_MASK: u64 = 0xF;
const ACCOUNT_INSTANCE_SHIFT: u32 = 32;
const ACCOUNT_INSTANCE_MASK: u64 = 0xF_FFFF;
const UNIVERSE_MASK: u64 = 0xFF;
const STEAM3_MASK: u64 = 0xFFFF_FFFF;
// Steam2 account numbers (Z) are 31 bits.
const STEAM2_ACCOUNT_NUMBER_MAX: u32 = 0x7FFF_FFFF;
// The highest universe bit is not kept when converting back to Steam64. No real universe sets it.
const STEAM64_MASK: u64 = !(1 << 63);

/// Error returned when a string can't be parsed as a [`SteamID`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseSteamIDError {
    /// The string is not exactly one Steam64, Steam3 or Steam2 ID.
    #[error("not a Steam64, Steam3 or Steam2 id")]
    UnknownFormat,
    /// The Steam3 account type letter is not supported.
    #[error("unsupported Steam3 account type `{0}`")]
    UnsupportedAccountType(char),
    /// The account doesn't fit in a SteamID.
    #[error("account `{0}` is out of range")]
    AccountOutOfRange(String),
}

struct AccountType(EAccountType);

impl AccountType {
    fn new(identifier: u8) -> Option<Self> {
        let kind = match identifier {
            b'A' => EAccountType::AnonGameServer,
            b'G' => EAccountType::GameServer,
            b'M' => EAccountType::Multiseat,
            b'P' => EAccountType::Pending,
            b'C' => EAccountType::ContentServer,
            b'g' => EAccountType::Clan,
            b'T' => EAccountType::Chat,
            b'I' => EAccountType::Invalid,
            b'U' => EAccountType::Individual,
            b'a' => EAccountType::AnonUser,
            _ => return None,
        };
        Some(Self(kind))
    }
}

fn universe_from_value(value: u64) -> Option<EUniverse> {
    let universe = match value {
        0 => EUniverse::Invalid,
        1 => EUniverse::Public,
        2 => EUniverse::Beta,
        3 => EUniverse::Internal,
        4 => EUniverse::Dev,
        _ => return None,
    };
    Some(universe)
}

fn universe_from_digit(digit: u8) -> Option<EUniverse> {
    universe_from_value(digit.checked_sub(b'0')? as u64)
}

fn account_type_from_value(value: u64) -> Option<EAccountType> {
    let account_type = match value {
        0 => EAccountType::Invalid,
        1 => EAccountType::Individual,
        2 => EAccountType::Multiseat,
        3 => EAccountType::GameServer,
        4 => EAccountType::AnonGameServer,
        5 => EAccountType::Pending,
        6 => EAccountType::ContentServer,
        7 => EAccountType::Clan,
        8 => EAccountType::Chat,
        9 => EAccountType::ConsoleUser,
        10 => EAccountType::AnonUser,
        _ => return None,
    };
    Some(account_type)
}

/// A SteamID matched in a string, before its values are checked.
enum Candidate<'a> {
    /// `[T:U:D]`: account type letter, universe digit and account digits.
    Steam3 {
        account_type: u8,
        universe: u8,
        account: &'a str,
    },
    /// `STEAM_X:Y:Z`: universe digit, Y digit and Z digits.
    Steam2 { universe: u8, y: u8, z: &'a str },
    /// The digit 7 followed by 16 digits.
    Steam64(&'a str),
}

impl Candidate<'_> {
    fn into_steamid(self) -> Result<SteamID, ParseSteamIDError> {
        match self {
            Candidate::Steam3 {
                account_type,
                universe,
                account,
            } => {
                // TODO - match instance
                let account_type = AccountType::new(account_type)
                    .ok_or(ParseSteamIDError::UnsupportedAccountType(account_type as char))?;
                let steam3 = account
                    .parse()
                    .map_err(|_| ParseSteamIDError::AccountOutOfRange(account.to_owned()))?;
                Ok(SteamID::from_steam3(
                    steam3,
                    // the matcher only accepts universes from 0 to 4
                    Some(universe_from_digit(universe).unwrap()),
                    Some(account_type.0),
                ))
            }
            Candidate::Steam2 { universe, y, z } => {
                let z = z
                    .parse::<u32>()
                    .ok()
                    .filter(|z| *z <= STEAM2_ACCOUNT_NUMBER_MAX)
                    .ok_or_else(|| ParseSteamIDError::AccountOutOfRange(z.to_owned()))?;
                let universe = match universe {
                    b'0' => EUniverse::Public,
                    // the matcher only accepts universes from 0 to 4
                    digit => universe_from_digit(digit).unwrap(),
                };
                Ok(SteamID::from_steam3(z * 2 + (y - b'0') as u32, Some(universe), None))
            }
            // 17 digits always fit in a u64
            Candidate::Steam64(steam64) => Ok(SteamID::from_steam64(steam64.parse().unwrap())),
        }
    }
}

/// Matches a format at the start of the input, returning the candidate and its length.
type Matcher = fn(&[u8]) -> Option<(Candidate<'_>, usize)>;

/// Formats in order of preference.
const MATCHERS: [Matcher; 3] = [match_steam3, match_steam64, match_steam2];

/// Returns the ascii digits at the start of the input.
fn leading_digits(input: &[u8]) -> &str {
    let len = input.iter().take_while(|c| c.is_ascii_digit()).count();
    // only ascii digits were taken, so this is valid utf8
    std::str::from_utf8(&input[..len]).unwrap()
}

/// Matches a Steam3 ID: `[T:U:D]`, where T is one of `AGMPCgcLTIUai`, U is a digit from 0 to 4 and
/// D is one or more digits.
fn match_steam3(input: &[u8]) -> Option<(Candidate<'_>, usize)> {
    match input {
        [b'[', account_type, b':', universe @ b'0'..=b'4', b':', rest @ ..]
            if b"AGMPCgcLTIUai".contains(account_type) =>
        {
            let account = leading_digits(rest);
            if account.is_empty() || rest.get(account.len()) != Some(&b']') {
                return None;
            }
            let candidate = Candidate::Steam3 {
                account_type: *account_type,
                universe: *universe,
                account,
            };
            Some((candidate, 6 + account.len()))
        }
        _ => None,
    }
}

/// Matches a Steam2 ID: `STEAM_X:Y:Z`, where X is a digit from 0 to 4, Y is either 0 or 1 and Z is
/// one or more digits.
fn match_steam2(input: &[u8]) -> Option<(Candidate<'_>, usize)> {
    match input {
        [b'S', b'T', b'E', b'A', b'M', b'_', universe @ b'0'..=b'4', b':', y @ b'0'..=b'1', b':', rest @ ..] => {
            let z = leading_digits(rest);
            if z.is_empty() {
                return None;
            }
            let candidate = Candidate::Steam2 {
                universe: *universe,
                y: *y,
                z,
            };
            Some((candidate, 10 + z.len()))
        }
        _ => None,
    }
}

/// Matches a Steam64 ID: the digit 7 followed by 16 digits.
fn match_steam64(input: &[u8]) -> Option<(Candidate<'_>, usize)> {
    let steam64 = input.get(..17)?;
    if steam64[0] != b'7' || !steam64.iter().all(u8::is_ascii_digit) {
        return None;
    }
    // only ascii digits were matched, so this is valid utf8
    let steam64 = std::str::from_utf8(steam64).unwrap();
    Some((Candidate::Steam64(steam64), 17))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Let X, Y and Z constants be defined by the SteamID: STEAM_X:Y:Z.
pub struct SteamID(u64);

/// Reference: <https://developer.valvesoftware.com/wiki/SteamID>
impl SteamID {
    /// Using the formula W=Z*2+Y, a SteamID can be converted to Steam3.
    /// Source: `https://steamcommunity.com/path/[letter:1:W]`
    pub fn to_steam3(self) -> u64 {
        self.0 & STEAM3_MASK
    }

    /// Formats as Steam2: `STEAM_X:Y:Z`, where X is the universe, Y the lowest bit of the account id and
    /// Z the remaining bits, so that Steam3 is Z*2+Y.
    ///
    /// The public universe is rendered as 0, its classic Steam2 form. Steam2 only describes individual
    /// accounts, so the account type and instance are not kept.
    pub fn to_steam2(self) -> String {
        let universe = match (self.0 >> UNIVERSE_SHIFT) & UNIVERSE_MASK {
            universe if universe == EUniverse::Public as u64 => 0,
            universe => universe,
        };
        let steam3 = self.to_steam3();
        format!("STEAM_{}:{}:{}", universe, steam3 & 1, steam3 >> 1)
    }

    pub fn to_steam64(self) -> u64 {
        self.0 & STEAM64_MASK
    }

    /// Returns the account id, also known as Steam32: the W in Steam3 `[U:1:W]`.
    ///
    /// Same value as [`SteamID::to_steam3`], as a `u32`.
    pub fn account_id(self) -> u32 {
        (self.0 & STEAM3_MASK) as u32
    }

    /// Returns the account instance. Desktop users have instance 1.
    pub fn instance(self) -> u32 {
        ((self.0 >> ACCOUNT_INSTANCE_SHIFT) & ACCOUNT_INSTANCE_MASK) as u32
    }

    /// Returns the account type, or `None` if it's not a known account type.
    pub fn account_type(self) -> Option<EAccountType> {
        account_type_from_value((self.0 >> ACCOUNT_TYPE_SHIFT) & ACCOUNT_TYPE_MASK)
    }

    /// Returns the universe, or `None` if it's not a known universe.
    pub fn universe(self) -> Option<EUniverse> {
        universe_from_value((self.0 >> UNIVERSE_SHIFT) & UNIVERSE_MASK)
    }

    /// Creates a new SteamID from the Steam3 format.
    /// Defaults to Public universe, and Individual account.
    /// You can use the parse utility function.
    pub fn from_steam3(steam3: u32, universe: Option<EUniverse>, account_type: Option<EAccountType>) -> Self {
        let universe = universe.unwrap_or(EUniverse::Public) as u64;
        let account_type = account_type.unwrap_or(EAccountType::Individual) as u64;
        let instance = 1u64;

        Self(
            universe << UNIVERSE_SHIFT
                | (account_type & ACCOUNT_TYPE_MASK) << ACCOUNT_TYPE_SHIFT
                | instance << ACCOUNT_INSTANCE_SHIFT
                | steam3 as u64,
        )
    }

    /// Creates a new SteamID from the Steam64 format.
    pub fn from_steam64(steam64: u64) -> Self {
        Self(steam64)
    }

    /// Finds the first SteamID in a text, such as a profile url or a chat message.
    ///
    /// The following formats are found:
    ///
    /// Steam64: digit 7 + 16 digits
    ///
    /// Steam3: `[T:U:D]` where T: The account type, U: The account universe, D: Account number,
    ///
    /// Steam2: `STEAM_X:Y:Z` where X: The account universe, with 0 meaning public, Y: Lowest bit of the account number,
    /// Z: Remaining bits of the account number. Always an individual account.
    ///
    /// When more than one format is present, Steam3 is preferred, then Steam64, then Steam2. Matches that can't be
    /// converted, such as an account that is out of range, are skipped.
    ///
    /// To parse a string that must be exactly one SteamID, use [`str::parse`] instead.
    pub fn find(text: &str) -> Option<Self> {
        let input = text.as_bytes();
        MATCHERS.iter().find_map(|matcher| {
            (0..input.len())
                .find_map(|start| matcher(&input[start..]).and_then(|(candidate, _)| candidate.into_steamid().ok()))
        })
    }

    /// Finds the first SteamID in a text.
    #[deprecated(
        since = "0.3.0",
        note = "use `SteamID::find`, or `str::parse` for a string that must be exactly one SteamID"
    )]
    pub fn parse(steamid: &str) -> Option<Self> {
        Self::find(steamid)
    }
}

/// Parses a string that is exactly one Steam64, Steam3 or Steam2 ID, in the formats described in
/// [`SteamID::find`].
impl FromStr for SteamID {
    type Err = ParseSteamIDError;

    fn from_str(steamid: &str) -> Result<Self, Self::Err> {
        let input = steamid.as_bytes();
        let (candidate, _) = MATCHERS
            .iter()
            .find_map(|matcher| matcher(input).filter(|(_, len)| *len == input.len()))
            .ok_or(ParseSteamIDError::UnknownFormat)?;
        candidate.into_steamid()
    }
}

#[cfg(feature = "serialize")]
impl Serialize for SteamID {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(self.to_steam64())
    }
}

#[cfg(feature = "serialize")]
impl<'de> Deserialize<'de> for SteamID {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let steamid = u64::deserialize(deserializer)?;
        Ok(SteamID::from_steam64(steamid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // We are using this for our tests:
    // https://steamidfinder.com/lookup/76561198092541763/
    fn get_steam64_odd() -> u64 {
        76_561_198_092_541_763
    }

    fn get_steam3() -> u64 {
        132_276_035
    }

    fn get_steam3_unformatted() -> &'static str {
        "[U:1:132276035]"
    }

    fn get_steam64_even() -> u64 {
        76561197984835396
    }

    fn get_steam3_even() -> u64 {
        24569668
    }

    #[test]
    fn steamid_from_steam64() {
        let steamid = SteamID::from_steam64(get_steam64_odd());
        assert_eq!(steamid.to_steam64(), get_steam64_odd())
    }

    #[test]
    fn steamid_to_steam64() {
        let steamid = SteamID::from_steam64(get_steam64_odd());
        assert_eq!(steamid.to_steam64(), get_steam64_odd())
    }

    #[test]
    fn steamid_from_steam3_mine() {
        let steamid = SteamID::from_steam3(get_steam3_even() as u32, None, None);
        assert_eq!(steamid.to_steam64(), get_steam64_even())
    }

    #[test]
    fn steamid64_to_steam3_mine() {
        let steamid = SteamID::from_steam64(get_steam64_even());
        assert_eq!(steamid.to_steam3(), get_steam3_even())
    }

    #[test]
    fn steamid_to_steam3() {
        let steamid = SteamID::from_steam64(get_steam64_odd());
        let steam32 = steamid.to_steam3();
        assert_eq!(steam32, get_steam3())
    }

    #[test]
    fn steamid_from_steam3() {
        let steamid = SteamID::from_steam3(get_steam3() as u32, None, None);
        assert_eq!(steamid.to_steam64(), get_steam64_odd())
    }

    #[test]
    fn steam64_find() {
        let formatted_steamid = format!("text {} xxaasssddff", get_steam64_odd());
        let steamid = SteamID::find(&formatted_steamid).unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[test]
    fn steam3_find() {
        let formatted_steamid = format!("text {} xxaasssddff", get_steam3_unformatted());
        let steamid = SteamID::find(&formatted_steamid).unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[cfg(feature = "serialize")]
    #[test]
    fn serde_se_de() {
        let steamid = SteamID::from_steam64(get_steam64_odd());

        let serialized = serde_json::to_string(&steamid).unwrap();
        let unserialized: SteamID = serde_json::from_str(&serialized).unwrap();

        assert_eq!(steamid, unserialized);
        assert_eq!(steamid.to_steam3(), unserialized.to_steam3());
    }

    /// Builds a Steam64 ID from its parts, independently of the implementation.
    fn steam64_from_parts(universe: u64, account_type: u64, instance: u64, steam3: u64) -> u64 {
        universe << 56 | account_type << 52 | instance << 32 | steam3
    }

    #[test]
    fn steam64_roundtrip() {
        let ids = [
            0,
            1,
            get_steam64_odd(),
            get_steam64_even(),
            // clan
            103_582_791_429_521_412,
            // anonymous game server
            steam64_from_parts(1, 4, 1234, 5678),
            (1 << 63) - 1,
        ];
        for id in ids {
            assert_eq!(SteamID::from_steam64(id).to_steam64(), id, "{}", id);
        }
    }

    #[test]
    fn steam64_highest_bit_is_dropped() {
        assert_eq!(SteamID::from_steam64(1 << 63).to_steam64(), 0);
        assert_eq!(SteamID::from_steam64(u64::MAX).to_steam64(), u64::MAX >> 1);
    }

    #[test]
    fn steam64_highest_bit_is_kept_for_equality() {
        assert_ne!(SteamID::from_steam64(1 << 63), SteamID::from_steam64(0));
    }

    #[test]
    fn steam64_to_steam3_ignores_upper_bits() {
        let id = steam64_from_parts(4, 7, 0xF_FFFF, 0xFFFF_FFFF);
        assert_eq!(SteamID::from_steam64(id).to_steam3(), 0xFFFF_FFFF);
        assert_eq!(SteamID::from_steam64(103_582_791_429_521_412).to_steam3(), 4);
    }

    #[test]
    fn steam3_roundtrip() {
        for steam3 in [0, 1, 2, 3, 24_569_668, 132_276_035, 1 << 31, u32::MAX - 1, u32::MAX] {
            let steamid = SteamID::from_steam3(steam3, None, None);
            assert_eq!(steamid.to_steam3(), steam3 as u64, "{}", steam3);
            assert_eq!(
                steamid.to_steam64(),
                steam64_from_parts(1, 1, 1, steam3 as u64),
                "{}",
                steam3
            );
        }
    }

    #[test]
    fn steam3_defaults_to_public_individual() {
        assert_eq!(
            SteamID::from_steam3(132_276_035, None, None),
            SteamID::from_steam3(132_276_035, Some(EUniverse::Public), Some(EAccountType::Individual))
        );
    }

    #[test]
    fn steam3_with_universe_and_account_type() {
        let universes = [
            EUniverse::Invalid,
            EUniverse::Public,
            EUniverse::Beta,
            EUniverse::Internal,
            EUniverse::Dev,
        ];
        let account_types = [
            EAccountType::Invalid,
            EAccountType::Individual,
            EAccountType::Multiseat,
            EAccountType::GameServer,
            EAccountType::AnonGameServer,
            EAccountType::Pending,
            EAccountType::ContentServer,
            EAccountType::Clan,
            EAccountType::Chat,
            EAccountType::ConsoleUser,
            EAccountType::AnonUser,
        ];
        for universe in universes {
            for account_type in account_types {
                let steamid = SteamID::from_steam3(77, Some(universe), Some(account_type));
                let expected = steam64_from_parts(universe as u64, account_type as u64, 1, 77);
                assert_eq!(steamid.to_steam64(), expected, "{:?} {:?}", universe, account_type);
            }
        }
    }

    #[test]
    fn steam3_always_uses_instance_one() {
        let steamid = SteamID::from_steam3(4, None, Some(EAccountType::Clan));
        assert_eq!(steamid.to_steam64(), steam64_from_parts(1, 7, 1, 4));
    }

    #[test]
    fn steam3_and_steam64_are_equal() {
        assert_eq!(
            SteamID::from_steam3(get_steam3() as u32, None, None),
            SteamID::from_steam64(get_steam64_odd())
        );
        assert_ne!(
            SteamID::from_steam3(get_steam3() as u32 + 1, None, None),
            SteamID::from_steam64(get_steam64_odd())
        );
    }

    #[test]
    fn find_exact() {
        assert_eq!(
            SteamID::find("76561198092541763").unwrap().to_steam64(),
            get_steam64_odd()
        );
        assert_eq!(
            SteamID::find("[U:1:132276035]").unwrap().to_steam64(),
            get_steam64_odd()
        );
    }

    #[test]
    fn find_from_url() {
        let steamid = SteamID::find("https://steamcommunity.com/profiles/76561198092541763/").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());

        let steamid = SteamID::find("https://steamcommunity.com/profiles/[U:1:132276035]").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[test]
    fn find_steam3_account_types() {
        let cases = [
            ('A', EAccountType::AnonGameServer),
            ('G', EAccountType::GameServer),
            ('M', EAccountType::Multiseat),
            ('P', EAccountType::Pending),
            ('C', EAccountType::ContentServer),
            ('g', EAccountType::Clan),
            ('T', EAccountType::Chat),
            ('I', EAccountType::Invalid),
            ('U', EAccountType::Individual),
            ('a', EAccountType::AnonUser),
        ];
        for (letter, account_type) in cases {
            let steamid = SteamID::find(&format!("[{}:1:42]", letter)).unwrap();
            assert_eq!(
                steamid.to_steam64(),
                steam64_from_parts(1, account_type as u64, 1, 42),
                "{}",
                letter
            );
        }
    }

    #[test]
    fn find_steam3_universes() {
        for universe in 0..=4 {
            let steamid = SteamID::find(&format!("[U:{}:42]", universe)).unwrap();
            assert_eq!(steamid.to_steam64(), steam64_from_parts(universe, 1, 1, 42));
        }
    }

    #[test]
    fn find_steam3_edges() {
        assert_eq!(SteamID::find("[U:1:0]").unwrap().to_steam3(), 0);
        assert_eq!(SteamID::find("[U:1:0000132276035]").unwrap().to_steam3(), get_steam3());
        assert_eq!(SteamID::find("[U:1:4294967295]").unwrap().to_steam3(), u32::MAX as u64);
    }

    #[test]
    fn find_prefers_steam3_over_steam64() {
        let steamid = SteamID::find("76561198092541763 [U:1:24569668]").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_even());
    }

    #[test]
    fn find_takes_first_match() {
        let steamid = SteamID::find("[U:1:24569668] [U:1:132276035]").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_even());

        let steamid = SteamID::find("76561197984835396 76561198092541763").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_even());
    }

    #[test]
    fn find_steam64_inside_longer_number() {
        // there are no boundaries around the match, the first 7 followed by 16 digits is used
        let steamid = SteamID::find("176561198092541763999").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[test]
    fn find_malformed_steam3_falls_back_to_steam64() {
        for input in [
            "[U:1:132276035 76561198092541763",
            "[U:5:132276035] 76561198092541763",
            "[U:1:] 76561198092541763",
            "[X:1:132276035] 76561198092541763",
            "[U:1:13227a035] 76561198092541763",
            "[U-1-132276035] 76561198092541763",
        ] {
            let steamid = SteamID::find(input).unwrap();
            assert_eq!(steamid.to_steam64(), get_steam64_odd(), "{}", input);
        }
    }

    #[test]
    fn find_invalid() {
        for input in [
            "",
            "steam",
            "[U:1:]",
            "[U:5:132276035]",
            "U:1:132276035",
            "[U:1:132276035",
            // steam2 universe out of range
            "STEAM_5:1:66138017",
            // steam2 y must be 0 or 1
            "STEAM_0:2:66138017",
            // steam2 z must fit in 31 bits
            "STEAM_0:1:2147483648",
            "STEAM_0:1:99999999999",
            "STEAM_0:1:",
            "steam_0:1:66138017",
            // too short
            "7656119809254176",
            // doesn't start with 7
            "86561198092541763",
            // clans start with 1
            "103582791429521412",
            // non-ascii digits
            "٧٦٥٦١١٩٨٠٩٢٥٤١٧٦٣",
        ] {
            assert_eq!(SteamID::find(input), None, "{}", input);
        }
    }

    #[test]
    fn find_skips_matches_that_cant_be_converted() {
        for input in [
            "[c:1:42]",
            "[L:1:42]",
            "[i:1:42]",
            "[U:1:4294967296]",
            "STEAM_0:1:2147483648",
        ] {
            assert_eq!(SteamID::find(input), None, "{}", input);
        }

        let steamid = SteamID::find("[c:1:42] [U:1:132276035]").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());

        let steamid = SteamID::find("[U:1:4294967296] 76561198092541763").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[test]
    #[allow(deprecated)]
    fn parse_is_find() {
        for input in [
            "76561198092541763",
            "x [U:1:132276035] y",
            "STEAM_0:1:66138017",
            "[c:1:42]",
            "steam",
        ] {
            assert_eq!(SteamID::parse(input), SteamID::find(input), "{}", input);
        }
    }

    #[test]
    fn from_str() {
        for input in [
            "76561198092541763",
            "[U:1:132276035]",
            "STEAM_0:1:66138017",
            "STEAM_1:1:66138017",
        ] {
            let steamid: SteamID = input.parse().unwrap();
            assert_eq!(steamid.to_steam64(), get_steam64_odd(), "{}", input);
        }
    }

    #[test]
    fn from_str_matches_find() {
        let inputs = [
            "[U:1:0]",
            "[g:1:4]",
            "[A:4:42]",
            "[U:1:0000132276035]",
            "[U:1:4294967295]",
            "STEAM_0:0:0",
            "STEAM_4:0:12284834",
            "STEAM_0:1:2147483647",
            "76561197960265728",
            "79999999999999999",
        ];
        for input in inputs {
            assert_eq!(input.parse::<SteamID>().ok(), SteamID::find(input), "{}", input);
        }
    }

    #[test]
    fn from_str_unknown_format() {
        for input in [
            "",
            " ",
            "steam",
            " 76561198092541763",
            "76561198092541763 ",
            "76561198092541763\n",
            "176561198092541763",
            "765611980925417630",
            "7656119809254176",
            "103582791429521412",
            "https://steamcommunity.com/profiles/76561198092541763",
            "[U:1:132276035] ",
            "[U:1:132276035]]",
            "[U:1:132276035",
            "[U:5:132276035]",
            "[X:1:132276035]",
            "[U:1:]",
            "[U:1:-1]",
            "STEAM_0:1:66138017 ",
            "STEAM_0:1:",
            "STEAM_0:2:66138017",
            "STEAM_5:1:66138017",
            "steam_0:1:66138017",
            "STEAM_0:1:66138017[U:1:1]",
            "٧٦٥٦١١٩٨٠٩٢٥٤١٧٦٣",
        ] {
            assert_eq!(
                input.parse::<SteamID>(),
                Err(ParseSteamIDError::UnknownFormat),
                "{:?}",
                input
            );
        }
    }

    #[test]
    fn from_str_unsupported_account_type() {
        for letter in ['c', 'L', 'i'] {
            assert_eq!(
                format!("[{}:1:42]", letter).parse::<SteamID>(),
                Err(ParseSteamIDError::UnsupportedAccountType(letter))
            );
        }
    }

    #[test]
    fn from_str_account_out_of_range() {
        for (input, account) in [
            ("[U:1:4294967296]", "4294967296"),
            ("[U:1:99999999999999999999999]", "99999999999999999999999"),
            ("STEAM_0:1:2147483648", "2147483648"),
            ("STEAM_0:1:4294967296", "4294967296"),
        ] {
            assert_eq!(
                input.parse::<SteamID>(),
                Err(ParseSteamIDError::AccountOutOfRange(account.to_owned())),
                "{}",
                input
            );
        }
    }

    #[test]
    fn error_messages() {
        assert_eq!(
            ParseSteamIDError::UnknownFormat.to_string(),
            "not a Steam64, Steam3 or Steam2 id"
        );
        assert_eq!(
            ParseSteamIDError::UnsupportedAccountType('c').to_string(),
            "unsupported Steam3 account type `c`"
        );
        assert_eq!(
            ParseSteamIDError::AccountOutOfRange("4294967296".to_owned()).to_string(),
            "account `4294967296` is out of range"
        );
    }

    #[test]
    fn error_is_std_error() {
        let error: Box<dyn std::error::Error> = Box::new(ParseSteamIDError::UnknownFormat);
        assert_eq!(error.to_string(), "not a Steam64, Steam3 or Steam2 id");
    }

    #[test]
    fn copy_is_equal() {
        let steamid = SteamID::from_steam64(get_steam64_odd());
        let copy = steamid;
        assert_eq!(copy, steamid);
    }

    #[test]
    fn hash_set() {
        let set: std::collections::HashSet<SteamID> = [
            SteamID::from_steam64(get_steam64_odd()),
            SteamID::from_steam3(get_steam3() as u32, None, None),
            SteamID::from_steam64(get_steam64_even()),
        ]
        .iter()
        .copied()
        .collect();
        assert_eq!(set.len(), 2);
        assert!(set.contains(&SteamID::from_steam64(get_steam64_even())));
    }

    #[test]
    fn ordered_by_steam64() {
        let mut ids = [
            SteamID::from_steam64(get_steam64_odd()),
            SteamID::from_steam64(103_582_791_429_521_412),
            SteamID::from_steam64(get_steam64_even()),
        ];
        ids.sort();
        assert_eq!(
            ids.map(SteamID::to_steam64),
            [get_steam64_even(), get_steam64_odd(), 103_582_791_429_521_412]
        );
    }

    #[cfg(feature = "serialize")]
    #[test]
    fn serde_as_number() {
        let steamid = SteamID::from_steam64(get_steam64_odd());
        assert_eq!(serde_json::to_string(&steamid).unwrap(), "76561198092541763");

        let steamid: SteamID = serde_json::from_str("76561198092541763").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[cfg(feature = "serialize")]
    #[test]
    fn serde_rejects_string() {
        assert!(serde_json::from_str::<SteamID>("\"76561198092541763\"").is_err());
    }

    #[cfg(feature = "serialize")]
    #[test]
    fn serde_drops_highest_bit() {
        let steamid = SteamID::from_steam64(u64::MAX);
        assert_eq!(serde_json::to_string(&steamid).unwrap(), (u64::MAX >> 1).to_string());
    }

    #[test]
    fn steam2_find() {
        let steamid = SteamID::find("STEAM_0:1:66138017").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());

        let steamid = SteamID::find("STEAM_0:0:12284834").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_even());
    }

    #[test]
    fn steam2_find_universe_one_is_public() {
        let steamid = SteamID::find("STEAM_1:1:66138017").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[test]
    fn steam2_find_universes() {
        for universe in 2..=4 {
            let steamid = SteamID::find(&format!("STEAM_{}:1:66138017", universe)).unwrap();
            assert_eq!(steamid.to_steam64(), steam64_from_parts(universe, 1, 1, get_steam3()));
        }
    }

    #[test]
    fn steam2_find_edges() {
        assert_eq!(SteamID::find("STEAM_0:0:0").unwrap().to_steam3(), 0);
        assert_eq!(SteamID::find("STEAM_0:1:0").unwrap().to_steam3(), 1);
        assert_eq!(
            SteamID::find("STEAM_0:1:2147483647").unwrap().to_steam3(),
            u32::MAX as u64
        );
        assert_eq!(
            SteamID::find("STEAM_0:1:0066138017").unwrap().to_steam64(),
            get_steam64_odd()
        );
    }

    #[test]
    fn steam2_find_from_text() {
        let steamid = SteamID::find("player STEAM_0:1:66138017 joined").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[test]
    fn steam2_find_skips_invalid_match() {
        let steamid = SteamID::find("STEAM_0:1:2147483648 STEAM_0:0:12284834").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_even());
    }

    #[test]
    fn steam2_find_has_lowest_priority() {
        let steamid = SteamID::find("STEAM_0:0:12284834 [U:1:132276035]").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());

        let steamid = SteamID::find("STEAM_0:0:12284834 76561198092541763").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[test]
    fn steam2_format() {
        assert_eq!(
            SteamID::from_steam64(get_steam64_odd()).to_steam2(),
            "STEAM_0:1:66138017"
        );
        assert_eq!(
            SteamID::from_steam64(get_steam64_even()).to_steam2(),
            "STEAM_0:0:12284834"
        );
        assert_eq!(SteamID::from_steam3(0, None, None).to_steam2(), "STEAM_0:0:0");
        assert_eq!(
            SteamID::from_steam3(u32::MAX, None, None).to_steam2(),
            "STEAM_0:1:2147483647"
        );
    }

    #[test]
    fn steam2_format_universes() {
        for (universe, digit) in [
            (EUniverse::Invalid, 0),
            (EUniverse::Public, 0),
            (EUniverse::Beta, 2),
            (EUniverse::Internal, 3),
            (EUniverse::Dev, 4),
        ] {
            let steamid = SteamID::from_steam3(get_steam3() as u32, Some(universe), None);
            assert_eq!(steamid.to_steam2(), format!("STEAM_{}:1:66138017", digit));
        }
    }

    #[test]
    fn steam2_roundtrip() {
        for steam3 in [0, 1, 2, 24_569_668, 132_276_035, u32::MAX] {
            let steamid = SteamID::from_steam3(steam3, None, None);
            assert_eq!(SteamID::find(&steamid.to_steam2()), Some(steamid), "{}", steam3);
        }
    }

    #[test]
    fn getters() {
        let steamid = SteamID::from_steam64(get_steam64_odd());
        assert_eq!(steamid.account_id(), get_steam3() as u32);
        assert_eq!(steamid.instance(), 1);
        assert_eq!(steamid.account_type(), Some(EAccountType::Individual));
        assert_eq!(steamid.universe(), Some(EUniverse::Public));

        // clan
        let steamid = SteamID::from_steam64(103_582_791_429_521_412);
        assert_eq!(steamid.account_id(), 4);
        assert_eq!(steamid.instance(), 0);
        assert_eq!(steamid.account_type(), Some(EAccountType::Clan));
        assert_eq!(steamid.universe(), Some(EUniverse::Public));
    }

    #[test]
    fn getters_extremes() {
        let steamid = SteamID::from_steam64(steam64_from_parts(4, 10, 0xF_FFFF, 0xFFFF_FFFF));
        assert_eq!(steamid.account_id(), u32::MAX);
        assert_eq!(steamid.instance(), 0xF_FFFF);
        assert_eq!(steamid.account_type(), Some(EAccountType::AnonUser));
        assert_eq!(steamid.universe(), Some(EUniverse::Dev));

        let steamid = SteamID::from_steam64(0);
        assert_eq!(steamid.account_id(), 0);
        assert_eq!(steamid.instance(), 0);
        assert_eq!(steamid.account_type(), Some(EAccountType::Invalid));
        assert_eq!(steamid.universe(), Some(EUniverse::Invalid));
    }

    #[test]
    fn getters_unknown_values() {
        let steamid = SteamID::from_steam64(steam64_from_parts(5, 11, 1, 42));
        assert_eq!(steamid.account_type(), None);
        assert_eq!(steamid.universe(), None);

        let steamid = SteamID::from_steam64(u64::MAX);
        assert_eq!(steamid.account_type(), None);
        assert_eq!(steamid.universe(), None);
    }

    #[test]
    fn account_id_is_to_steam3() {
        for steam64 in [
            0,
            get_steam64_odd(),
            get_steam64_even(),
            103_582_791_429_521_412,
            u64::MAX,
        ] {
            let steamid = SteamID::from_steam64(steam64);
            assert_eq!(steamid.account_id() as u64, steamid.to_steam3(), "{}", steam64);
        }
    }

    #[test]
    fn getters_match_from_steam3() {
        let steamid = SteamID::from_steam3(42, Some(EUniverse::Beta), Some(EAccountType::GameServer));
        assert_eq!(steamid.account_id(), 42);
        assert_eq!(steamid.instance(), 1);
        assert_eq!(steamid.account_type(), Some(EAccountType::GameServer));
        assert_eq!(steamid.universe(), Some(EUniverse::Beta));
    }
}
