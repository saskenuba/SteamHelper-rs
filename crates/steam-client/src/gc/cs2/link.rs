//! CS2 inspect links: the "Inspect in Game..." links of the Steam inventory and market.
//!
//! ```
//! # fn main() -> Result<(), steam_client::gc::cs2::InspectLinkError> {
//! use steam_client::gc::cs2::InspectLink;
//!
//! let link: InspectLink = concat!(
//!     "steam://rungame/730/76561202255233023/+csgo_econ_action_preview",
//!     "%20S76561198084749846A698323590D7935523998312483177",
//! )
//! .parse()?;
//! assert_eq!(link.asset_id(), Some(698323590));
//! # Ok(())
//! # }
//! ```
//!
//! Most links name an item, which the game coordinator then describes. Some, since 2025, embed the item instead:
//! see [`EmbeddedItem`].

use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;

use steamid_parser::SteamID;
use thiserror::Error;

use crate::gc::cs2::ItemDetails;
use crate::gc::cs2::ItemDetailsError;

/// The command an inspect link runs.
const COMMAND: &str = "csgo_econ_action_preview";
/// What Steam puts in the `steam://rungame` links it gives, where a SteamID goes.
const RUNGAME_STEAM_ID: u64 = 76561202255233023;

/// A CS2 inspect link, as the Steam inventory and market give.
///
/// Parses from a whole link (`steam://rungame/730/…/+csgo_econ_action_preview%20S…A…D…`), with `%20` or a space,
/// or from its command (`+csgo_econ_action_preview S…A…D…`), or from its argument alone (`S…A…D…`). Displays as a
/// whole link.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InspectLink {
    /// An item in an inventory: `S<owner>A<asset id>D<d>`.
    Inventory {
        /// The account the item is in.
        owner: SteamID,
        /// The item.
        asset_id: u64,
        /// The D of the link, which the game coordinator needs.
        d: u64,
    },
    /// An item listed on the market: `M<listing id>A<asset id>D<d>`.
    Market {
        /// The market listing.
        listing_id: u64,
        /// The item.
        asset_id: u64,
        /// The D of the link, which the game coordinator needs.
        d: u64,
    },
    /// An item the link carries itself, so the game coordinator isn't needed.
    ///
    /// Also what an `S…A…D…` or `M…A…D…` link is when its D is such an item, in hex: the S, M and A are then not
    /// kept.
    Embedded(EmbeddedItem),
}

/// What the game coordinator is asked for an inspect link: `CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockRequest`.
/// 0 when the link has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PreviewParams {
    /// The owner's SteamID64.
    pub(crate) s: u64,
    /// The asset id.
    pub(crate) a: u64,
    /// The D.
    pub(crate) d: u64,
    /// The market listing id.
    pub(crate) m: u64,
}

impl InspectLink {
    /// Fills the link template of an item in an inventory, as Steam's inventory JSON gives in its `actions`, with
    /// `%owner_steamid%` and `%assetid%` in place of the owner and asset id.
    pub fn from_template(template: &str, owner: SteamID, asset_id: u64) -> Result<Self, InspectLinkError> {
        template
            .replace("%owner_steamid%", &owner.to_string())
            .replace("%assetid%", &asset_id.to_string())
            .parse()
    }

    /// The item's asset id. `None` for an embedded item, whose data has it.
    pub fn asset_id(&self) -> Option<u64> {
        match self {
            Self::Inventory { asset_id, .. } | Self::Market { asset_id, .. } => Some(*asset_id),
            Self::Embedded(_) => None,
        }
    }

    /// What to ask the game coordinator, or the item the link embeds, which needs no asking.
    pub(crate) fn params(&self) -> Result<PreviewParams, &EmbeddedItem> {
        match *self {
            Self::Inventory { owner, asset_id, d } => Ok(PreviewParams {
                s: owner.to_steam64(),
                a: asset_id,
                d,
                m: 0,
            }),
            Self::Market {
                listing_id,
                asset_id,
                d,
            } => Ok(PreviewParams {
                s: 0,
                a: asset_id,
                d,
                m: listing_id,
            }),
            Self::Embedded(ref item) => Err(item),
        }
    }
}

impl FromStr for InspectLink {
    type Err = InspectLinkError;

    fn from_str(link: &str) -> Result<Self, Self::Err> {
        let argument = argument(link.trim()).ok_or(InspectLinkError::NotInspectLink)?;
        let (kind, rest) = match argument.as_bytes().first() {
            Some(b'S') => ('S', &argument[1..]),
            Some(b'M') => ('M', &argument[1..]),
            // S and M aren't hex digits, but A and D are
            Some(first) if first.is_ascii_hexdigit() => return EmbeddedItem::from_hex(argument).map(Self::Embedded),
            _ => return Err(InspectLinkError::NotInspectLink),
        };
        let (id, rest) = rest.split_once('A').ok_or(InspectLinkError::Missing('A'))?;
        let (asset_id, d) = rest.split_once('D').ok_or(InspectLinkError::Missing('D'))?;
        let id = number(id, kind)?;
        let asset_id = number(asset_id, 'A')?;
        let owner = match kind {
            'S' => Some(owner(id)?),
            _ => None,
        };
        // links that embed the item in place of the D number, which names it already
        if d.bytes().any(|c| !c.is_ascii_digit()) && d.bytes().all(|c| c.is_ascii_hexdigit()) {
            return EmbeddedItem::from_hex(d).map(Self::Embedded);
        }
        let d = number(d, 'D')?;
        Ok(match owner {
            Some(owner) => Self::Inventory { owner, asset_id, d },
            None => Self::Market {
                listing_id: id,
                asset_id,
                d,
            },
        })
    }
}

impl Display for InspectLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inventory { owner, asset_id, d } => write!(
                f,
                "steam://rungame/730/{}/+{}%20S{}A{}D{}",
                RUNGAME_STEAM_ID, COMMAND, owner, asset_id, d
            ),
            Self::Market {
                listing_id,
                asset_id,
                d,
            } => write!(
                f,
                "steam://rungame/730/{}/+{}%20M{}A{}D{}",
                RUNGAME_STEAM_ID, COMMAND, listing_id, asset_id, d
            ),
            Self::Embedded(item) => write!(f, "steam://run/730//+{}%20{}", COMMAND, item),
        }
    }
}

/// The argument of the command of an inspect link: a whole link, the command, or the argument alone.
fn argument(link: &str) -> Option<&str> {
    let command = match link.strip_prefix("steam://") {
        Some(url) => {
            let path = url
                .strip_prefix("rungame/730/")
                .or_else(|| url.strip_prefix("run/730/"))?;
            // a SteamID in `rungame` links, nothing in `run` ones
            let (steam_id, command) = path.split_once('/')?;
            if !steam_id.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            command.strip_prefix('+')?
        }
        None => link.strip_prefix('+').unwrap_or(link),
    };
    match command.strip_prefix(COMMAND) {
        Some(argument) => argument.strip_prefix("%20").or_else(|| argument.strip_prefix(' ')),
        None if link.starts_with("steam://") || link.starts_with('+') => None,
        None => Some(command),
    }
}

fn number(digits: &str, part: char) -> Result<u64, InspectLinkError> {
    // `u64::from_str` takes a sign too
    if !digits.bytes().all(|c| c.is_ascii_digit()) {
        return Err(InspectLinkError::InvalidNumber(part));
    }
    digits.parse().map_err(|_| InspectLinkError::InvalidNumber(part))
}

/// The owner of an item: an individual account in the public universe.
fn owner(steam64: u64) -> Result<SteamID, InspectLinkError> {
    let owner = SteamID::from_steam64(steam64);
    if owner.account_id() == 0 || SteamID::from_account_id(owner.account_id()) != owner {
        return Err(InspectLinkError::InvalidOwner(steam64));
    }
    Ok(owner)
}

/// An item an inspect link carries: its `CEconItemPreviewDataBlock`, which the game coordinator otherwise gives.
///
/// The link's argument is hex: a key byte, then the message and a checksum, each byte XORed with the key. The
/// checksum is `(crc & 0xFFFF) ^ (len * crc)` as a big-endian `u32`, where `crc` is the CRC-32 of the key and the
/// unmasked message, and `len` the message's length. Links Steam gives use a key; generated ones may use 0, which
/// leaves them unmasked.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EmbeddedItem {
    key: u8,
    data: Vec<u8>,
}

impl EmbeddedItem {
    /// The serialized `CEconItemPreviewDataBlock`, unmasked.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The serialized `CEconItemPreviewDataBlock`, unmasked.
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    /// Decodes the item, offline.
    pub fn details(&self) -> Result<ItemDetails, ItemDetailsError> {
        ItemDetails::from_bytes(&self.data)
    }

    fn from_hex(hex: &str) -> Result<Self, InspectLinkError> {
        let bytes = decode_hex(hex).ok_or(InspectLinkError::InvalidHex)?;
        // the key and the checksum
        if bytes.len() < 5 {
            return Err(InspectLinkError::TooShort);
        }
        let key = bytes[0];
        let (data, checksum) = bytes[1..].split_at(bytes.len() - 5);
        let item = Self {
            key,
            data: data.iter().map(|byte| byte ^ key).collect(),
        };
        let checksum = u32::from_be_bytes([
            checksum[0] ^ key,
            checksum[1] ^ key,
            checksum[2] ^ key,
            checksum[3] ^ key,
        ]);
        if checksum != item.checksum() {
            return Err(InspectLinkError::Checksum);
        }
        Ok(item)
    }

    fn checksum(&self) -> u32 {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&[self.key]);
        hasher.update(&self.data);
        let crc = hasher.finalize();
        (crc & 0xFFFF) ^ (self.data.len() as u32).wrapping_mul(crc)
    }
}

/// Debug shows the data in hex.
impl fmt::Debug for EmbeddedItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data: String = self.data.iter().map(|byte| format!("{:02X}", byte)).collect();
        f.debug_struct("EmbeddedItem")
            .field("key", &self.key)
            .field("data", &data)
            .finish()
    }
}

/// Formats as the argument of an inspect link: masked, in uppercase hex, as Steam gives.
impl Display for EmbeddedItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let checksum = self.checksum().to_be_bytes();
        let masked = self.data.iter().chain(&checksum).map(|byte| byte ^ self.key);
        std::iter::once(self.key)
            .chain(masked)
            .try_for_each(|byte| write!(f, "{:02X}", byte))
    }
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    let (pairs, odd) = hex.as_bytes().as_chunks::<2>();
    if !odd.is_empty() {
        return None;
    }
    pairs
        .iter()
        .map(|&[high, low]| Some(nibble(high)? << 4 | nibble(low)?))
        .collect()
}

fn nibble(digit: u8) -> Option<u8> {
    char::from(digit).to_digit(16).map(|nibble| nibble as u8)
}

/// Why a string isn't an inspect link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum InspectLinkError {
    /// The string isn't an inspect link of CS2: it's empty, another link or another command.
    #[error("Not a CS2 inspect link.")]
    NotInspectLink,
    /// The link has no `A`, or no `D`.
    #[error("The inspect link has no {0}.")]
    Missing(char),
    /// The number after this letter isn't one, or doesn't fit in 64 bits.
    #[error("The {0} of the inspect link is not a number.")]
    InvalidNumber(char),
    /// The `S` of the link isn't the SteamID of an individual account.
    #[error("The owner {0} of the inspect link is not an individual account.")]
    InvalidOwner(u64),
    /// The embedded item isn't hex of whole bytes.
    #[error("The embedded item of the inspect link is not hex.")]
    InvalidHex,
    /// The embedded item is too short to have its key and checksum.
    #[error("The embedded item of the inspect link is too short.")]
    TooShort,
    /// The embedded item's checksum doesn't match: the link is corrupt or cut short.
    #[error("The checksum of the embedded item of the inspect link does not match.")]
    Checksum,
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: u64 = 76561198084749846;
    const INVENTORY: &str = "steam://rungame/730/76561202255233023/+csgo_econ_action_preview%\
                             20S76561198084749846A698323590D7935523998312483177";
    const MARKET: &str = "steam://rungame/730/76561202255233023/+csgo_econ_action_preview%\
                          20M625254122282020305A6760346663D30614827701953021";

    /// An AK-47 with five stickers, key 0xE3, from vlydev/cs2-masked-inspect-csharp's tests.
    const AK47: &str = "E3F3367440334DE2FBE4C345E0CBE0D3E7DB6943400AE0A379E481ECEBE2F36FD9DE2BDB515EA6E30D74D981ECEBE3F37BCBDE640D475DA6E35EFCD881ECEBE3F359D5DE37E9D75DA6436DD3DD81ECEBE3F366DCDE3F8F9BDDA69B43B6DE81ECEBE3F33BC8DEBB1CA3DFA623F7DDDF8B71E293EBFD43382B";
    /// A link Steam gave, decoded on the node-globaloffensive forum (dev.doctormckay.com/topic/5926).
    const FORUM: &str = "steam://run/730//+csgo_econ_action_preview%\
                         2092826859045B28938A6E91B20991BA94A291AA016F546091D24895FAE5E29A90E5B003";
    /// A generated link, key 0, from vlydev/cs2-masked-inspect-csharp's tests.
    const GENERATED: &str = "00183C20B803280538E9A3C5DD0340E102C246A0D1";
    /// An S…A…D link with the item in place of the D number, from vlydev/cs2-masked-inspect-csharp's tests.
    const EMBEDDED_D: &str = "steam://rungame/730/76561202255233023/+csgo_econ_action_preview%20S76561199323320483A50075495125D1101C4C4FCD4AB10092D31B8143914211829A1FAE3FD125119591141117308191301EA550C1111912E3C111151D12C413E6BAC54D1D29BAD731E191501B92C2C9B6BF92F5411C25B2A731E191501B92C2CEA2B182E5411F7212A731E191501B92C2C4F89C12F549164592A799713611956F4339F";

    fn inventory() -> InspectLink {
        InspectLink::Inventory {
            owner: SteamID::from_steam64(OWNER),
            asset_id: 698323590,
            d: 7935523998312483177,
        }
    }

    fn market() -> InspectLink {
        InspectLink::Market {
            listing_id: 625254122282020305,
            asset_id: 6760346663,
            d: 30614827701953021,
        }
    }

    fn parse(link: &str) -> Result<InspectLink, InspectLinkError> {
        link.parse()
    }

    fn embedded(link: &str) -> EmbeddedItem {
        match parse(link).unwrap() {
            InspectLink::Embedded(item) => item,
            link => panic!("not embedded: {:?}", link),
        }
    }

    /// The varint fields of a message, enough to check what the items hold.
    fn varints(mut data: &[u8]) -> Vec<(u64, u64)> {
        fn varint(data: &mut &[u8]) -> u64 {
            let mut value = 0;
            for shift in (0..).step_by(7) {
                let byte = data[0];
                *data = &data[1..];
                value |= u64::from(byte & 0x7F) << shift;
                if byte & 0x80 == 0 {
                    break;
                }
            }
            value
        }
        let mut fields = Vec::new();
        while !data.is_empty() {
            let tag = varint(&mut data);
            match tag & 7 {
                0 => fields.push((tag >> 3, varint(&mut data))),
                2 => {
                    let len = varint(&mut data) as usize;
                    data = &data[len..];
                }
                5 => data = &data[4..],
                wire => panic!("wire type {}", wire),
            }
        }
        fields
    }

    fn field(item: &EmbeddedItem, number: u64) -> Option<u64> {
        varints(item.data())
            .into_iter()
            .find(|(n, _)| *n == number)
            .map(|(_, value)| value)
    }

    #[test]
    fn parses_inventory_and_market_links() {
        assert_eq!(parse(INVENTORY), Ok(inventory()));
        assert_eq!(parse(MARKET), Ok(market()));
    }

    #[test]
    fn parses_with_a_space_the_command_or_the_argument_alone() {
        for link in [INVENTORY, MARKET] {
            let expected = parse(link).unwrap();
            let (_, argument) = link.split_once("%20").unwrap();
            for link in [
                link.replace("%20", " "),
                format!("+csgo_econ_action_preview%20{}", argument),
                format!("+csgo_econ_action_preview {}", argument),
                format!("csgo_econ_action_preview {}", argument),
                argument.to_owned(),
                format!("  {}\n", link),
            ] {
                assert_eq!(parse(&link), Ok(expected.clone()), "{}", link);
            }
        }
    }

    #[test]
    fn round_trips() {
        assert_eq!(inventory().to_string(), INVENTORY);
        assert_eq!(market().to_string(), MARKET);
        for link in [AK47, GENERATED] {
            let item = parse(link).unwrap();
            assert_eq!(
                item.to_string(),
                format!("steam://run/730//+csgo_econ_action_preview%20{}", link)
            );
            assert_eq!(parse(&item.to_string()), Ok(item));
        }
        assert_eq!(parse(FORUM).unwrap().to_string(), FORUM);
    }

    #[test]
    fn gives_the_game_coordinator_params() {
        let params = inventory().params().unwrap();
        assert_eq!(
            params,
            PreviewParams {
                s: OWNER,
                a: 698323590,
                d: 7935523998312483177,
                m: 0
            }
        );
        let params = market().params().unwrap();
        assert_eq!(
            params,
            PreviewParams {
                s: 0,
                a: 6760346663,
                d: 30614827701953021,
                m: 625254122282020305
            }
        );
        assert!(parse(AK47).unwrap().params().is_err());
        assert_eq!(inventory().asset_id(), Some(698323590));
        assert_eq!(market().asset_id(), Some(6760346663));
        assert_eq!(parse(AK47).unwrap().asset_id(), None);
    }

    #[test]
    fn fills_templates() {
        let template = "steam://rungame/730/76561202255233023/+csgo_econ_action_preview%20S%owner_steamid%A%assetid%\
                        D7935523998312483177";
        let link = InspectLink::from_template(template, SteamID::from_steam64(OWNER), 698323590);
        assert_eq!(link, Ok(inventory()));
        // a link that names no owner, or embeds the item, has nothing to fill
        let embedded = format!("steam://run/730//+csgo_econ_action_preview%20{}", AK47);
        assert_eq!(
            InspectLink::from_template(MARKET, SteamID::from_steam64(OWNER), 1),
            Ok(market())
        );
        assert_eq!(
            InspectLink::from_template(&embedded, SteamID::from_steam64(OWNER), 1),
            parse(AK47)
        );
    }

    #[test]
    fn unmasks_embedded_items() {
        // CEconItemPreviewDataBlock: 2 itemid, 3 defindex, 4 paintindex, 7 paintwear (f32 bits), 8 paintseed
        let ak47 = embedded(AK47);
        assert_eq!(field(&ak47, 2), Some(46876117973));
        assert_eq!(field(&ak47, 3), Some(7));
        assert_eq!(field(&ak47, 4), Some(422));
        assert_eq!(field(&ak47, 8), Some(922));
        let wear = f32::from_bits(field(&ak47, 7).unwrap() as u32);
        assert!((wear - 0.04121).abs() < 0.0001, "{}", wear);

        let forum = embedded(FORUM);
        assert_eq!(field(&forum, 2), Some(50082457082));
        assert_eq!(field(&forum, 3), Some(508));
        assert_eq!(field(&forum, 4), Some(411));
        assert_eq!(field(&forum, 7), Some(1045544595));
        assert_eq!(field(&forum, 8), Some(986));

        let generated = embedded(GENERATED);
        assert_eq!(
            generated.data(),
            &decode_hex(&GENERATED[2..GENERATED.len() - 8]).unwrap()[..]
        );
        assert_eq!(field(&generated, 3), Some(60));
        assert_eq!(field(&generated, 4), Some(440));
        assert_eq!(field(&generated, 8), Some(353));

        // the item replaces the D number, and the item id is the A
        assert_eq!(field(&embedded(EMBEDDED_D), 2), Some(50075495125));
        // lowercase is hex too
        assert_eq!(embedded(&AK47.to_lowercase()), ak47);
    }

    #[test]
    fn unmasks_links_whose_key_starts_with_a_or_d() {
        // key 0xA6, from vlydev/cs2-masked-inspect-csharp's tests: defindex 34
        let item = embedded(
            "steam://run/730//+csgo_econ_action_preview%\
             20A6B6710C51510DA7BE848628A18EA396A29E1C181D56A5E682CEE8D6AEE7BC380F",
        );
        assert_eq!(field(&item, 3), Some(34));
        let item = EmbeddedItem {
            key: 0xD0,
            data: vec![0x18, 0x07],
        };
        assert!(item.to_string().starts_with('D'));
        assert_eq!(parse(&item.to_string()), Ok(InspectLink::Embedded(item)));
    }

    #[test]
    fn rejects_embedded_items_that_are_corrupt() {
        let mut corrupt = AK47.to_owned();
        corrupt.replace_range(10..11, if &AK47[10..11] == "0" { "1" } else { "0" });
        assert_eq!(parse(&corrupt), Err(InspectLinkError::Checksum));
        assert_eq!(parse(&AK47[..AK47.len() - 2]), Err(InspectLinkError::Checksum));
        assert_eq!(parse(&AK47[..AK47.len() - 1]), Err(InspectLinkError::InvalidHex));
        assert_eq!(parse(&format!("{}G0", AK47)), Err(InspectLinkError::InvalidHex));
        assert_eq!(parse("00AABBCC"), Err(InspectLinkError::TooShort));
        // the smallest: a key and the checksum of nothing
        let empty = EmbeddedItem {
            key: 0x5A,
            data: vec![],
        };
        assert_eq!(parse(&empty.to_string()), Ok(InspectLink::Embedded(empty)));
    }

    #[test]
    fn rejects_what_isnt_an_inspect_link() {
        use InspectLinkError::*;

        for (link, error) in [
            ("", NotInspectLink),
            ("   ", NotInspectLink),
            (
                "steam://rungame/730/76561202255233023/+csgo_econ_action_previe%20S1A2D3",
                NotInspectLink,
            ),
            (
                "steam://rungame/730/76561202255233023/+connect%20S1A2D3",
                NotInspectLink,
            ),
            (
                "steam://rungame/440/76561202255233023/+csgo_econ_action_preview%20S1A2D3",
                NotInspectLink,
            ),
            (
                "steam://rungame/730/76561202255233023/csgo_econ_action_preview%20S1A2D3",
                NotInspectLink,
            ),
            (
                "steam://rungame/730/7656x/+csgo_econ_action_preview%20S1A2D3",
                NotInspectLink,
            ),
            (
                "steam://rungame/730/76561202255233023/+csgo_econ_action_preview",
                NotInspectLink,
            ),
            (
                "steam://rungame/730/76561202255233023/+csgo_econ_action_previewS1A2D3",
                NotInspectLink,
            ),
            ("https://steamcommunity.com/market/listings/730/AK-47", NotInspectLink),
            ("+connect 127.0.0.1", NotInspectLink),
            ("X1A2D3", NotInspectLink),
            ("s76561198084749846A1D2", NotInspectLink),
            ("S76561198084749846D2", Missing('A')),
            ("S76561198084749846A1", Missing('D')),
            ("M1A2", Missing('D')),
            ("SA1D2", InvalidNumber('S')),
            ("S76561198084749846AD2", InvalidNumber('A')),
            ("S76561198084749846A1D", InvalidNumber('D')),
            ("M1x2A1D2", InvalidNumber('M')),
            ("M+1A1D2", InvalidNumber('M')),
            ("M1A-1D2", InvalidNumber('A')),
            ("M18446744073709551616A1D2", InvalidNumber('M')),
            ("M1A1D18446744073709551616", InvalidNumber('D')),
            ("M1A1D2 more", InvalidNumber('D')),
            ("M1A1D2x", InvalidNumber('D')),
            ("M1A1D2%20", InvalidNumber('D')),
            // the D isn't a number, so it's an embedded item, which this isn't
            ("M1A1D7935523998312483177abc", Checksum),
            ("S0A1D2", InvalidOwner(0)),
            // account 0, a clan, another universe
            ("S76561197960265728A1D2", InvalidOwner(76561197960265728)),
            ("S103582791429521412A1D2", InvalidOwner(103582791429521412)),
            ("S148618787703226646A1D2", InvalidOwner(148618787703226646)),
        ] {
            assert_eq!(parse(link), Err(error), "{:?}", link);
        }
        assert_eq!(
            parse("M18446744073709551615A0D0").unwrap().params().unwrap().m,
            u64::MAX
        );
    }
}
