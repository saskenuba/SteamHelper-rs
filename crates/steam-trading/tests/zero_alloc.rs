//! Reading an inventory page allocates nothing per item: its lists grow a few times per page, and reading its items,
//! with their pattern and wear, allocates nothing at all.
//!
//! A global allocator counts the allocations of the thread being measured, so this is a test binary of its own. Run
//! with `--nocapture` to see the counts.

#![allow(unsafe_code)]

use std::alloc::GlobalAlloc;
use std::alloc::Layout;
use std::alloc::System;
use std::cell::Cell;

use steam_trading::InventoryPage;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

thread_local! {
    /// Allocations made by this thread, while measuring.
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

struct CountingAllocator;

impl CountingAllocator {
    fn count(&self) {
        // fails while the thread is torn down, when nothing is measured anyway
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get().map(|count| count + 1)));
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.count();
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        self.count();
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        self.count();
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

/// Runs `f`, and returns its result with how many allocations it made on this thread.
fn allocations<R>(f: impl FnOnce() -> R) -> (R, usize) {
    ALLOCATIONS.with(|count| count.set(Some(0)));
    let result = f();
    let count = ALLOCATIONS.with(|count| count.take()).unwrap();
    (result, count)
}

/// A CS2 page of `items` skins as Steam sends them (`fixtures/live/2026-10-20/ack-cycle.jsonl`): each with its own
/// description, nested lists included, and its properties.
fn page(items: usize) -> String {
    let mut assets = Vec::new();
    let mut descriptions = Vec::new();
    let mut properties = Vec::new();
    for i in 0..items {
        let assetid = 24234602605 + i as u64;
        let classid = 241575744 + i as u64;
        assets.push(format!(
            r#"{{"appid":730,"contextid":"16","assetid":"{assetid}","classid":"{classid}","instanceid":"9119903511","amount":"1"}}"#
        ));
        descriptions.push(format!(
            r#"{{"appid":730,"classid":"{classid}","instanceid":"9119903511","currency":0,"background_color":"",
              "icon_url":"i0CoZ81Ui0m-9KwlBY1L_18myuGuq1wfhWSaZgMttyVfPaERSR0Wqmu7LAocGIGz3UqlXOLrxM-vMGmW8VNxu5Dx60noTyL6kJ_m-B1Q7uCvZaZkNM-SA1iSzvp3j-1gSCGn20tztm_UyIn_JHKUbgYlWMcmQ-ZcskSwldS0MOnntAfd3YlMzH35jntXrnE8SOGRGG4",
              "descriptions":[{{"type":"html","value":"Exterior: Field-Tested","name":"exterior_wear"}},
                {{"type":"html","value":" ","name":"blank"}},{{"type":"html","value":"\nIt has been painted\n","name":"description"}}],
              "tradable":0,"actions":[{{"link":"steam://run/730//+csgo_econ_action_preview%20%propid:6%","name":"Inspect in Game..."}}],
              "owner_descriptions":[{{"type":"html","value":" "}},{{"type":"html","value":"⇆ This item is trade-protected and cannot be consumed, modified, or transferred until 27 Oct @ 9:00am","color":"e4ae39"}}],
              "name":"AUG | Storm","name_color":"D2D2D2","type":"Consumer Grade Rifle","market_name":"AUG | Storm (Field-Tested)",
              "market_hash_name":"AUG | Storm (Field-Tested)","commodity":0,"market_tradable_restriction":7,
              "market_marketable_restriction":7,"marketable":0,"sealed":1,"sealed_type":0,
              "tags":[{{"category":"Type","internal_name":"CSGO_Type_Rifle","localized_category_name":"Type","localized_tag_name":"Rifle"}},
                {{"category":"Weapon","internal_name":"weapon_aug","localized_category_name":"Weapon","localized_tag_name":"AUG"}}]}}"#
        ));
        properties.push(format!(
            r#"{{"appid":730,"contextid":"16","assetid":"{assetid}","asset_properties":[
              {{"propertyid":1,"int_value":"{}","name":"Pattern Template"}},
              {{"propertyid":2,"float_value":"0.372153759002685547","name":"Wear Rating"}},
              {{"propertyid":6,"string_value":"6B7BDCC9EEECA16A73634B0F436A5B6F539BFE919E682BC56903E8EBEBEB671B7332B429B2","name":"Item Certificate"}}]}}"#,
            i % 1000
        ));
    }
    format!(
        r#"{{"assets":[{}],"descriptions":[{}],"asset_properties":[{}],"total_inventory_count":{items},"success":1,"rwgrsn":-2}}"#,
        assets.join(","),
        descriptions.join(","),
        properties.join(",")
    )
}

/// Allocations to read a page of `items` items.
fn parse_allocations(body: &str) -> usize {
    let (page, count) = allocations(|| InventoryPage::parse(body).unwrap());
    drop(page);
    count
}

#[test]
fn reading_a_page_allocates_nothing_per_item() {
    let counts: Vec<(usize, usize)> = [1, 10, 100, 1000]
        .iter()
        .map(|&items| (items, parse_allocations(&page(items))))
        .collect();
    println!("allocations to read a page, by its items: {:?}", counts);

    // the three lists double as they fill: a few allocations per tenfold more items, not ten times more
    for pair in counts.windows(2) {
        let ((_, fewer), (_, more)) = (pair[0], pair[1]);
        assert!(more - fewer <= 3 * 4, "{:?}", counts);
    }
    assert!(counts[3].1 <= 40, "{:?}", counts);
}

#[test]
fn reading_items_allocates_nothing() {
    let body = page(100);
    let page = InventoryPage::parse(&body).unwrap();

    let (checksum, count) = allocations(|| {
        let mut checksum = 0;
        for item in page.items() {
            checksum += item.assetid + item.classid + item.market_hash_name.len() as i64;
            checksum += item.pattern().unwrap() + item.wear().map_or(0, |wear| (wear * 1000.0) as i64);
            checksum += i64::from(item.tradable) + i64::from(item.commodity);
        }
        checksum
    });
    assert!(checksum > 0);
    assert_eq!(count, 0);
}

#[test]
fn an_empty_page_allocates_nothing() {
    let count = parse_allocations(r#"{"total_inventory_count":0,"success":1,"rwgrsn":-2}"#);
    assert_eq!(count, 0);
}
