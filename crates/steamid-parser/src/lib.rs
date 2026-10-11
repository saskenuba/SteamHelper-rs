#[cfg(feature = "serialize")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use steam_language_gen::generated::enums::{EAccountType, EUniverse};

// TODO - Error catching

// Bit layout of a Steam64 ID, from most to least significant:
// universe (8) | account type (4) | account instance (20) | account number (31) | account id (1)
const UNIVERSE_SHIFT: u32 = 56;
const ACCOUNT_TYPE_SHIFT: u32 = 52;
const ACCOUNT_TYPE_MASK: u64 = 0xF;
const ACCOUNT_INSTANCE_SHIFT: u32 = 32;
const STEAM3_MASK: u64 = 0xFFFF_FFFF;
// The highest universe bit is not kept when converting back to Steam64. No real universe sets it.
const STEAM64_MASK: u64 = !(1 << 63);

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

fn universe_from_digit(digit: u8) -> Option<EUniverse> {
    let universe = match digit {
        b'0' => EUniverse::Invalid,
        b'1' => EUniverse::Public,
        b'2' => EUniverse::Beta,
        b'3' => EUniverse::Internal,
        b'4' => EUniverse::Dev,
        _ => return None,
    };
    Some(universe)
}

/// Finds the first Steam3 ID in the format `[T:U:D]`, returning the account type letter,
/// universe digit and account number digits.
///
/// T is one of `AGMPCgcLTIUai`, U is a digit from 0 to 4 and D is one or more digits.
fn find_steam3(input: &[u8]) -> Option<(u8, u8, &str)> {
    (0..input.len()).find_map(|start| {
        let rest = &input[start..];
        match rest {
            [b'[', kind, b':', universe @ b'0'..=b'4', b':', account @ ..] if b"AGMPCgcLTIUai".contains(kind) => {
                let digits = account.iter().take_while(|c| c.is_ascii_digit()).count();
                if digits == 0 || account.get(digits) != Some(&b']') {
                    return None;
                }
                // only ascii digits were matched, so this is valid utf8
                let account = std::str::from_utf8(&account[..digits]).unwrap();
                Some((*kind, *universe, account))
            }
            _ => None,
        }
    })
}

/// Finds the first Steam64 ID: the digit 7 followed by 16 digits.
fn find_steam64(input: &[u8]) -> Option<&str> {
    input
        .windows(17)
        .find(|window| window[0] == b'7' && window.iter().all(u8::is_ascii_digit))
        // only ascii digits were matched, so this is valid utf8
        .map(|window| std::str::from_utf8(window).unwrap())
}

#[derive(Debug, Clone, PartialEq)]
/// Let X, Y and Z constants be defined by the SteamID: STEAM_X:Y:Z.
pub struct SteamID(u64);

/// Reference: https://developer.valvesoftware.com/wiki/SteamID
impl SteamID {
    /// Using the formula W=Z*2+Y, a SteamID can be converted to Steam3.
    /// Source: https://steamcommunity.com/path/[letter:1:W]
    pub fn to_steam3(&self) -> u64 {
        self.0 & STEAM3_MASK
    }

    pub fn to_steam64(&self) -> u64 {
        self.0 & STEAM64_MASK
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

    /// Parses the following formats:
    /// Steam64: digit 7 + 16 digits
    ///
    /// Steam3: [T:U:D] where T: The account type, U: The account universe, D: Account number,
    pub fn parse(steamid: &str) -> Option<Self> {
        if let Some((account_type, account_universe, account_number)) = find_steam3(steamid.as_bytes()) {
            // TODO - match instance
            // let account_instance = captures.name("instance");

            return Some(Self::from_steam3(
                account_number.parse().unwrap(),
                Some(universe_from_digit(account_universe).unwrap()),
                Some(AccountType::new(account_type).unwrap().0),
            ));
        } else if let Some(number) = find_steam64(steamid.as_bytes()) {
            return Some(Self::from_steam64(number.parse().unwrap()));
        }
        None
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
    fn steam64_parse() {
        let formatted_steamid = format!("text {} xxaasssddff", get_steam64_odd());
        let steamid = SteamID::parse(&formatted_steamid).unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[test]
    fn steam3_parse() {
        let formatted_steamid = format!("text {} xxaasssddff", get_steam3_unformatted());
        let steamid = SteamID::parse(&formatted_steamid).unwrap();
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
    fn parse_exact() {
        assert_eq!(
            SteamID::parse("76561198092541763").unwrap().to_steam64(),
            get_steam64_odd()
        );
        assert_eq!(
            SteamID::parse("[U:1:132276035]").unwrap().to_steam64(),
            get_steam64_odd()
        );
    }

    #[test]
    fn parse_from_url() {
        let steamid = SteamID::parse("https://steamcommunity.com/profiles/76561198092541763/").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());

        let steamid = SteamID::parse("https://steamcommunity.com/profiles/[U:1:132276035]").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[test]
    fn parse_steam3_account_types() {
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
            let steamid = SteamID::parse(&format!("[{}:1:42]", letter)).unwrap();
            assert_eq!(
                steamid.to_steam64(),
                steam64_from_parts(1, account_type as u64, 1, 42),
                "{}",
                letter
            );
        }
    }

    #[test]
    fn parse_steam3_universes() {
        for universe in 0..=4 {
            let steamid = SteamID::parse(&format!("[U:{}:42]", universe)).unwrap();
            assert_eq!(steamid.to_steam64(), steam64_from_parts(universe, 1, 1, 42));
        }
    }

    #[test]
    fn parse_steam3_edges() {
        assert_eq!(SteamID::parse("[U:1:0]").unwrap().to_steam3(), 0);
        assert_eq!(SteamID::parse("[U:1:0000132276035]").unwrap().to_steam3(), get_steam3());
        assert_eq!(SteamID::parse("[U:1:4294967295]").unwrap().to_steam3(), u32::MAX as u64);
    }

    #[test]
    fn parse_prefers_steam3_over_steam64() {
        let steamid = SteamID::parse("76561198092541763 [U:1:24569668]").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_even());
    }

    #[test]
    fn parse_takes_first_match() {
        let steamid = SteamID::parse("[U:1:24569668] [U:1:132276035]").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_even());

        let steamid = SteamID::parse("76561197984835396 76561198092541763").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_even());
    }

    #[test]
    fn parse_steam64_inside_longer_number() {
        // there are no boundaries around the match, the first 7 followed by 16 digits is used
        let steamid = SteamID::parse("176561198092541763999").unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64_odd());
    }

    #[test]
    fn parse_malformed_steam3_falls_back_to_steam64() {
        for input in [
            "[U:1:132276035 76561198092541763",
            "[U:5:132276035] 76561198092541763",
            "[U:1:] 76561198092541763",
            "[X:1:132276035] 76561198092541763",
            "[U:1:13227a035] 76561198092541763",
            "[U-1-132276035] 76561198092541763",
        ] {
            let steamid = SteamID::parse(input).unwrap();
            assert_eq!(steamid.to_steam64(), get_steam64_odd(), "{}", input);
        }
    }

    #[test]
    fn parse_invalid() {
        for input in [
            "",
            "steam",
            "[U:1:]",
            "[U:5:132276035]",
            "U:1:132276035",
            "[U:1:132276035",
            // steam2 is not supported
            "STEAM_0:1:66138017",
            // too short
            "7656119809254176",
            // doesn't start with 7
            "86561198092541763",
            // clans start with 1
            "103582791429521412",
            // non-ascii digits
            "٧٦٥٦١١٩٨٠٩٢٥٤١٧٦٣",
        ] {
            assert_eq!(SteamID::parse(input), None, "{}", input);
        }
    }

    // The following inputs are matched but can't be converted, which currently panics.

    #[test]
    #[should_panic]
    fn parse_steam3_unsupported_account_type_c() {
        SteamID::parse("[c:1:42]");
    }

    #[test]
    #[should_panic]
    fn parse_steam3_unsupported_account_type_l() {
        SteamID::parse("[L:1:42]");
    }

    #[test]
    #[should_panic]
    fn parse_steam3_unsupported_account_type_i() {
        SteamID::parse("[i:1:42]");
    }

    #[test]
    #[should_panic]
    fn parse_steam3_account_overflow() {
        SteamID::parse("[U:1:4294967296]");
    }

    #[test]
    fn clone_is_equal() {
        let steamid = SteamID::from_steam64(get_steam64_odd());
        assert_eq!(steamid.clone(), steamid);
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
}
