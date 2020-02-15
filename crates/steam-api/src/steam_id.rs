use bitvec::prelude::*;
use regex::{Regex, RegexBuilder};
use std::error::Error;

#[allow(clippy::invalid_regex)]
lazy_static! {
    static ref REGEX_STEAM2: Regex = Regex::new(r"STEAM_(?<universe>[0-4]):(?<authserver>[0-1]):(?<accountid>\d+)").unwrap();
    static ref REGEX_STEAM3: Regex = Regex::new(r"\[(?<type>[AGMPCgcLTIUai]):(?<universe>[0-4]):(?<account>\d+)(:(?<instance>\d+))]").unwrap();
    static ref REGEX_STEAM3_FALLBACK: Regex = Regex::new(r"").unwrap();
}

#[derive(Debug)]
/// Let X, Y and Z constants be defined by the SteamID: STEAM_X:Y:Z.
pub struct SteamID {
    /// ID number of account. Either 0 or 1
    account_id: bool,
    /// Account Number. Z
    account_number: BitVec<Msb0, u64>,
    account_instance: BitVec<Msb0, u64>,
    account_type: BitVec<Msb0, u64>,
    /// Universe
    universe: BitVec<Msb0, u64>,
}

/// Reference: https://developer.valvesoftware.com/wiki/SteamID
impl SteamID {
    /// Using the formula W=Z*2+Y, a SteamID can be converted to the following link:
    /// http or https://steamcommunity.com/path/[letter:1:W]
    fn to_steam32(&self) -> u64 {
        let steamid64_identifier: u64 = 0x0110_0001_0000_0000;

        let z = self.account_number.load::<u64>();
        let y = self.account_id as u64;
        let x = self.universe.load::<u64>();

        z * 2 + y
    }

    fn to_steam64(&self) -> u64 {
        let mut vec: BitVec<Msb0> = BitVec::with_capacity(64);
        vec.extend_from_slice(self.universe.as_bitslice());
        vec.extend_from_slice(self.account_type.as_bitslice());
        vec.extend_from_slice(self.account_instance.as_bitslice());
        vec.extend_from_slice(self.account_number.as_bitslice());
        vec.push(self.account_id);

        trace!("Generated STEAM64: {:?}", vec);
        trace!("Generated STEAM64 len: {:?}", vec.len());

        // this should be ..64, we are omitting a initial zero(first bit)
        // from the steamID
        vec[1..].load::<u64>()
    }

    fn from_steam64(steam64: u64) -> Self {
        let steam_as_bits = steam64.bits::<Msb0>();
        let steamid_len = steam_as_bits.len() - 1;

        let account_id = steam_as_bits[steamid_len];
        let account_number = steam_as_bits[32..steamid_len].to_vec();
        let account_instance = steam_as_bits[12..32].to_vec();
        let account_type = steam_as_bits[8..12].to_vec();
        let universe = steam_as_bits[0..8].to_vec();

        Self { account_id, account_number, account_instance, account_type, universe }
    }

    /// Utility function
    fn parse() -> Self {
        unimplemented!()
    }
}

enum SteamId {
    Steam2,
    Steam3,
    Steam64,
    SteamGUID,
}

impl SteamId {}


#[cfg(test)]
mod tests {
    use super::*;

    // We are using this for our tests:
    // https://steamidfinder.com/lookup/76561198092541763/
    fn get_steam_id_64() -> u64 {
        76_561_198_092_541_763
    }

    fn get_steam_id_32() -> u64 {
        132_276_035
    }

    #[test]
    fn steamid_from_u64() {
        let steamid = SteamID::from_steam64(get_steam_id_64());
    }

    #[test]
    fn steamid_to_u64() {
        let steamid = SteamID::from_steam64(get_steam_id_64());
        let steam64 = steamid.to_steam64();
        assert_eq!(steam64, get_steam_id_64())
    }

    #[test]
    fn steamid_to_steam32() {
        let steamid = SteamID::from_steam64(get_steam_id_64());
        let steam32 = steamid.to_steam32();
        assert_eq!(steam32, get_steam_id_32())
    }
}