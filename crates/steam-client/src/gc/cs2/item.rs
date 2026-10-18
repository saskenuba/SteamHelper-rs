//! What the game coordinator says of a CS2 item: its skin, pattern, float, stickers and keychains.

use std::convert::TryFrom;
use std::fmt;
use std::fmt::Formatter;

use steam_protobuf::protobuf;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::cecon_item_preview_data_block;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CEconItemPreviewDataBlock;
use steamid_parser::SteamID;
use thiserror::Error;

/// A CS2 item, as the game coordinator describes it: a `CEconItemPreviewDataBlock`, read as what a skin is.
///
/// Read from the game coordinator's answer to an inspect link, or offline from a link that embeds the item:
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use steam_client::gc::cs2::InspectLink;
///
/// let link: InspectLink = concat!(
///     "steam://run/730//+csgo_econ_action_preview",
///     "%2092826859045B28938A6E91B20991BA94A291AA016F546091D24895FAE5E29A90E5B003",
/// )
/// .parse()?;
/// let InspectLink::Embedded(item) = link else {
///     unreachable!("the link embeds its item");
/// };
/// let details = item.details()?;
/// assert_eq!(details.def_index(), 508);
/// assert_eq!(details.paint_index(), Some(411));
/// assert_eq!(details.pattern(), Some(986));
/// assert!((details.float().unwrap() - 0.2048).abs() < 0.0001);
/// # Ok(())
/// # }
/// ```
///
/// What the methods don't give, such as the music kit or the variations, is in [`raw`](Self::raw).
#[derive(Clone, PartialEq)]
pub struct ItemDetails {
    /// Has a def index that isn't 0.
    proto: CEconItemPreviewDataBlock,
}

impl ItemDetails {
    /// Reads a serialized `CEconItemPreviewDataBlock`.
    pub fn from_bytes(data: &[u8]) -> Result<Self, ItemDetailsError> {
        Self::try_from(CEconItemPreviewDataBlock::parse_from_bytes(data)?)
    }

    /// The item's id, its asset id in the inventory it's in. `None` for an item that doesn't exist, as a generated
    /// inspect link describes.
    pub fn item_id(&self) -> Option<u64> {
        self.proto.itemid.filter(|&id| id != 0)
    }

    /// What the item is: the weapon, knife, gloves, sticker, case… as numbered in the game's `items_game.txt`.
    pub fn def_index(&self) -> u32 {
        self.proto.defindex()
    }

    /// The skin (paint kit). `None` for an item without one, such as a vanilla knife, a sticker or a case.
    ///
    /// Doppler and Gamma Doppler phases are paint kits of their own: a Ruby's paint index differs from a Phase 2's.
    pub fn paint_index(&self) -> Option<u32> {
        self.proto.paintindex.filter(|&index| index != 0)
    }

    /// The pattern (paint seed), from 0 to 1000. `None` when the game coordinator gives none.
    pub fn pattern(&self) -> Option<u32> {
        self.proto.paintseed
    }

    /// The float (paint wear), from 0 to 1. `None` when the game coordinator gives none, as for items without a
    /// skin.
    pub fn float(&self) -> Option<f32> {
        self.proto.paintwear.map(f32::from_bits)
    }

    /// The rarity, such as 1 for Consumer Grade up to 6 for Covert and 7 for Contraband.
    pub fn rarity(&self) -> Option<u32> {
        self.proto.rarity
    }

    /// The quality, such as 4 for a plain item, 9 for StatTrak, 12 for Souvenir and 3 for a ★ knife or gloves.
    pub fn quality(&self) -> Option<u32> {
        self.proto.quality
    }

    /// How the item came to be, such as 0 for a drop or 8 for a case opening.
    pub fn origin(&self) -> Option<u32> {
        self.proto.origin
    }

    /// The StatTrak counter, if the item has one.
    pub fn stat_trak(&self) -> Option<StatTrak> {
        Some(StatTrak {
            score_type: self.proto.killeaterscoretype(),
            value: self.proto.killeatervalue?,
        })
    }

    /// The name tag's name.
    pub fn custom_name(&self) -> Option<&str> {
        self.proto
            .customnames
            .first()
            .map(String::as_str)
            .filter(|name| !name.is_empty())
    }

    /// The stickers on the item, or the patches on an agent.
    pub fn stickers(&self) -> impl ExactSizeIterator<Item = Sticker> + '_ {
        self.proto.stickers.iter().map(Sticker::from_proto)
    }

    /// The keychains (charms) on the item.
    pub fn keychains(&self) -> impl ExactSizeIterator<Item = Keychain> + '_ {
        self.proto.keychains.iter().map(Keychain::from_proto)
    }

    /// The account whose inventory the item is in. `None` when the game coordinator doesn't say, as for an item on
    /// the market.
    pub fn owner(&self) -> Option<SteamID> {
        self.proto.accountid.filter(|&id| id != 0).map(SteamID::from_account_id)
    }

    /// The message the game coordinator gave.
    pub fn raw(&self) -> &CEconItemPreviewDataBlock {
        &self.proto
    }

    /// The message the game coordinator gave.
    pub fn into_proto(self) -> CEconItemPreviewDataBlock {
        self.proto
    }
}

/// Fails when the message names no item: it has no def index, or 0.
impl TryFrom<CEconItemPreviewDataBlock> for ItemDetails {
    type Error = ItemDetailsError;

    fn try_from(proto: CEconItemPreviewDataBlock) -> Result<Self, Self::Error> {
        if proto.defindex() == 0 {
            return Err(ItemDetailsError::NoDefIndex);
        }
        Ok(Self { proto })
    }
}

/// Debug shows what the methods give.
impl fmt::Debug for ItemDetails {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ItemDetails")
            .field("item_id", &self.item_id())
            .field("def_index", &self.def_index())
            .field("paint_index", &self.paint_index())
            .field("pattern", &self.pattern())
            .field("float", &self.float())
            .field("rarity", &self.rarity())
            .field("quality", &self.quality())
            .field("origin", &self.origin())
            .field("stat_trak", &self.stat_trak())
            .field("custom_name", &self.custom_name())
            .field("stickers", &self.stickers().collect::<Vec<_>>())
            .field("keychains", &self.keychains().collect::<Vec<_>>())
            .field("owner", &self.owner())
            .finish()
    }
}

/// A StatTrak counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct StatTrak {
    /// What it counts: 0 for kills, which weapons count.
    pub score_type: u32,
    /// The count.
    pub value: u32,
}

/// A sticker on an item, or a patch on an agent.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Sticker {
    /// Where it is, from 0. 0 when the game coordinator doesn't say.
    pub slot: u32,
    /// What sticker it is, as numbered in `items_game.txt`.
    pub id: u32,
    /// How scraped it is, from 0 to 1. `None` for a sticker not scraped.
    pub wear: Option<f32>,
    /// Its scale, when not the default.
    pub scale: Option<f32>,
    /// Its rotation, when not the default.
    pub rotation: Option<f32>,
    /// Where on the item it is, when the game coordinator says.
    pub offset_x: Option<f32>,
    /// Where on the item it is, when the game coordinator says.
    pub offset_y: Option<f32>,
    /// Where on the item it is, when the game coordinator says.
    pub offset_z: Option<f32>,
}

impl Sticker {
    fn from_proto(sticker: &cecon_item_preview_data_block::Sticker) -> Self {
        Self {
            slot: sticker.slot(),
            id: sticker.sticker_id(),
            wear: sticker.wear,
            scale: sticker.scale,
            rotation: sticker.rotation,
            offset_x: sticker.offset_x,
            offset_y: sticker.offset_y,
            offset_z: sticker.offset_z,
        }
    }
}

/// A keychain (charm) on an item.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Keychain {
    /// Where it is, from 0. 0 when the game coordinator doesn't say.
    pub slot: u32,
    /// What keychain it is, as numbered in `items_game.txt`.
    pub id: u32,
    /// Its pattern (seed), which some keychains vary by.
    pub pattern: Option<u32>,
    /// Where on the item it hangs, when the game coordinator says.
    pub offset_x: Option<f32>,
    /// Where on the item it hangs, when the game coordinator says.
    pub offset_y: Option<f32>,
    /// Where on the item it hangs, when the game coordinator says.
    pub offset_z: Option<f32>,
    /// The highlight a souvenir charm plays.
    pub highlight_reel: Option<u32>,
    /// The sticker a sticker slab holds.
    pub wrapped_sticker: Option<u32>,
}

impl Keychain {
    fn from_proto(keychain: &cecon_item_preview_data_block::Sticker) -> Self {
        Self {
            slot: keychain.slot(),
            id: keychain.sticker_id(),
            pattern: keychain.pattern,
            offset_x: keychain.offset_x,
            offset_y: keychain.offset_y,
            offset_z: keychain.offset_z,
            highlight_reel: keychain.highlight_reel,
            wrapped_sticker: keychain.wrapped_sticker,
        }
    }
}

/// Why a message isn't an item's details.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ItemDetailsError {
    /// The data isn't a `CEconItemPreviewDataBlock`.
    #[error("The item details could not be decoded: {0}")]
    Decode(#[from] protobuf::Error),
    /// The message names no item: it has no def index.
    #[error("The item details name no item: they have no def index.")]
    NoDefIndex,
    /// The game coordinator answered an inspect link without an item.
    #[error("The game coordinator answered without an item.")]
    NoItem,
}

#[cfg(test)]
mod tests {
    use steam_protobuf::protobuf::MessageField;

    use super::*;
    use crate::gc::cs2::InspectLink;

    type ProtoSticker = cecon_item_preview_data_block::Sticker;

    /// The forum sample of `link.rs`: an M9 Bayonet.
    const FORUM: &str = "steam://run/730//+csgo_econ_action_preview%\
                         2092826859045B28938A6E91B20991BA94A291AA016F546091D24895FAE5E29A90E5B003";
    /// vlydev's AK-47 of `link.rs`, with five stickers.
    const AK47: &str = "E3F3367440334DE2FBE4C345E0CBE0D3E7DB6943400AE0A379E481ECEBE2F36FD9DE2BDB515EA6E30D74D981ECEBE3F37BCBDE640D475DA6E35EFCD881ECEBE3F359D5DE37E9D75DA6436DD3DD81ECEBE3F366DCDE3F8F9BDDA69B43B6DE81ECEBE3F33BC8DEBB1CA3DFA623F7DDDF8B71E293EBFD43382B";
    /// The generated link of `link.rs`: key 0, no item id.
    const GENERATED: &str = "00183C20B803280538E9A3C5DD0340E102C246A0D1";

    fn details(link: &str) -> ItemDetails {
        match link.parse().unwrap() {
            InspectLink::Embedded(item) => item.details().unwrap(),
            link => panic!("not embedded: {:?}", link),
        }
    }

    fn close(value: Option<f32>, expected: f32) -> bool {
        value.is_some_and(|value| (value - expected).abs() < 0.0001)
    }

    fn full() -> CEconItemPreviewDataBlock {
        let mut sticker = ProtoSticker::new();
        sticker.set_slot(2);
        sticker.set_sticker_id(5032);
        sticker.set_wear(0.25);
        sticker.set_scale(1.5);
        sticker.set_rotation(-30.0);
        sticker.set_tint_id(3);
        sticker.set_offset_x(0.1);
        sticker.set_offset_y(-0.2);
        sticker.set_offset_z(0.3);
        let mut keychain = ProtoSticker::new();
        keychain.set_slot(0);
        keychain.set_sticker_id(36);
        keychain.set_offset_x(10.5);
        keychain.set_offset_y(0.5);
        keychain.set_offset_z(-1.0);
        keychain.set_pattern(71234);
        keychain.set_highlight_reel(4);
        keychain.set_wrapped_sticker(5032);
        let mut variation = ProtoSticker::new();
        variation.set_sticker_id(9);

        let mut proto = CEconItemPreviewDataBlock::new();
        proto.set_accountid(124484118);
        proto.set_itemid(46876117973);
        proto.set_defindex(7);
        proto.set_paintindex(422);
        proto.set_rarity(5);
        proto.set_quality(9);
        proto.set_paintwear(1045544595);
        proto.set_paintseed(922);
        proto.set_killeaterscoretype(0);
        proto.set_killeatervalue(1337);
        proto.customnames = vec!["Old Faithful".to_owned(), "ignored".to_owned()];
        proto.stickers = vec![sticker];
        proto.set_inventory(3);
        proto.set_origin(8);
        proto.set_questid(1);
        proto.set_dropreason(2);
        proto.set_musicindex(3);
        proto.set_entindex(-1);
        proto.set_petindex(4);
        proto.keychains = vec![keychain];
        proto.set_style(5);
        proto.variations = vec![variation];
        proto.set_upgrade_level(6);
        proto.set_pet_food_expiration_date(7);
        proto.set_blobdata(vec![1, 2, 3].into());
        proto
    }

    #[test]
    fn reads_every_field() {
        let proto = full();
        let item = ItemDetails::try_from(proto.clone()).unwrap();
        assert_eq!(item.item_id(), Some(46876117973));
        assert_eq!(item.def_index(), 7);
        assert_eq!(item.paint_index(), Some(422));
        assert_eq!(item.pattern(), Some(922));
        assert!(close(item.float(), 0.2048), "{:?}", item.float());
        assert_eq!(item.rarity(), Some(5));
        assert_eq!(item.quality(), Some(9));
        assert_eq!(item.origin(), Some(8));
        assert_eq!(
            item.stat_trak(),
            Some(StatTrak {
                score_type: 0,
                value: 1337
            })
        );
        assert_eq!(item.custom_name(), Some("Old Faithful"));
        assert_eq!(
            item.stickers().collect::<Vec<_>>(),
            [Sticker {
                slot: 2,
                id: 5032,
                wear: Some(0.25),
                scale: Some(1.5),
                rotation: Some(-30.0),
                offset_x: Some(0.1),
                offset_y: Some(-0.2),
                offset_z: Some(0.3),
            }]
        );
        assert_eq!(
            item.keychains().collect::<Vec<_>>(),
            [Keychain {
                slot: 0,
                id: 36,
                pattern: Some(71234),
                offset_x: Some(10.5),
                offset_y: Some(0.5),
                offset_z: Some(-1.0),
                highlight_reel: Some(4),
                wrapped_sticker: Some(5032),
            }]
        );
        assert_eq!(item.owner(), Some(SteamID::from_account_id(124484118)));
        // nothing is lost
        assert_eq!(item.raw(), &proto);
        assert_eq!(ItemDetails::from_bytes(&proto.write_to_bytes().unwrap()).unwrap(), item);
        assert_eq!(item.into_proto(), proto);
    }

    #[test]
    fn reads_an_item_with_only_a_def_index() {
        let mut proto = CEconItemPreviewDataBlock::new();
        proto.set_defindex(4001);
        let item = ItemDetails::try_from(proto).unwrap();
        assert_eq!(item.item_id(), None);
        assert_eq!(item.def_index(), 4001);
        assert_eq!(item.paint_index(), None);
        assert_eq!(item.pattern(), None);
        assert_eq!(item.float(), None);
        assert_eq!(item.rarity(), None);
        assert_eq!(item.quality(), None);
        assert_eq!(item.origin(), None);
        assert_eq!(item.stat_trak(), None);
        assert_eq!(item.custom_name(), None);
        assert_eq!(item.stickers().len(), 0);
        assert_eq!(item.keychains().len(), 0);
        assert_eq!(item.owner(), None);
    }

    #[test]
    fn tells_unset_from_zero_where_zero_is_a_value() {
        let mut proto = CEconItemPreviewDataBlock::new();
        proto.set_defindex(7);
        proto.set_itemid(0);
        proto.set_paintindex(0);
        proto.set_accountid(0);
        proto.set_paintseed(0);
        proto.set_paintwear(0);
        proto.set_origin(0);
        proto.set_killeatervalue(0);
        proto.customnames = vec![String::new()];
        proto.stickers = vec![ProtoSticker::new()];
        let item = ItemDetails::try_from(proto).unwrap();
        // 0 names no item, no skin, no account
        assert_eq!(item.item_id(), None);
        assert_eq!(item.paint_index(), None);
        assert_eq!(item.owner(), None);
        assert_eq!(item.custom_name(), None);
        // but is a pattern, a float, an origin, a fresh StatTrak counting kills
        assert_eq!(item.pattern(), Some(0));
        assert_eq!(item.float(), Some(0.0));
        assert_eq!(item.origin(), Some(0));
        assert_eq!(
            item.stat_trak(),
            Some(StatTrak {
                score_type: 0,
                value: 0
            })
        );
        let sticker = item.stickers().next().unwrap();
        assert_eq!((sticker.slot, sticker.id, sticker.wear), (0, 0, None));
    }

    #[test]
    fn rejects_what_names_no_item() {
        let mut proto = CEconItemPreviewDataBlock::new();
        proto.set_itemid(1);
        assert!(matches!(
            ItemDetails::try_from(proto.clone()),
            Err(ItemDetailsError::NoDefIndex)
        ));
        proto.set_defindex(0);
        assert!(matches!(
            ItemDetails::try_from(proto),
            Err(ItemDetailsError::NoDefIndex)
        ));
        assert!(matches!(
            ItemDetails::from_bytes(&[]),
            Err(ItemDetailsError::NoDefIndex)
        ));
        // field 3 (def index), varint, cut short
        assert!(matches!(
            ItemDetails::from_bytes(&[0x18, 0x80]),
            Err(ItemDetailsError::Decode(_))
        ));
    }

    #[test]
    fn decodes_embedded_items() {
        let forum = details(FORUM);
        assert_eq!(forum.item_id(), Some(50082457082));
        assert_eq!(forum.def_index(), 508);
        assert_eq!(forum.paint_index(), Some(411));
        assert_eq!(forum.raw().paintwear, Some(1045544595));
        assert!(close(forum.float(), 0.2048), "{:?}", forum.float());
        assert_eq!(forum.pattern(), Some(986));

        let ak47 = details(AK47);
        assert_eq!(ak47.item_id(), Some(46876117973));
        assert_eq!(ak47.def_index(), 7);
        assert_eq!(ak47.paint_index(), Some(422));
        assert_eq!(ak47.pattern(), Some(922));
        assert!(close(ak47.float(), 0.04121), "{:?}", ak47.float());
        assert_eq!(ak47.rarity(), Some(3));
        assert_eq!(ak47.quality(), Some(4));
        assert_eq!(ak47.origin(), Some(8));
        assert_eq!(ak47.stat_trak(), None);
        // placed freely: slot 1, then four in slot 0, by offset
        let stickers: Vec<_> = ak47.stickers().map(|sticker| (sticker.slot, sticker.id)).collect();
        assert_eq!(stickers, [(1, 7436), (0, 5144), (0, 6970), (0, 8069), (0, 5592)]);
        let first = ak47.stickers().next().unwrap();
        assert_eq!((first.wear, first.scale, first.rotation), (None, None, None));
        assert!(close(first.offset_x, -0.08702), "{:?}", first);
        assert!(close(first.offset_y, 0.00116), "{:?}", first);
        assert_eq!(first.offset_z, None);

        // a generated item exists nowhere
        let generated = details(GENERATED);
        assert_eq!(generated.item_id(), None);
        assert_eq!(generated.def_index(), 60);
        assert_eq!(generated.paint_index(), Some(440));
        assert_eq!(generated.pattern(), Some(353));
    }

    #[test]
    fn decodes_the_item_of_a_preview_response() {
        use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse;

        let mut response = CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse::new();
        response.iteminfo = MessageField::some(full());
        let response = CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse::parse_from_bytes(
            &response.write_to_bytes().unwrap(),
        )
        .unwrap();
        let item = ItemDetails::try_from(response.iteminfo.unwrap()).unwrap();
        assert_eq!(item.raw(), &full());
    }
}
