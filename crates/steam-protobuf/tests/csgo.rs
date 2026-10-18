use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::cecon_item_preview_data_block::Sticker;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CEconItemPreviewDataBlock;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse;
use steam_protobuf::ProtobufDeserialize;
use steam_protobuf::ProtobufSerialize;

#[test]
fn econ_preview_data_block_response_roundtrip() {
    let mut sticker = Sticker::new();
    sticker.set_slot(2);
    sticker.set_sticker_id(5032);
    sticker.set_wear(0.25);

    // Keychains reuse the sticker message, with the charm's seed in `pattern`.
    let mut keychain = Sticker::new();
    keychain.set_slot(0);
    keychain.set_sticker_id(36);
    keychain.set_pattern(81234);
    keychain.set_offset_x(10.5);

    let mut iteminfo = CEconItemPreviewDataBlock::new();
    iteminfo.set_itemid(39151488012);
    iteminfo.set_defindex(7);
    iteminfo.set_paintindex(282);
    iteminfo.set_paintseed(661);
    // The float is sent as its bits.
    iteminfo.set_paintwear(0.071_234_56_f32.to_bits());
    iteminfo.stickers.push(sticker);
    iteminfo.keychains.push(keychain);

    let mut response = CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse::new();
    response.iteminfo = Some(iteminfo).into();

    let bytes = response.to_bytes().unwrap();
    let decoded = CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, response);

    let iteminfo = decoded.iteminfo.as_ref().unwrap();
    assert_eq!(iteminfo.itemid(), 39151488012);
    assert_eq!(iteminfo.paintseed(), 661);
    assert_eq!(f32::from_bits(iteminfo.paintwear()), 0.071_234_56);

    assert_eq!(iteminfo.stickers.len(), 1);
    assert_eq!(iteminfo.stickers[0].slot(), 2);
    assert_eq!(iteminfo.stickers[0].sticker_id(), 5032);
    assert_eq!(iteminfo.stickers[0].wear(), 0.25);

    assert_eq!(iteminfo.keychains.len(), 1);
    assert_eq!(iteminfo.keychains[0].sticker_id(), 36);
    assert_eq!(iteminfo.keychains[0].pattern(), 81234);
    assert_eq!(iteminfo.keychains[0].offset_x(), 10.5);
}
