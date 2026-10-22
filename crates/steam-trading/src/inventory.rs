//! A Steam Community inventory (`/inventory/{steamid}/{appid}/{contextid}`), typed.
//!
//! The shape is the one seen live on 2026-10-20 (`fixtures/live/2026-10-20/`): `assets`, their `descriptions` (by
//! `classid` and `instanceid`), and CS2's `asset_properties` (pattern, wear). An empty inventory has none of the three.
//!
//! Reading a page allocates nothing per item: one serde_json pass borrows from the body, so its lists grow a few times
//! per page and that is all (`tests/zero_alloc.rs`). Texts are borrowed, unless Steam escaped something in them. An
//! item's properties and owner-only texts stay raw JSON until asked for.

use std::borrow::Cow;
#[cfg(feature = "cs2")]
use std::collections::HashMap;
#[cfg(feature = "cs2")]
use std::convert::TryFrom;
use std::fmt;
use std::future::Future;

use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use serde_json::value::RawValue;

#[cfg(feature = "cs2")]
use crate::Fingerprint;
use crate::InventoryContext;
use crate::TradeError;

/// At most this many inventory pages are read in one call, a guard against a page chain that never ends.
pub(crate) const MAX_INVENTORY_PAGES: usize = 100;

/// Items asked for per page, as the full_trade example asked live.
const PAGE_SIZE: u32 = 1000;

const PATTERN_PROPERTY: u32 = 1;
const WEAR_PROPERTY: u32 = 2;

/// An inventory as Steam sent it: one body per page, each checked when it came.
///
/// Its items borrow from it: read them with [`Inventory::pages`] and [`InventoryPage::items`].
#[derive(Default)]
pub struct Inventory {
    pages: Vec<String>,
}

impl Inventory {
    /// One page, such as a body kept from an earlier read.
    ///
    /// # Errors
    ///
    /// As [`InventoryPage::parse`].
    pub fn from_json(body: String) -> Result<Self, TradeError> {
        InventoryPage::parse(&body)?;
        Ok(Self { pages: vec![body] })
    }

    /// The pages, read again from their bodies: one serde_json pass each.
    pub fn pages(&self) -> impl Iterator<Item = InventoryPage<'_>> + '_ {
        // checked when they came, so they read the same now
        self.pages.iter().filter_map(|body| InventoryPage::parse(body).ok())
    }

    /// How many items Steam says the inventory has, from the first page: compare it with the items of every page to
    /// tell a complete read. `None` if Steam didn't say.
    pub fn total_inventory_count(&self) -> Option<u32> {
        self.pages().next()?.total_inventory_count()
    }

    /// The [`Fingerprint`] of each item with a pattern and a wear, by its asset id. Feature `cs2`.
    ///
    /// One call after reading the partner's inventory
    /// ([`SteamTradeManager::inventory`](crate::SteamTradeManager::inventory)) gives the fingerprints of an offer's
    /// items: extra entries don't matter, they are looked up by the offer's asset ids.
    #[cfg(feature = "cs2")]
    pub fn fingerprints(&self) -> HashMap<u64, Fingerprint> {
        let mut fingerprints = HashMap::new();
        for page in self.pages() {
            for item in page.items() {
                if let (Ok(assetid), Some(fingerprint)) = (u64::try_from(item.assetid), Fingerprint::of(&item)) {
                    fingerprints.insert(assetid, fingerprint);
                }
            }
        }
        fingerprints
    }
}

impl fmt::Debug for Inventory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inventory").field("pages", &self.pages.len()).finish()
    }
}

/// One page of an inventory, borrowed from Steam's answer.
#[derive(Debug)]
pub struct InventoryPage<'a> {
    assets: Vec<RawAsset>,
    /// Sorted by `classid`, then `instanceid`.
    descriptions: Vec<RawDescription<'a>>,
    /// Sorted by asset id.
    properties: Vec<RawProperties<'a>>,
    next_start: Option<&'a str>,
    total_inventory_count: Option<&'a RawValue>,
}

impl<'a> InventoryPage<'a> {
    /// Reads Steam's answer.
    ///
    /// # Errors
    ///
    /// [`TradeError::GeneralError`] if it isn't an inventory, Steam doesn't say `success: 1`, or it says more items
    /// come but not where.
    pub fn parse(body: &'a str) -> Result<Self, TradeError> {
        let page: RawPage<'a> = serde_json::from_str(body)
            .map_err(|error| TradeError::GeneralError(format!("Steam's inventory answer can't be read: {}", error)))?;
        if !page.success {
            return Err(TradeError::GeneralError(
                "Steam's inventory answer doesn't say `success: 1`.".to_string(),
            ));
        }
        let next_start = match (page.more_items, page.last_assetid) {
            (false, _) => None,
            (true, Some(last)) => Some(last),
            (true, None) => {
                return Err(TradeError::GeneralError(
                    "Steam says the inventory has more items, but not where they start: items would be missing."
                        .to_string(),
                ))
            }
        };

        let mut descriptions = page.descriptions;
        descriptions.sort_unstable_by_key(|description| (description.classid, description.instanceid));
        let mut properties = page.asset_properties;
        properties.sort_unstable_by_key(|properties| properties.assetid);
        Ok(Self {
            assets: page.assets,
            descriptions,
            properties,
            next_start,
            total_inventory_count: page.total_inventory_count,
        })
    }

    /// The page's items, each with its description and properties. Allocates nothing.
    pub fn items(&self) -> impl Iterator<Item = InventoryItem<'_>> + '_ {
        self.assets.iter().map(move |asset| {
            let description = self
                .descriptions
                .binary_search_by_key(&(asset.classid, asset.instanceid), |d| (d.classid, d.instanceid))
                .ok()
                .map(|index| &self.descriptions[index]);
            let properties = self
                .properties
                .binary_search_by_key(&asset.assetid, |properties| properties.assetid)
                .ok()
                .and_then(|index| self.properties[index].asset_properties);
            InventoryItem {
                appid: asset.appid,
                contextid: asset.contextid,
                assetid: asset.assetid,
                classid: asset.classid,
                instanceid: asset.instanceid,
                amount: asset.amount,
                market_hash_name: description.map_or("", |d| &d.market_hash_name),
                tradable: description.is_some_and(|d| d.tradable),
                marketable: description.is_some_and(|d| d.marketable),
                commodity: description.is_some_and(|d| d.commodity),
                owner_descriptions: description.and_then(|d| d.owner_descriptions),
                properties,
            }
        })
    }

    /// How many items the page has.
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Where the next page starts (Steam's `last_assetid`), `None` after the last page.
    pub fn next_start(&self) -> Option<&'a str> {
        self.next_start
    }

    /// How many items Steam says the whole inventory has (`total_inventory_count`, on every page), `None` if it didn't
    /// say or not as a count.
    pub fn total_inventory_count(&self) -> Option<u32> {
        let raw = self.total_inventory_count?.get();
        serde_json::from_str(raw)
            .ok()
            .or_else(|| serde_json::from_str::<&str>(raw).ok()?.parse().ok())
    }
}

/// An item of an inventory page.
#[derive(Debug, Clone, Copy)]
pub struct InventoryItem<'p> {
    pub appid: u32,
    pub contextid: u32,
    pub assetid: i64,
    /// The item's kind. It stays the same when the item is traded; `instanceid` doesn't (seen live, CS2).
    pub classid: i64,
    pub instanceid: i64,
    pub amount: i64,
    /// Empty if Steam sent no description for the item.
    pub market_hash_name: &'p str,
    pub tradable: bool,
    pub marketable: bool,
    /// Copies of a commodity (a case, a graffiti) are identical: no pattern, no wear.
    pub commodity: bool,
    owner_descriptions: Option<&'p RawValue>,
    properties: Option<&'p RawValue>,
}

impl<'p> InventoryItem<'p> {
    /// CS2's pattern ("Pattern Template", property 1). Allocates nothing.
    pub fn pattern(&self) -> Option<i64> {
        self.property(PATTERN_PROPERTY)?.int_value?.parse().ok()
    }

    /// CS2's wear, the float ("Wear Rating", property 2). It is the game coordinator's 32-bit float written out in
    /// full: `wear() as f32` is the game coordinator's value. Allocates nothing.
    pub fn wear(&self) -> Option<f64> {
        self.property(WEAR_PROPERTY)?.float_value?.parse().ok()
    }

    /// Steam's `asset_properties` for the item: CS2's pattern, wear and certificate. Read when called.
    pub fn properties(&self) -> Vec<AssetProperty<'p>> {
        self.properties
            .and_then(|raw| serde_json::from_str::<Vec<RawProperty<'p>>>(raw.get()).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(RawProperty::into_property)
            .collect()
    }

    /// The non-blank texts only the owner sees, such as the end of a trade protection: "⇆ This item is
    /// trade-protected and cannot be consumed, modified, or transferred until 27 Oct @ 9:00am" (US Pacific, no year).
    /// Read when called.
    pub fn owner_descriptions(&self) -> Vec<Cow<'p, str>> {
        self.owner_descriptions
            .and_then(|raw| serde_json::from_str::<Vec<RawText<'p>>>(raw.get()).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|text| text.value)
            .filter(|text| !text.trim().is_empty())
            .collect()
    }

    /// The property `id`, found without allocating.
    fn property(&self, id: u32) -> Option<RawProperty<'p>> {
        let raw = self.properties?;
        let mut deserializer = serde_json::Deserializer::from_str(raw.get());
        deserializer.deserialize_seq(FindProperty(id)).ok()?
    }
}

/// One of Steam's `asset_properties` of an item.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetProperty<'p> {
    /// Steam's `propertyid`: 1 pattern, 2 wear, 6 certificate for CS2.
    pub id: u32,
    /// Steam's name for it, in English: "Pattern Template", "Wear Rating"…
    pub name: Cow<'p, str>,
    pub value: PropertyValue<'p>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue<'p> {
    Int(i64),
    Float(f64),
    String(Cow<'p, str>),
}

#[derive(Debug, Deserialize)]
struct RawPage<'a> {
    #[serde(default, deserialize_with = "flag")]
    success: bool,
    #[serde(default)]
    assets: Vec<RawAsset>,
    #[serde(default, borrow)]
    descriptions: Vec<RawDescription<'a>>,
    #[serde(default, borrow)]
    asset_properties: Vec<RawProperties<'a>>,
    #[serde(default, deserialize_with = "flag")]
    more_items: bool,
    #[serde(default, borrow)]
    last_assetid: Option<&'a str>,
    /// Read on demand: a count Steam sends in another shape leaves the page readable.
    #[serde(default, borrow)]
    total_inventory_count: Option<&'a RawValue>,
}

#[derive(Debug, Deserialize)]
struct RawAsset {
    appid: u32,
    #[serde(with = "serde_with::rust::display_fromstr")]
    contextid: u32,
    #[serde(with = "serde_with::rust::display_fromstr")]
    assetid: i64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    classid: i64,
    #[serde(default, with = "serde_with::rust::display_fromstr")]
    instanceid: i64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    amount: i64,
}

#[derive(Debug, Deserialize)]
struct RawDescription<'a> {
    #[serde(with = "serde_with::rust::display_fromstr")]
    classid: i64,
    #[serde(default, with = "serde_with::rust::display_fromstr")]
    instanceid: i64,
    #[serde(default, borrow)]
    market_hash_name: Cow<'a, str>,
    #[serde(default, deserialize_with = "flag")]
    tradable: bool,
    #[serde(default, deserialize_with = "flag")]
    marketable: bool,
    #[serde(default, deserialize_with = "flag")]
    commodity: bool,
    #[serde(default, borrow)]
    owner_descriptions: Option<&'a RawValue>,
}

#[derive(Debug, Deserialize)]
struct RawText<'a> {
    #[serde(default, borrow)]
    value: Cow<'a, str>,
}

#[derive(Debug, Deserialize)]
struct RawProperties<'a> {
    #[serde(with = "serde_with::rust::display_fromstr")]
    assetid: i64,
    #[serde(default, borrow)]
    asset_properties: Option<&'a RawValue>,
}

#[derive(Debug, Deserialize)]
struct RawProperty<'a> {
    propertyid: u32,
    #[serde(default, borrow)]
    name: Cow<'a, str>,
    #[serde(default, borrow)]
    int_value: Option<&'a str>,
    #[serde(default, borrow)]
    float_value: Option<&'a str>,
    /// Raw: serde copies a `Cow` inside an `Option` (it borrows only a bare one), and `pattern()` and `wear()` must
    /// not allocate. Decoded by [`RawProperty::into_property`].
    #[serde(default, borrow)]
    string_value: Option<&'a RawValue>,
}

impl<'a> RawProperty<'a> {
    /// `None` for a property without a value Steam's way (or one that isn't a number where it says it is one).
    fn into_property(self) -> Option<AssetProperty<'a>> {
        let value = match (self.int_value, self.float_value, self.string_value) {
            (Some(int), _, _) => PropertyValue::Int(int.parse().ok()?),
            (None, Some(float), _) => PropertyValue::Float(float.parse().ok()?),
            (None, None, Some(string)) => PropertyValue::String(text(string)?),
            (None, None, None) => return None,
        };
        Some(AssetProperty {
            id: self.propertyid,
            name: self.name,
            value,
        })
    }
}

/// A JSON string, borrowed unless Steam escaped something in it.
fn text(raw: &RawValue) -> Option<Cow<'_, str>> {
    match serde_json::from_str::<&str>(raw.get()) {
        Ok(text) => Some(Cow::Borrowed(text)),
        Err(_) => serde_json::from_str::<String>(raw.get()).ok().map(Cow::Owned),
    }
}

/// The first property with an id, from a property list, without keeping the others.
struct FindProperty(u32);

impl<'de> de::Visitor<'de> for FindProperty {
    type Value = Option<RawProperty<'de>>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a list of asset properties")
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut found = None;
        while let Some(property) = seq.next_element::<RawProperty<'de>>()? {
            if found.is_none() && property.propertyid == self.0 {
                found = Some(property);
            }
        }
        Ok(found)
    }
}

/// Steam's 0/1 flags, also taken as booleans.
fn flag<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    struct Flag;

    impl<'de> de::Visitor<'de> for Flag {
        type Value = bool;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("0, 1, or a boolean")
        }

        fn visit_bool<E: de::Error>(self, flag: bool) -> Result<bool, E> {
            Ok(flag)
        }

        fn visit_u64<E: de::Error>(self, number: u64) -> Result<bool, E> {
            Ok(number == 1)
        }

        fn visit_i64<E: de::Error>(self, number: i64) -> Result<bool, E> {
            Ok(number == 1)
        }

        fn visit_str<E: de::Error>(self, text: &str) -> Result<bool, E> {
            Ok(text == "1" || text == "true")
        }

        fn visit_unit<E: de::Error>(self) -> Result<bool, E> {
            Ok(false)
        }
    }

    deserializer.deserialize_any(Flag)
}

/// The URL of one inventory page: the first is the request the full_trade example sent live, the next ones add
/// `start_assetid` (as node-steamcommunity does; not seen live).
pub(crate) fn inventory_url(steam64: u64, context: InventoryContext, start_assetid: Option<&str>) -> String {
    let mut url = format!(
        "https://steamcommunity.com/inventory/{}/{}/{}?l=english&count={}",
        steam64,
        context.appid(),
        context.contextid(),
        PAGE_SIZE
    );
    if let Some(start) = start_assetid {
        url.push_str("&start_assetid=");
        url.push_str(start);
    }
    url
}

/// Reads the first page body with `page(None)`, then the next ones while Steam says there are more. Every page or an
/// error: one that can't be read, a repeated start or more than [`MAX_INVENTORY_PAGES`] pages fail, since items would
/// be missing.
pub(crate) async fn read_all_inventory_pages<F, Fut>(mut page: F) -> Result<Inventory, TradeError>
where
    F: FnMut(Option<String>) -> Fut,
    Fut: Future<Output = Result<String, TradeError>>,
{
    let mut inventory = Inventory::default();
    let mut starts = Vec::new();
    let mut start = None;

    loop {
        let body = page(start.take()).await?;
        let next = InventoryPage::parse(&body)?.next_start().map(String::from);
        inventory.pages.push(body);
        let next = match next {
            Some(next) => next,
            None => return Ok(inventory),
        };
        if inventory.pages.len() >= MAX_INVENTORY_PAGES {
            return Err(TradeError::GeneralError(format!(
                "The inventory has more than {} pages: items would be missing.",
                MAX_INVENTORY_PAGES
            )));
        }
        if starts.contains(&next) {
            return Err(TradeError::GeneralError(format!(
                "Steam sent the inventory page after asset `{}` twice: items would be missing.",
                next
            )));
        }
        starts.push(next.clone());
        start = Some(next);
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::*;

    /// Part of B's context 2 in `fixtures/live/2026-10-20/ack-cycle.jsonl` (line 61): a case and a skin.
    const CONTEXT_2: &str = r#"{"assets":[
      {"appid":730,"contextid":"2","assetid":"25838682267","classid":"4717330486","instanceid":"0","amount":"1"},
      {"appid":730,"contextid":"2","assetid":"24936074294","classid":"7993036706","instanceid":"480085569","amount":"1"}],
     "descriptions":[
      {"appid":730,"classid":"4717330486","instanceid":"0","tradable":1,"market_hash_name":"Dreams & Nightmares Case",
       "commodity":1,"marketable":1},
      {"appid":730,"classid":"7993036706","instanceid":"480085569","tradable":1,
       "market_hash_name":"G3SG1 | Green Apple (Factory New)","commodity":0,"marketable":1}],
     "asset_properties":[{"appid":730,"contextid":"2","assetid":"24936074294","asset_properties":[
      {"propertyid":1,"int_value":"355","name":"Pattern Template"},
      {"propertyid":2,"float_value":"0.0661438107490539551","name":"Wear Rating"},
      {"propertyid":6,"string_value":"8292341A3B70DE9A89A22480AA80B286BA6A6E1F6E81C26180EA010202028EF2860B8E4C98",
       "name":"Item Certificate"}]}],
     "total_inventory_count":2,"success":1,"rwgrsn":-2}"#;

    /// B's context 16 after the trade (line 62): the received, trade-protected item.
    const CONTEXT_16: &str = r#"{"assets":[
      {"appid":730,"contextid":"16","assetid":"54238728503","classid":"241575744","instanceid":"9119903511","amount":"1"}],
     "descriptions":[{"appid":730,"classid":"241575744","instanceid":"9119903511","tradable":0,
       "owner_descriptions":[{"type":"html","value":" "},{"type":"html",
         "value":"⇆ This item is trade-protected and cannot be consumed, modified, or transferred until 27 Oct @ 9:00am",
         "color":"e4ae39"}],
       "market_hash_name":"AUG | Storm (Field-Tested)","commodity":0,"marketable":0}],
     "asset_properties":[{"appid":730,"contextid":"16","assetid":"54238728503","asset_properties":[
      {"propertyid":1,"int_value":"302","name":"Pattern Template"},
      {"propertyid":2,"float_value":"0.372153759002685547","name":"Wear Rating"},
      {"propertyid":6,"string_value":"6B7BDCC9EEECA16A73634B0F436A5B6F539BFE919E682BC56903E8EBEBEB671B7332B429B2",
       "name":"Item Certificate"}]}],
     "total_inventory_count":1,"success":1,"rwgrsn":-2}"#;

    /// A's empty context 16 (line 24).
    const EMPTY: &str = r#"{"total_inventory_count":0,"success":1,"rwgrsn":-2}"#;

    #[test]
    fn items_with_descriptions_and_properties() {
        let page = InventoryPage::parse(CONTEXT_2).unwrap();
        let items: Vec<_> = page.items().collect();
        assert_eq!(items.len(), 2);

        let case = &items[0];
        assert_eq!(case.assetid, 25838682267);
        assert_eq!(case.market_hash_name, "Dreams & Nightmares Case");
        assert!(case.commodity && case.tradable && case.marketable);
        assert!(case.properties().is_empty());
        assert_eq!((case.pattern(), case.wear()), (None, None));

        let skin = &items[1];
        assert_eq!((skin.appid, skin.contextid, skin.assetid), (730, 2, 24936074294));
        assert_eq!((skin.classid, skin.instanceid, skin.amount), (7993036706, 480085569, 1));
        assert!(!skin.commodity);
        assert_eq!(skin.pattern(), Some(355));
        assert_eq!(skin.wear(), "0.0661438107490539551".parse().ok());
        let properties = skin.properties();
        assert_eq!(properties.len(), 3);
        assert_eq!(properties[2].name, "Item Certificate");
        assert!(matches!(properties[2].value, PropertyValue::String(_)));
    }

    #[test]
    fn trade_protected_item() {
        let page = InventoryPage::parse(CONTEXT_16).unwrap();
        let item = page.items().next().unwrap();
        assert_eq!(
            (item.contextid, item.assetid, item.classid),
            (16, 54238728503, 241575744)
        );
        assert!(!item.tradable && !item.marketable);
        assert_eq!(item.pattern(), Some(302));
        assert_eq!(item.wear(), "0.372153759002685547".parse().ok());
        // the 32 bits the game coordinator has for it
        assert_eq!(item.wear().map(|wear| (wear as f32).to_bits()), Some(0x3ebe_8af0));
        // the blank one is left out
        let texts = item.owner_descriptions();
        assert_eq!(texts.len(), 1);
        assert!(texts[0].ends_with("until 27 Oct @ 9:00am"));
    }

    #[test]
    fn escaped_text() {
        let body = r#"{"success":1,"assets":[{"appid":730,"contextid":"2","assetid":"1","classid":"1","amount":"1"}],
          "descriptions":[{"classid":"1","market_hash_name":"StatTrak™ \"x\""}]}"#;
        let page = InventoryPage::parse(body).unwrap();
        assert_eq!(page.items().next().unwrap().market_hash_name, "StatTrak™ \"x\"");
    }

    #[test]
    fn empty_inventory() {
        let page = InventoryPage::parse(EMPTY).unwrap();
        assert!(page.is_empty());
        assert_eq!(page.next_start(), None);
    }

    #[test]
    fn not_a_success() {
        for body in &[r#"{"success":false}"#, r#"{"success":0}"#, r#"{}"#, "null", "<html>"] {
            match InventoryPage::parse(body) {
                Err(TradeError::GeneralError(_)) => {}
                other => panic!("{}: {:?}", body, other),
            }
        }
        assert!(InventoryPage::parse(r#"{"success":true}"#).is_ok());
    }

    #[test]
    fn more_items_without_last_assetid_is_an_error() {
        let result = InventoryPage::parse(r#"{"assets":[],"success":1,"more_items":1}"#);
        assert!(matches!(result, Err(TradeError::GeneralError(_))));
    }

    #[test]
    fn urls() {
        assert_eq!(
            inventory_url(76561197961265730, InventoryContext::CS2_TRADE_PROTECTED, None),
            "https://steamcommunity.com/inventory/76561197961265730/730/16?l=english&count=1000"
        );
        assert_eq!(
            inventory_url(76561197961265730, InventoryContext::CS2, Some("123")),
            "https://steamcommunity.com/inventory/76561197961265730/730/2?l=english&count=1000&start_assetid=123"
        );
    }

    /// A page body holding `assetids`, saying more come after `last` if given.
    fn page(assetids: &[i64], last: Option<&str>) -> String {
        let assets: Vec<String> = assetids
            .iter()
            .map(|id| {
                format!(
                    r#"{{"appid":730,"contextid":"2","assetid":"{}","classid":"1","amount":"1"}}"#,
                    id
                )
            })
            .collect();
        let more = match last {
            Some(last) => format!(r#","more_items":1,"last_assetid":"{}""#, last),
            None => String::new(),
        };
        format!(r#"{{"assets":[{}],"success":1{}}}"#, assets.join(","), more)
    }

    fn ids(inventory: &Inventory) -> Vec<i64> {
        inventory
            .pages()
            .flat_map(|page| page.items().map(|item| item.assetid).collect::<Vec<_>>())
            .collect()
    }

    #[test]
    fn pages_are_followed() {
        let mut pages = vec![
            (None, page(&[1, 2], Some("2"))),
            (Some("2".to_string()), page(&[3], Some("3"))),
            (Some("3".to_string()), page(&[4], None)),
        ]
        .into_iter();
        let inventory = block_on(read_all_inventory_pages(|start| {
            let (expected, body) = pages.next().expect("no more pages");
            assert_eq!(start, expected);
            async move { Ok(body) }
        }))
        .unwrap();
        assert_eq!(ids(&inventory), vec![1, 2, 3, 4]);
        assert!(pages.next().is_none());
    }

    #[test]
    fn total_inventory_count() {
        assert_eq!(
            InventoryPage::parse(CONTEXT_2).unwrap().total_inventory_count(),
            Some(2)
        );
        assert_eq!(InventoryPage::parse(EMPTY).unwrap().total_inventory_count(), Some(0));
        let without = page(&[1], None);
        assert_eq!(InventoryPage::parse(&without).unwrap().total_inventory_count(), None);
        for (count, expected) in &[
            (r#""7""#, Some(7)),
            (r#""x""#, None),
            ("-1", None),
            ("null", None),
            ("{}", None),
        ] {
            let body = format!(r#"{{"success":1,"total_inventory_count":{}}}"#, count);
            assert_eq!(
                InventoryPage::parse(&body).unwrap().total_inventory_count(),
                *expected,
                "{}",
                count
            );
        }

        let inventory = Inventory::from_json(CONTEXT_16.to_string()).unwrap();
        assert_eq!(inventory.total_inventory_count(), Some(1));
        assert_eq!(Inventory::from_json(without).unwrap().total_inventory_count(), None);
        assert_eq!(Inventory::default().total_inventory_count(), None);
    }

    #[test]
    fn total_inventory_count_of_a_paged_inventory_is_the_first_page_s() {
        let mut pages = vec![
            r#"{"assets":[],"success":1,"more_items":1,"last_assetid":"2","total_inventory_count":3}"#,
            r#"{"assets":[],"success":1,"total_inventory_count":4}"#,
        ]
        .into_iter();
        let inventory = block_on(read_all_inventory_pages(|_| {
            let body = pages.next().expect("no more pages").to_string();
            async move { Ok(body) }
        }))
        .unwrap();
        assert_eq!(inventory.total_inventory_count(), Some(3));
    }

    #[test]
    fn repeated_page_is_an_error() {
        let mut served = 0;
        let result = block_on(read_all_inventory_pages(|_| {
            served += 1;
            let body = page(&[served], Some("1"));
            async move { Ok(body) }
        }));
        assert!(matches!(result, Err(TradeError::GeneralError(_))));
        assert_eq!(served, 2);
    }

    #[test]
    fn page_cap_is_an_error() {
        let mut served = 0;
        let result = block_on(read_all_inventory_pages(|_| {
            served += 1;
            let body = page(&[1], Some(&served.to_string()));
            async move { Ok(body) }
        }));
        assert!(matches!(result, Err(TradeError::GeneralError(_))));
        assert_eq!(served, MAX_INVENTORY_PAGES);
    }

    #[cfg(feature = "cs2")]
    #[test]
    fn fingerprints_of_the_skins() {
        let mut fingerprints = Inventory::from_json(CONTEXT_2.to_string()).unwrap().fingerprints();
        // the case has none
        assert_eq!(fingerprints.len(), 1);
        let skin = fingerprints.remove(&24936074294).unwrap();
        assert_eq!(skin, Fingerprint::new(355, "0.0661438107490539551".parse().unwrap()));
        assert!(Inventory::default().fingerprints().is_empty());
    }

    #[test]
    fn one_page_inventory() {
        let inventory = Inventory::from_json(CONTEXT_2.to_string()).unwrap();
        assert_eq!(ids(&inventory), vec![25838682267, 24936074294]);
        assert!(Inventory::from_json("null".to_string()).is_err());
    }
}
