use std::str::FromStr;

use bitvec::prelude::*;
use num::FromPrimitive;
use regex::Regex;

use lazy_static::lazy_static;
use steam_language_gen::generated::enums::{EAccountType, EUniverse};

// TODO - Error catching

lazy_static! {
    static ref REGEX_STEAM2: Regex =
        Regex::new(r"STEAM_(?P<universe>[0-4]):(?P<authserver>[0-1]):(?P<accountid>\d+)").unwrap();
    static ref REGEX_STEAM3: Regex =
        Regex::new(r"\[(?P<type>[AGMPCgcLTIUai]):(?P<universe>[0-4]):(?P<account>\d+)\]").unwrap();
    static ref REGEX_STEAM64: Regex = Regex::new(r"(?P<account>7\d{16})").unwrap();
    static ref REGEX_STEAM3_FALLBACK: Regex = Regex::new(r"").unwrap();
}

struct AccountType(EAccountType);

impl AccountType {
    fn new(identifier: &str) -> Option<Self> {
        let kind = match identifier {
            "A" => EAccountType::AnonGameServer,
            "G" => EAccountType::GameServer,
            "M" => EAccountType::Multiseat,
            "P" => EAccountType::Pending,
            "C" => EAccountType::ContentServer,
            "g" => EAccountType::Clan,
            "T" => EAccountType::Chat,
            "I" => EAccountType::Invalid,
            "U" => EAccountType::Individual,
            "a" => EAccountType::AnonUser,
            _ => return None,
        };
        Some(Self { 0: kind })
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Let X, Y and Z constants be defined by the SteamID: STEAM_X:Y:Z.
pub struct SteamID {
    /// ID number of account. Either 0 or 1
    account_id: bool,
    /// Account Number. Z
    account_number: BitVec<Msb0, u64>,
    account_instance: BitVec<Msb0, u64>,
    /// 4 Bits.
    account_type: BitVec<Msb0, u64>,
    /// Universe. 8 Bits
    universe: BitVec<Msb0, u64>,
}

/// Reference: https://developer.valvesoftware.com/wiki/SteamID
impl SteamID {
    /// Using the formula W=Z*2+Y, a SteamID can be converted to Steam3.
    /// Source: https://steamcommunity.com/path/[letter:1:W]
    pub fn to_steam3(&self) -> u64 {
        // let steamid64_identifier: u64 = 0x0110_0001_0000_0000;

        let z = self.account_number.load::<u64>();
        let y = self.account_id as u64;
        // let x = self.universe.load::<u64>();

        z * 2 + y
    }

    pub fn to_steam64(&self) -> u64 {
        let mut vec: BitVec<Msb0> = BitVec::with_capacity(64);
        vec.extend_from_slice(self.universe.as_bitslice());
        vec.extend_from_slice(self.account_type.as_bitslice());
        vec.extend_from_slice(self.account_instance.as_bitslice());
        vec.extend_from_slice(self.account_number.as_bitslice());
        vec.push(self.account_id);

        // this should be ..64, we are omitting a initial zero(first bit)
        // from the steamID
        vec[1..].load::<u64>()
    }

    /// Creates a new SteamID from the Steam3 format.
    /// Defaults to Public universe, and Individual account.
    /// You can use the parse utility function.
    pub fn from_steam3(
        steam3: u32,
        universe: Option<EUniverse>,
        account_type: Option<EAccountType>,
    ) -> Self {
        let account_number = ((steam3 - 1) / 2) as u64;
        let universe = universe.unwrap_or(EUniverse::Public) as u64;
        let account_type = account_type.unwrap_or(EAccountType::Individual) as u64;
        let instance = 1u64;

        Self {
            account_id: true,
            account_number: BitVec::from(&account_number.bits()[33..]),
            account_instance: BitVec::from(&instance.bits()[44..]),
            account_type: BitVec::from(&account_type.bits()[60..]),
            universe: BitVec::from(&universe.bits()[56..]),
        }
    }

    /// Creates a new SteamID from the Steam64 format.
    pub fn from_steam64(steam64: u64) -> Self {
        let steam_as_bits = steam64.bits::<Msb0>();
        let steamid_len = steam_as_bits.len() - 1;

        let account_id = steam_as_bits[steamid_len];
        let account_number = steam_as_bits[32..steamid_len].to_vec();
        let account_instance = steam_as_bits[12..32].to_vec();
        let account_type = steam_as_bits[8..12].to_vec();
        let universe = steam_as_bits[0..8].to_vec();

        Self { account_id, account_number, account_instance, account_type, universe }
    }

    /// Parses the following formats:
    /// Steam64: digit 7 + 16 digits
    ///
    /// Steam3: [T:U:D] where T: The account type, U: The account universe, D: Account number,
    pub fn parse(steamid: &str) -> Option<Self> {
        if REGEX_STEAM3.is_match(steamid) {
            let captures = REGEX_STEAM3.captures(steamid).unwrap();

            // since it got matched, we can unwrap
            let account_number = captures.name("account").unwrap().as_str();
            let account_universe = captures.name("universe").unwrap().as_str();
            let account_type = captures.name("type").unwrap().as_str();

            // TODO - match instance
            // let account_instance = captures.name("instance");

            return Some(Self::from_steam3(
                account_number.parse().unwrap(),
                Some(EUniverse::from_u32(u32::from_str(account_universe).unwrap()).unwrap()),
                Some(AccountType::new(account_type).unwrap().0),
            ));
        } else if REGEX_STEAM64.is_match(steamid) {
            let captures = REGEX_STEAM64.captures(steamid).unwrap();
            let number = captures.name("account").unwrap();

            return Some(Self::from_steam64(u64::from_str(number.as_str()).unwrap()));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // We are using this for our tests:
    // https://steamidfinder.com/lookup/76561198092541763/
    fn get_steam64() -> u64 {
        76_561_198_092_541_763
    }

    fn get_steam3() -> u64 {
        132_276_035
    }

    fn get_steam3_unformatted() -> &'static str {
        "[U:1:132276035]"
    }

    #[test]
    fn steamid_from_steam64() {
        let steamid = SteamID::from_steam64(get_steam64());
    }

    #[test]
    fn steamid_to_steam64() {
        let steamid = SteamID::from_steam64(get_steam64());
        let steam64 = steamid.to_steam64();
        assert_eq!(steam64, get_steam64())
    }

    #[test]
    fn steamid_to_steam3() {
        let steamid = SteamID::from_steam64(get_steam64());
        let steam32 = steamid.to_steam3();
        assert_eq!(steam32, get_steam3())
    }

    #[test]
    fn steamid_from_steam3() {
        let steamid = SteamID::from_steam3(get_steam3() as u32, None, None);
        assert_eq!(steamid.to_steam64(), get_steam64())
    }

    #[test]
    fn steam64_parse() {
        let formatted_steamid = format!("text {} xxaasssddff", get_steam64());
        let steamid = SteamID::parse(&formatted_steamid).unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64());
    }

    #[test]
    fn steam3_parse() {
        let formatted_steamid = format!("text {} xxaasssddff", get_steam3_unformatted());
        let steamid = SteamID::parse(&formatted_steamid).unwrap();
        assert_eq!(steamid.to_steam64(), get_steam64());
    }
}
