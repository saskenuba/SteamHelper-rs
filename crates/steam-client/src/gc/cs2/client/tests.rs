use bytes::Bytes;
use steam_language_gen::generated::enums::EMsg;
use steam_protobuf::protobuf::EnumOrUnknown;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::csgo::base_gcmessages::CSOEconItem;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::cecon_item_preview_data_block::Sticker as ProtoSticker;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CDataGCCStrike15_v2_MatchInfo;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CEconItemPreviewDataBlock;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::cmsg_somultiple_objects;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgClientWelcome;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgSOIDOwner;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgSOMultipleObjects;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgSOSingleObject;
use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::protobufs::steammessages_clientserver::CMsgClientGamesPlayed;
use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgGCClient;
use tokio::sync::mpsc;
use tokio::time::timeout;

use super::*;
use crate::connection::tests::soon;
use crate::connection::tests::FakeCm;
use crate::events::Event;
use crate::gc;
use crate::gc::cs2::inventory;
use crate::gc::cs2::inventory::SO_CACHE_SUBSCRIBED;
use crate::gc::cs2::inventory::SO_CREATE;
use crate::gc::cs2::inventory::SO_DESTROY;
use crate::gc::cs2::inventory::SO_UPDATE;
use crate::gc::cs2::inventory::SO_UPDATE_MULTIPLE;
use crate::gc::cs2::matches::tests::finished;
use crate::gc::cs2::InspectLinkError;
use crate::handle::tests::next;
use crate::handle::tests::reconnected;
use crate::handle::tests::recv;
use crate::handle::tests::relogs;
use crate::handle::tests::round_trip;
use crate::handle::tests::spawned_as;
use crate::handle::tests::user;
use crate::handle::tests::Reconnection;
use crate::handle::tests::Spawned;
use crate::handle::Command;
use crate::handle::Events;

const OWNER: u64 = 76561198084749846;
const ASSET: u64 = 698323590;
const D: u64 = 7935523998312483177;
/// Paintwear 0.20482855, as f32 bits.
const WEAR: u32 = 1045544595;
/// A generated link that embeds an M4A1-S (def index 60), as `item.rs` has it.
const EMBEDDED: &str = "00183C20B803280538E9A3C5DD0340E102C246A0D1";

fn inventory(asset_id: u64) -> InspectLink {
    format!("S{}A{}D{}", OWNER, asset_id, D).parse().unwrap()
}

/// An item as the game coordinator describes it: an AK-47 Case Hardened with a sticker.
fn item(asset_id: u64) -> CEconItemPreviewDataBlock {
    let mut sticker = ProtoSticker::new();
    sticker.set_slot(1);
    sticker.set_sticker_id(5032);
    sticker.set_wear(0.5);
    let mut item = CEconItemPreviewDataBlock::new();
    item.set_itemid(asset_id);
    item.set_defindex(7);
    item.set_paintindex(44);
    item.set_paintseed(661);
    item.set_paintwear(WEAR);
    item.stickers.push(sticker);
    item
}

fn answer(item: Option<CEconItemPreviewDataBlock>) -> CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse {
    let mut response = CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse::new();
    response.iteminfo = item.into();
    response
}

fn status(status: i32) -> CMsgConnectionStatus {
    let mut message = CMsgConnectionStatus::new();
    message.status = Some(EnumOrUnknown::from_i32(status));
    message
}

/// The next message to CS2's game coordinator: its type and body. Skips the games played, which the client sets
/// again each time it logs on.
async fn to_gc(cm: &mut FakeCm) -> (u32, Bytes) {
    loop {
        let call = next(cm).await;
        if call.emsg == EMsg::ClientGamesPlayedWithDataBlob {
            continue;
        }
        assert_eq!(call.emsg, EMsg::ClientToGC);
        assert_eq!(call.header.routing_appid(), APP_ID);
        let envelope = call.body::<CMsgGCClient>();
        assert_eq!(envelope.appid(), APP_ID);
        let msg_type = envelope.msgtype();
        let body = gc::gc_body(msg_type, envelope.payload.unwrap()).unwrap();
        return (msg_type & !0x8000_0000, body);
    }
}

/// Reads a hello, as node-globaloffensive sends it. Returns when it came.
async fn hello_read(cm: &mut FakeCm) -> Instant {
    let (msg_type, body) = to_gc(cm).await;
    assert_eq!(msg_type, CLIENT_HELLO);
    let hello = CMsgClientHello::parse_from_tokio_bytes(&body).unwrap();
    assert_eq!(hello.version(), 2000244);
    assert!(hello.has_client_launcher());
    assert_eq!(hello, super::hello());
    Instant::now()
}

/// Reads an inspect request. Returns it and when it came.
async fn request_read(cm: &mut FakeCm) -> (CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockRequest, Instant) {
    let (msg_type, body) = to_gc(cm).await;
    assert_eq!(msg_type, PREVIEW_REQUEST);
    let request = CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockRequest::parse_from_tokio_bytes(&body).unwrap();
    (request, Instant::now())
}

/// Sends `body`, of type `msg_type`, from CS2's game coordinator.
async fn from_gc<M: Message>(cm: &mut FakeCm, msg_type: u32, body: &M) {
    let payload = gc::tests::proto_payload(msg_type, &CMsgProtoBufHeader::new(), &body.write_to_bytes().unwrap());
    cm.send(gc::tests::from_gc(APP_ID, msg_type | 0x8000_0000, payload))
        .await;
}

async fn welcome(cm: &mut FakeCm) {
    from_gc(cm, CLIENT_WELCOME, &CMsgClientWelcome::new()).await;
}

/// Checks the client sends nothing more to the game coordinator for `wait`.
async fn quiet(cm: &mut FakeCm, wait: Duration) {
    if let Ok((msg_type, _)) = timeout(wait, to_gc(cm)).await {
        panic!("sent the game coordinator a {}", msg_type);
    }
}

/// A spawned client welcomed by CS2's game coordinator, behind a fake CM.
struct Session {
    cs2: Cs2,
    cm: FakeCm,
    /// Keeps the client running.
    handle: SteamHandle<User>,
    events: Events,
    reconnects: mpsc::UnboundedReceiver<Reconnection>,
}

async fn session() -> Session {
    session_welcomed(|_| CMsgClientWelcome::new()).await
}

/// A session the game coordinator welcomed with `welcome(account)`.
async fn session_welcomed(welcome: impl FnOnce(SteamID) -> CMsgClientWelcome) -> Session {
    let Spawned {
        handle,
        events,
        mut cm,
        reconnects,
        ..
    } = spawned_as(user()).await;
    let welcome = welcome(handle.steam_id());
    let gc = async {
        hello_read(&mut cm).await;
        from_gc(&mut cm, CLIENT_WELCOME, &welcome).await;
    };
    let (cs2, ()) = tokio::join!(soon(Cs2::connect(&handle)), gc);
    Session {
        cs2: cs2.unwrap(),
        cm,
        handle,
        events,
        reconnects,
    }
}

async fn inspect(cs2: &mut Cs2, link: InspectLink) -> Result<ItemDetails, ClientError> {
    cs2.inspect(&link).await
}

/// Inspects `link` while the game coordinator answers the request with `answer`. Returns the result and the request.
async fn inspected(
    session: &mut Session,
    link: &InspectLink,
    response: CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse,
) -> (
    Result<ItemDetails, ClientError>,
    CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockRequest,
) {
    let cm = &mut session.cm;
    let gc = async {
        let (request, _) = request_read(cm).await;
        from_gc(cm, PREVIEW_RESPONSE, &response).await;
        request
    };
    tokio::join!(soon(session.cs2.inspect(link)), gc)
}

#[tokio::test(start_paused = true)]
async fn connects_once_welcomed_greeting_less_and_less_often() {
    let Spawned { handle, mut cm, .. } = spawned_as(user()).await;
    let start = Instant::now();
    let gc = async {
        let games = next(&mut cm).await;
        assert_eq!(games.emsg, EMsg::ClientGamesPlayedWithDataBlob);
        let games = games.body::<CMsgClientGamesPlayed>();
        let games: Vec<_> = games.games_played.iter().map(|game| game.game_id()).collect();
        assert_eq!(games, [u64::from(APP_ID)]);

        let mut hellos = vec![];
        for _ in 0..3 {
            hellos.push(hello_read(&mut cm).await - start);
        }
        welcome(&mut cm).await;
        hellos
    };
    let (cs2, hellos) = tokio::join!(soon(Cs2::connect(&handle)), gc);
    cs2.unwrap();
    // half a second, then 2 s, then 4 s
    assert_eq!(
        hellos,
        [
            Duration::from_millis(500),
            Duration::from_millis(2500),
            Duration::from_millis(6500)
        ]
    );
    // welcomed: no more hellos
    quiet(&mut cm, Duration::from_secs(120)).await;
}

#[tokio::test(start_paused = true)]
async fn connecting_times_out_without_a_welcome() {
    let Spawned { handle, mut cm, .. } = spawned_as(user()).await;
    let start = Instant::now();
    assert!(matches!(
        soon(Cs2::connect(&handle)).await,
        Err(ClientError::GcTimeout(CLIENT_WELCOME))
    ));
    assert_eq!(start.elapsed(), GREET_TIMEOUT);
    // at 0.5, 2.5, 6.5, 14.5 and 30.5 s
    for _ in 0..5 {
        hello_read(&mut cm).await;
    }
    quiet(&mut cm, Duration::from_secs(120)).await;
}

#[tokio::test(start_paused = true)]
async fn connecting_fails_when_the_game_coordinator_refuses_the_session() {
    let Spawned { handle, mut cm, .. } = spawned_as(user()).await;
    let gc = async {
        hello_read(&mut cm).await;
        let mut error = CMsgGCCStrike15_v2_ClientLogonFatalError::new();
        error.set_errorcode(4);
        error.set_message("No CS2 license".to_string());
        from_gc(&mut cm, LOGON_FATAL_ERROR, &error).await;
    };
    let (result, ()) = tokio::join!(soon(Cs2::connect(&handle)), gc);
    match result {
        Err(ClientError::GcRefused { code, message }) => assert_eq!((code, message.as_str()), (4, "No CS2 license")),
        result => panic!("expected GcRefused, got {:?}", result),
    }
}

#[tokio::test(start_paused = true)]
async fn inspects_an_item_in_an_inventory() {
    let mut session = session().await;
    let (details, request) = inspected(&mut session, &inventory(ASSET), answer(Some(item(ASSET)))).await;
    assert_eq!(
        (
            request.param_s(),
            request.param_a(),
            request.param_d(),
            request.param_m()
        ),
        (OWNER, ASSET, D, 0)
    );
    let details = details.unwrap();
    assert_eq!(details.item_id(), Some(ASSET));
    assert_eq!(details.def_index(), 7);
    assert_eq!(details.paint_index(), Some(44));
    assert_eq!(details.pattern(), Some(661));
    assert!((details.float().unwrap() - 0.204_828_55).abs() < 1e-7);
    let stickers: Vec<_> = details
        .stickers()
        .map(|sticker| (sticker.slot, sticker.id, sticker.wear))
        .collect();
    assert_eq!(stickers, [(1, 5032, Some(0.5))]);
}

#[tokio::test(start_paused = true)]
async fn skips_answers_about_other_items() {
    let mut session = session().await;
    let listing = 625254122282020305;
    let asset = 6760346663;
    let link: InspectLink = format!("M{}A{}D{}", listing, asset, D).parse().unwrap();
    let cm = &mut session.cm;
    let gc = async {
        let (request, _) = request_read(cm).await;
        assert_eq!(
            (
                request.param_s(),
                request.param_a(),
                request.param_d(),
                request.param_m()
            ),
            (0, asset, D, listing)
        );
        // about another item, then about none it seems, then the one asked
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(ASSET)))).await;
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(CEconItemPreviewDataBlock::new()))).await;
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(asset)))).await;
    };
    let (details, ()) = tokio::join!(soon(session.cs2.inspect(&link)), gc);
    assert_eq!(details.unwrap().item_id(), Some(asset));
}

#[tokio::test(start_paused = true)]
async fn an_inspect_times_out_and_its_late_answer_is_skipped() {
    let mut session = session().await;
    let (result, (_, sent)) = tokio::join!(
        soon(inspect(&mut session.cs2, inventory(ASSET))),
        request_read(&mut session.cm)
    );
    assert!(matches!(result, Err(ClientError::GcTimeout(PREVIEW_RESPONSE))));
    assert_eq!(sent.elapsed(), INSPECT_TIMEOUT);

    let cm = &mut session.cm;
    let gc = async {
        let (request, _) = request_read(cm).await;
        assert_eq!(request.param_a(), ASSET + 1);
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(ASSET)))).await;
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(ASSET + 1)))).await;
    };
    let (details, ()) = tokio::join!(soon(inspect(&mut session.cs2, inventory(ASSET + 1))), gc);
    assert_eq!(details.unwrap().item_id(), Some(ASSET + 1));
}

#[tokio::test(start_paused = true)]
async fn the_timeout_can_be_changed() {
    let Session {
        cs2, mut cm, handle, ..
    } = session().await;
    let mut cs2 = cs2.with_timeout(Duration::from_secs(2));
    let (result, (_, sent)) = tokio::join!(soon(inspect(&mut cs2, inventory(ASSET))), request_read(&mut cm));
    assert!(matches!(result, Err(ClientError::GcTimeout(PREVIEW_RESPONSE))));
    assert_eq!(sent.elapsed(), Duration::from_secs(2));
    drop(handle);
}

#[tokio::test(start_paused = true)]
async fn waits_the_cooldown_from_one_request_to_the_next() {
    let mut session = session().await;
    let cm = &mut session.cm;
    let cs2 = &mut session.cs2;
    let gc = async {
        let mut sent = vec![];
        for asset in 0..2 {
            let (_, at) = request_read(cm).await;
            sent.push(at);
            from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(asset)))).await;
        }
        // answered after the cooldown: the next request goes at once
        let (_, at) = request_read(cm).await;
        sent.push(at);
        tokio::time::sleep(Duration::from_secs(2)).await;
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(2)))).await;
        let (_, at) = request_read(cm).await;
        sent.push(at);
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(3)))).await;
        sent
    };
    let inspects = async {
        for asset in 0..4 {
            soon(inspect(cs2, inventory(asset))).await.unwrap();
        }
    };
    let (sent, ()) = tokio::join!(gc, inspects);
    let gaps: Vec<_> = sent.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert_eq!(gaps, [INSPECT_COOLDOWN, INSPECT_COOLDOWN, Duration::from_secs(2)]);
}

#[tokio::test(start_paused = true)]
async fn the_cooldown_can_be_changed() {
    let Session {
        cs2, mut cm, handle, ..
    } = session().await;
    let mut cs2 = cs2.with_cooldown(Duration::from_secs(3));
    let gc = async {
        let mut sent = vec![];
        for asset in 0..2 {
            let (_, at) = request_read(&mut cm).await;
            sent.push(at);
            from_gc(&mut cm, PREVIEW_RESPONSE, &answer(Some(item(asset)))).await;
        }
        sent[1] - sent[0]
    };
    let inspects = async {
        for asset in 0..2 {
            soon(inspect(&mut cs2, inventory(asset))).await.unwrap();
        }
    };
    let (gap, ()) = tokio::join!(gc, inspects);
    assert_eq!(gap, Duration::from_secs(3));
    drop(handle);
}

#[tokio::test(start_paused = true)]
async fn greets_again_before_inspecting_once_the_connection_to_steam_was_lost() {
    let mut session = session().await;
    session.cm = reconnected_cm(&mut session).await;
    // the client logged on again: the game coordinator doesn't know the account anymore
    let cm = &mut session.cm;
    let gc = async {
        hello_read(cm).await;
        welcome(cm).await;
        let (request, _) = request_read(cm).await;
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(request.param_a())))).await;
    };
    let (details, ()) = tokio::join!(soon(inspect(&mut session.cs2, inventory(ASSET))), gc);
    assert_eq!(details.unwrap().item_id(), Some(ASSET));
}

/// Closes the session's connection to Steam, and gives the fake CM the client logged on to again.
async fn reconnected_cm(session: &mut Session) -> FakeCm {
    let old = mem::replace(&mut session.cm, placeholder());
    drop(old);
    assert!(matches!(
        recv(&mut session.events).await,
        Some(Event::Disconnected { .. })
    ));
    let mut cm = reconnected(&mut session.reconnects).await;
    relogs(&mut cm).await;
    assert!(matches!(recv(&mut session.events).await, Some(Event::Reconnected)));
    cm
}

/// A fake CM no client is connected to.
fn placeholder() -> FakeCm {
    let (_, server) = tokio::io::duplex(1);
    FakeCm::new(server, crate::connection::tests::key())
}

#[tokio::test(start_paused = true)]
async fn greets_again_when_the_game_coordinator_has_no_session() {
    let mut session = session().await;
    from_gc(
        &mut session.cm,
        CONNECTION_STATUS,
        &status(GCConnectionStatus::GCConnectionStatus_NO_SESSION as i32),
    )
    .await;
    round_trip(&session.handle, &mut session.cm).await;
    let cm = &mut session.cm;
    let gc = async {
        hello_read(cm).await;
        welcome(cm).await;
        let (request, _) = request_read(cm).await;
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(request.param_a())))).await;
    };
    let (details, ()) = tokio::join!(soon(inspect(&mut session.cs2, inventory(ASSET))), gc);
    details.unwrap();

    // a status the crate doesn't know isn't a session either
    from_gc(&mut session.cm, CONNECTION_STATUS, &status(9)).await;
    round_trip(&session.handle, &mut session.cm).await;
    let cm = &mut session.cm;
    let gc = async {
        hello_read(cm).await;
        welcome(cm).await;
        let (request, _) = request_read(cm).await;
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(request.param_a())))).await;
    };
    let (details, ()) = tokio::join!(soon(inspect(&mut session.cs2, inventory(ASSET))), gc);
    details.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_session_kept_or_welcomed_again_unasked_needs_no_greeting() {
    let mut session = session().await;
    from_gc(
        &mut session.cm,
        CONNECTION_STATUS,
        &status(GCConnectionStatus::GCConnectionStatus_HAVE_SESSION as i32),
    )
    .await;
    // the default status is a session
    from_gc(&mut session.cm, CONNECTION_STATUS, &CMsgConnectionStatus::new()).await;
    round_trip(&session.handle, &mut session.cm).await;
    // straight to the request
    let (details, _) = inspected(&mut session, &inventory(ASSET), answer(Some(item(ASSET)))).await;
    details.unwrap();

    from_gc(
        &mut session.cm,
        CONNECTION_STATUS,
        &status(GCConnectionStatus::GCConnectionStatus_GC_GOING_DOWN as i32),
    )
    .await;
    welcome(&mut session.cm).await;
    round_trip(&session.handle, &mut session.cm).await;
    let (details, _) = inspected(&mut session, &inventory(ASSET), answer(Some(item(ASSET)))).await;
    details.unwrap();
}

#[tokio::test(start_paused = true)]
async fn embedded_links_are_read_without_the_game_coordinator() {
    let mut session = session().await;
    let start = Instant::now();
    let details = soon(inspect(&mut session.cs2, EMBEDDED.parse().unwrap()))
        .await
        .unwrap();
    assert_eq!(details.def_index(), 60);
    assert_eq!(details.item_id(), None);
    assert_eq!(start.elapsed(), Duration::ZERO);
    quiet(&mut session.cm, Duration::from_secs(30)).await;

    // a link that isn't one
    assert!(matches!(
        soon(session.cs2.inspect_str("S1A2")).await,
        Err(ClientError::InspectLink(InspectLinkError::Missing('D')))
    ));
    let details = soon(session.cs2.inspect_str(EMBEDDED)).await.unwrap();
    assert_eq!(details.def_index(), 60);
}

#[tokio::test(start_paused = true)]
async fn an_answer_without_an_item_is_an_error() {
    let mut session = session().await;
    let (result, _) = inspected(&mut session, &inventory(ASSET), answer(None)).await;
    assert!(matches!(
        result,
        Err(ClientError::ItemDetails(ItemDetailsError::NoItem))
    ));
    // an item without a def index isn't an item
    let mut nameless = item(ASSET);
    nameless.clear_defindex();
    let (result, _) = inspected(&mut session, &inventory(ASSET), answer(Some(nameless))).await;
    assert!(matches!(
        result,
        Err(ClientError::ItemDetails(ItemDetailsError::NoDefIndex))
    ));
}

#[tokio::test(start_paused = true)]
async fn inspecting_fails_once_the_client_stopped() {
    let Session { mut cs2, handle, .. } = session().await;
    drop(handle);
    assert!(matches!(
        soon(inspect(&mut cs2, inventory(ASSET))).await,
        Err(ClientError::Closed)
    ));
}

#[tokio::test(start_paused = true)]
async fn keeps_waiting_for_the_answer_after_missing_messages() {
    let (commands, mut sent) = mpsc::unbounded_channel();
    let (channel, route) = gc::channel(APP_ID, commands.downgrade());
    let mut cs2 = Cs2::new(channel, SteamID::from_account_id(1));
    cs2.session = super::Session::Welcomed;
    let gc = async {
        let Some(Command::ToGc(request)) = sent.recv().await else {
            panic!("expected a message to the game coordinator");
        };
        assert_eq!(request.msgtype(), PREVIEW_REQUEST | 0x8000_0000);
        // some dropped, then the answer
        route.queue.lock().dropped = 3;
        let response = answer(Some(item(ASSET))).write_to_bytes().unwrap();
        let payload = gc::tests::proto_payload(PREVIEW_RESPONSE, &CMsgProtoBufHeader::new(), &response);
        let packet = gc::tests::from_gc(APP_ID, PREVIEW_RESPONSE | 0x8000_0000, payload);
        assert!(route.queue.push(gc::tests::raw(packet).gc_message().unwrap().unwrap()));
    };
    let (details, ()) = tokio::join!(soon(inspect(&mut cs2, inventory(ASSET))), gc);
    assert_eq!(details.unwrap().item_id(), Some(ASSET));
    assert!(cs2.has_session());
}

#[tokio::test(start_paused = true)]
async fn a_session_lost_while_waiting_is_greeted_again_by_the_next_inspect() {
    let (commands, mut sent) = mpsc::unbounded_channel();
    let (channel, route) = gc::channel(APP_ID, commands.downgrade());
    let mut cs2 = Cs2::new(channel, SteamID::from_account_id(1)).with_timeout(Duration::from_secs(1));
    cs2.session = super::Session::Welcomed;
    let gc = async {
        assert!(matches!(sent.recv().await, Some(Command::ToGc(_))));
        route.queue.session_lost();
    };
    let (result, ()) = tokio::join!(soon(inspect(&mut cs2, inventory(ASSET))), gc);
    assert!(matches!(result, Err(ClientError::GcTimeout(PREVIEW_RESPONSE))));
    assert!(!cs2.has_session());
    drop(route);
    // the channel ended: greeting fails at once
    assert!(matches!(
        soon(inspect(&mut cs2, inventory(ASSET))).await,
        Err(ClientError::Closed)
    ));
}

// Requests other than inspecting, events and the inventory.

/// A player, as the inspect links have it: account id 124484118.
const PLAYER: u64 = OWNER;
const PLAYER_ACCOUNT: u32 = 124484118;
/// akiver's first share code sample.
const SHARE_CODE: &str = "CSGO-L9spZ-ihuov-cyhtE-kxbqa-FkBAA";
const SHARED_MATCH: u64 = 3400360672356205056;

/// Reads the next message to the game coordinator, which must be of type `msg_type`, as `M`. Returns it and when it
/// came.
async fn read_request<M: Message>(cm: &mut FakeCm, msg_type: u32) -> (M, Instant) {
    let (got, body) = to_gc(cm).await;
    assert_eq!(got, msg_type);
    (M::parse_from_tokio_bytes(&body).unwrap(), Instant::now())
}

fn stats(account_id: u32, level: i32) -> CMsgGCCStrike15_v2_MatchmakingGC2ClientHello {
    let mut stats = CMsgGCCStrike15_v2_MatchmakingGC2ClientHello::new();
    stats.set_account_id(account_id);
    stats.set_player_level(level);
    stats
}

fn profiles(profiles: Vec<CMsgGCCStrike15_v2_MatchmakingGC2ClientHello>) -> CMsgGCCStrike15_v2_PlayersProfile {
    let mut message = CMsgGCCStrike15_v2_PlayersProfile::new();
    message.account_profiles = profiles;
    message
}

fn list(
    request: u32,
    account_id: Option<u32>,
    matches: Vec<CDataGCCStrike15_v2_MatchInfo>,
) -> CMsgGCCStrike15_v2_MatchList {
    let mut list = CMsgGCCStrike15_v2_MatchList::new();
    list.set_msgrequestid(request);
    list.accountid = account_id;
    list.matches = matches;
    list
}

/// A shared object holding `item`, of type `type_id`, owned by `owner` when it says.
fn object(type_id: i32, item: &CSOEconItem, owner: Option<SteamID>) -> CMsgSOSingleObject {
    let mut object = CMsgSOSingleObject::new();
    object.set_type_id(type_id);
    object.set_object_data(inventory::tests::object(item));
    if let Some(owner) = owner {
        let mut soid = CMsgSOIDOwner::new();
        soid.set_type(1);
        soid.set_id(owner.to_steam64());
        object.owner_soid = Some(soid).into();
    }
    object
}

/// A welcome with the account's cache, holding `items`, and someone else's.
fn welcome_with(account: SteamID, items: &[CSOEconItem]) -> CMsgClientWelcome {
    let mut welcome = CMsgClientWelcome::new();
    welcome.outofdate_subscribed_caches = vec![
        inventory::tests::cache(Some(SteamID::from_account_id(5)), &[inventory::tests::item(99)]),
        inventory::tests::cache(Some(account), items),
    ];
    welcome
}

fn ids(inventory: &Inventory) -> Vec<u64> {
    inventory.iter().map(InventoryItem::id).collect()
}

/// The events waiting, once the client read what the fake CM sent.
async fn waiting(session: &mut Session) -> Vec<Cs2Event> {
    round_trip(&session.handle, &mut session.cm).await;
    let mut events = vec![];
    while let Some(event) = session.cs2.try_next_event().unwrap() {
        events.push(event);
    }
    events
}

fn other_type(event: &Cs2Event) -> Option<u32> {
    match event {
        Cs2Event::Other(message) => Some(message.msg_type()),
        _ => None,
    }
}

/// A message of type `msg_type` from CS2's game coordinator, as the channel gives it.
fn gc_message<M: Message>(msg_type: u32, body: &M) -> GcMessage {
    let payload = gc::tests::proto_payload(msg_type, &CMsgProtoBufHeader::new(), &body.write_to_bytes().unwrap());
    gc::tests::raw(gc::tests::from_gc(APP_ID, msg_type | 0x8000_0000, payload))
        .gc_message()
        .unwrap()
        .unwrap()
}

#[tokio::test(start_paused = true)]
async fn keeps_the_inventory_and_the_stats_the_game_coordinator_sends() {
    let mut session =
        session_welcomed(|account| welcome_with(account, &[inventory::tests::item(1), inventory::tests::item(2)]))
            .await;
    // the account's items only
    assert_eq!(ids(session.cs2.inventory()), [1, 2]);
    let item = session.cs2.inventory().get(1).unwrap();
    assert_eq!((item.paint_index(), item.pattern()), (Some(44), Some(661)));
    assert!(session.cs2.matchmaking_stats().is_none());

    from_gc(&mut session.cm, MATCHMAKING_HELLO, &stats(PLAYER_ACCOUNT, 12)).await;
    let events = waiting(&mut session).await;
    // the first welcome is no event
    match events.as_slice() {
        [Cs2Event::MatchmakingStats(stats)] => assert_eq!(stats.level(), Some(12)),
        events => panic!("unexpected events: {:?}", events),
    }
    assert_eq!(session.cs2.matchmaking_stats().unwrap().level(), Some(12));
    assert!(session.cs2.has_session());
}

#[tokio::test(start_paused = true)]
async fn asks_for_the_matchmaking_stats_again() {
    let mut session = session().await;
    let cm = &mut session.cm;
    let gc = async {
        let (request, _) =
            read_request::<CMsgGCCStrike15_v2_MatchmakingClient2GCHello>(cm, MATCHMAKING_CLIENT_HELLO).await;
        assert_eq!(request.compute_size(), 0);
        from_gc(cm, MATCHMAKING_HELLO, &stats(PLAYER_ACCOUNT, 30)).await;
    };
    let (stats, ()) = tokio::join!(soon(session.cs2.request_matchmaking_stats()), gc);
    assert_eq!(stats.unwrap().level(), Some(30));
    assert_eq!(session.cs2.matchmaking_stats().unwrap().level(), Some(30));
    // the answer is no event
    assert!(waiting(&mut session).await.is_empty());
}

#[tokio::test(start_paused = true)]
async fn asks_for_a_players_profile() {
    let mut session = session().await;
    let cm = &mut session.cm;
    let gc = async {
        let (request, _) =
            read_request::<CMsgGCCStrike15_v2_ClientRequestPlayersProfile>(cm, REQUEST_PLAYERS_PROFILE).await;
        assert_eq!((request.account_id(), request.request_level()), (PLAYER_ACCOUNT, 32));
        // none, someone else's, then the player's
        from_gc(cm, PLAYERS_PROFILE, &profiles(vec![])).await;
        from_gc(cm, PLAYERS_PROFILE, &profiles(vec![stats(7, 1)])).await;
        from_gc(
            cm,
            PLAYERS_PROFILE,
            &profiles(vec![stats(PLAYER_ACCOUNT, 21), stats(7, 1)]),
        )
        .await;
    };
    let (profile, ()) = tokio::join!(soon(session.cs2.player_profile(SteamID::from_steam64(PLAYER))), gc);
    let profile = profile.unwrap();
    assert_eq!(profile.player(), Some(SteamID::from_steam64(PLAYER)));
    assert_eq!(profile.level(), Some(21));
    // the others are passed on
    let others: Vec<_> = waiting(&mut session).await.iter().map(other_type).collect();
    assert_eq!(others, [Some(PLAYERS_PROFILE), Some(PLAYERS_PROFILE)]);
}

#[tokio::test(start_paused = true)]
async fn requests_about_players_need_individual_accounts() {
    let mut session = session().await;
    let clan = SteamID::from_steam64(103582791429521412);
    assert!(matches!(
        soon(session.cs2.player_profile(clan)).await,
        Err(ClientError::NotIndividual(id)) if id == clan
    ));
    let nobody = SteamID::from_account_id(0);
    assert!(matches!(
        soon(session.cs2.recent_matches(nobody)).await,
        Err(ClientError::NotIndividual(_))
    ));
    assert!(matches!(
        soon(session.cs2.live_match(clan)).await,
        Err(ClientError::NotIndividual(_))
    ));
    quiet(&mut session.cm, Duration::from_secs(30)).await;
}

#[tokio::test(start_paused = true)]
async fn a_profile_request_times_out() {
    let mut session = session().await;
    let player = SteamID::from_steam64(PLAYER);
    let (result, (_, sent)) = tokio::join!(
        soon(session.cs2.player_profile(player)),
        read_request::<CMsgGCCStrike15_v2_ClientRequestPlayersProfile>(&mut session.cm, REQUEST_PLAYERS_PROFILE)
    );
    assert!(matches!(result, Err(ClientError::GcTimeout(PLAYERS_PROFILE))));
    assert_eq!(sent.elapsed(), INSPECT_TIMEOUT);
}

#[tokio::test(start_paused = true)]
async fn asks_for_a_match_by_its_share_code() {
    let mut session = session().await;
    let code: ShareCode = SHARE_CODE.parse().unwrap();
    let cm = &mut session.cm;
    let gc = async {
        let (request, _) = read_request::<CMsgGCCStrike15_v2_MatchListRequestFullGameInfo>(cm, FULL_GAME_INFO).await;
        assert_eq!(
            (request.matchid(), request.outcomeid(), request.token()),
            (SHARED_MATCH, 3400367402569957763, 9725)
        );
        // the answer to another request, about another match, then the one asked
        from_gc(
            cm,
            MATCH_LIST,
            &list(RECENT_GAMES, Some(PLAYER_ACCOUNT), vec![finished(SHARED_MATCH)]),
        )
        .await;
        from_gc(cm, MATCH_LIST, &list(FULL_GAME_INFO, None, vec![finished(1)])).await;
        from_gc(
            cm,
            MATCH_LIST,
            &list(FULL_GAME_INFO, None, vec![finished(SHARED_MATCH)]),
        )
        .await;
    };
    let (matches, ()) = tokio::join!(soon(session.cs2.match_info(&code)), gc);
    let matches = matches.unwrap();
    assert_eq!(matches.raw().msgrequestid(), FULL_GAME_INFO);
    let info = matches.matches().next().unwrap();
    assert_eq!(info.match_id(), Some(SHARED_MATCH));
    assert_eq!(info.share_code(), Some(code));
    assert!(info.demo_url().unwrap().ends_with(".dem.bz2"));
    let others: Vec<_> = waiting(&mut session).await.iter().map(other_type).collect();
    assert_eq!(others, [Some(MATCH_LIST), Some(MATCH_LIST)]);

    // the game coordinator found none
    let cm = &mut session.cm;
    let gc = async {
        read_request::<CMsgGCCStrike15_v2_MatchListRequestFullGameInfo>(cm, FULL_GAME_INFO).await;
        from_gc(cm, MATCH_LIST, &list(FULL_GAME_INFO, None, vec![])).await;
    };
    let (matches, ()) = tokio::join!(soon(session.cs2.match_info(&code)), gc);
    assert_eq!(matches.unwrap().matches().len(), 0);
}

#[tokio::test(start_paused = true)]
async fn asks_for_recent_and_live_matches() {
    let mut session = session().await;
    let player = SteamID::from_steam64(PLAYER);
    let cm = &mut session.cm;
    let gc = async {
        let (request, _) = read_request::<CMsgGCCStrike15_v2_MatchListRequestRecentUserGames>(cm, RECENT_GAMES).await;
        assert_eq!(request.accountid(), PLAYER_ACCOUNT);
        // about someone else, then the player
        from_gc(cm, MATCH_LIST, &list(RECENT_GAMES, Some(7), vec![finished(3)])).await;
        from_gc(
            cm,
            MATCH_LIST,
            &list(RECENT_GAMES, Some(PLAYER_ACCOUNT), vec![finished(1), finished(2)]),
        )
        .await;
    };
    let (recent, ()) = tokio::join!(soon(session.cs2.recent_matches(player)), gc);
    let recent = recent.unwrap();
    assert_eq!(recent.player(), Some(player));
    let ids: Vec<_> = recent.matches().map(|info| info.match_id().unwrap()).collect();
    assert_eq!(ids, [1, 2]);

    let cm = &mut session.cm;
    let gc = async {
        let (request, _) = read_request::<CMsgGCCStrike15_v2_MatchListRequestCurrentLiveGames>(cm, LIVE_GAMES).await;
        assert_eq!(request.compute_size(), 0);
        from_gc(cm, MATCH_LIST, &list(LIVE_GAMES, None, vec![finished(4)])).await;
    };
    let (live, ()) = tokio::join!(soon(session.cs2.live_matches()), gc);
    assert_eq!(live.unwrap().matches().len(), 1);

    // which the game coordinator doesn't answer any more, ValvePython says
    let cm = &mut session.cm;
    let gc = async {
        let (request, _) =
            read_request::<CMsgGCCStrike15_v2_MatchListRequestLiveGameForUser>(cm, LIVE_GAME_FOR_USER).await;
        assert_eq!(request.accountid(), PLAYER_ACCOUNT);
        from_gc(cm, MATCH_LIST, &list(LIVE_GAMES, None, vec![])).await;
    };
    let (live, ()) = tokio::join!(soon(session.cs2.live_match(player)), gc);
    assert!(matches!(live, Err(ClientError::GcTimeout(MATCH_LIST))));
}

#[tokio::test(start_paused = true)]
async fn every_kind_of_request_waits_the_same_cooldown() {
    let mut session = session().await;
    let cm = &mut session.cm;
    let cs2 = &mut session.cs2;
    let gc = async {
        let (_, inspected) = request_read(cm).await;
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(ASSET)))).await;
        let (_, profiled) =
            read_request::<CMsgGCCStrike15_v2_ClientRequestPlayersProfile>(cm, REQUEST_PLAYERS_PROFILE).await;
        from_gc(cm, PLAYERS_PROFILE, &profiles(vec![stats(PLAYER_ACCOUNT, 1)])).await;
        let (_, listed) = read_request::<CMsgGCCStrike15_v2_MatchListRequestCurrentLiveGames>(cm, LIVE_GAMES).await;
        from_gc(cm, MATCH_LIST, &list(LIVE_GAMES, None, vec![])).await;
        [profiled - inspected, listed - profiled]
    };
    let requests = async {
        soon(inspect(cs2, inventory(ASSET))).await.unwrap();
        soon(cs2.player_profile(SteamID::from_steam64(PLAYER))).await.unwrap();
        soon(cs2.live_matches()).await.unwrap();
    };
    let (gaps, ()) = tokio::join!(gc, requests);
    assert_eq!(gaps, [INSPECT_COOLDOWN, INSPECT_COOLDOWN]);
}

#[tokio::test(start_paused = true)]
async fn keeps_what_comes_while_a_request_waits() {
    let mut session = session().await;
    let account = session.handle.steam_id();
    let cm = &mut session.cm;
    let gc = async {
        request_read(cm).await;
        from_gc(cm, SO_CREATE, &object(1, &inventory::tests::item(5), Some(account))).await;
        from_gc(cm, 9999, &stats(1, 1)).await;
        from_gc(cm, MATCHMAKING_HELLO, &stats(PLAYER_ACCOUNT, 3)).await;
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(ASSET + 1)))).await;
        from_gc(cm, PREVIEW_RESPONSE, &answer(Some(item(ASSET)))).await;
    };
    let (details, ()) = tokio::join!(soon(inspect(&mut session.cs2, inventory(ASSET))), gc);
    assert_eq!(details.unwrap().item_id(), Some(ASSET));
    assert_eq!(ids(session.cs2.inventory()), [5]);
    let events = waiting(&mut session).await;
    match events.as_slice() {
        [Cs2Event::ItemAcquired(item), Cs2Event::Other(unknown), Cs2Event::MatchmakingStats(stats), Cs2Event::Other(late)] =>
        {
            assert_eq!(item.id(), 5);
            assert_eq!(unknown.msg_type(), 9999);
            assert_eq!(stats.level(), Some(3));
            assert_eq!(late.msg_type(), PREVIEW_RESPONSE);
        }
        events => panic!("unexpected events: {:?}", events),
    }
}

#[tokio::test(start_paused = true)]
async fn follows_the_inventory() {
    let mut session =
        session_welcomed(|account| welcome_with(account, &[inventory::tests::item(1), inventory::tests::item(2)]))
            .await;
    let account = session.handle.steam_id();
    let mut moved = inventory::tests::item(1);
    moved.set_inventory(8);
    let mut gone = CSOEconItem::new();
    gone.set_id(2);
    let mut unknown = CSOEconItem::new();
    unknown.set_id(77);
    let cm = &mut session.cm;
    from_gc(cm, SO_CREATE, &object(1, &inventory::tests::item(3), Some(account))).await;
    from_gc(cm, SO_UPDATE, &object(1, &moved, None)).await;
    from_gc(cm, SO_DESTROY, &object(1, &gone, Some(account))).await;
    // nothing to remove; nothing changed
    from_gc(cm, SO_DESTROY, &object(1, &unknown, Some(account))).await;
    from_gc(cm, SO_UPDATE, &object(1, &moved, None)).await;
    // someone else's, and not an item: passed on
    from_gc(
        cm,
        SO_CREATE,
        &object(1, &inventory::tests::item(4), Some(SteamID::from_account_id(5))),
    )
    .await;
    from_gc(cm, SO_CREATE, &object(7, &inventory::tests::item(4), None)).await;
    let events = waiting(&mut session).await;
    match events.as_slice() {
        [Cs2Event::ItemAcquired(acquired), Cs2Event::ItemChanged { old, new }, Cs2Event::ItemRemoved(removed), Cs2Event::Other(someones), Cs2Event::Other(not_item)] =>
        {
            assert_eq!(acquired.id(), 3);
            assert_eq!((old.id(), old.position(), new.position()), (1, Some(3), Some(8)));
            // the item as it was: the message only names it
            assert_eq!((removed.id(), removed.paint_index()), (2, Some(44)));
            assert_eq!((someones.msg_type(), not_item.msg_type()), (SO_CREATE, SO_CREATE));
        }
        events => panic!("unexpected events: {:?}", events),
    }
    assert_eq!(ids(session.cs2.inventory()), [1, 3]);
    assert_eq!(session.cs2.inventory().get(1).unwrap().position(), Some(8));

    // the whole inventory again
    let cache = inventory::tests::cache(Some(account), &[inventory::tests::item(6)]);
    from_gc(&mut session.cm, SO_CACHE_SUBSCRIBED, &cache).await;
    // several objects at once
    let mut objects = CMsgSOMultipleObjects::new();
    let mut single = cmsg_somultiple_objects::SingleObject::new();
    single.set_type_id(1);
    single.set_object_data(inventory::tests::object(&inventory::tests::item(7)));
    objects.objects_modified.push(single);
    from_gc(&mut session.cm, SO_UPDATE_MULTIPLE, &objects).await;
    let events = waiting(&mut session).await;
    assert!(
        matches!(events.as_slice(), [Cs2Event::InventoryLoaded, Cs2Event::ItemAcquired(item)] if item.id() == 7),
        "{:?}",
        events
    );
    assert_eq!(ids(session.cs2.inventory()), [6, 7]);
}

#[tokio::test(start_paused = true)]
async fn passes_on_messages_it_doesnt_read() {
    let mut session = session().await;
    // `ItemCustomizationNotification`
    from_gc(&mut session.cm, 1090, &stats(1, 1)).await;
    match soon(session.cs2.next_event()).await.unwrap() {
        Cs2Event::Other(message) => {
            assert_eq!(message.msg_type(), 1090);
            assert!(message.is_proto());
            assert_eq!(
                message
                    .parse::<CMsgGCCStrike15_v2_MatchmakingGC2ClientHello>()
                    .unwrap()
                    .player_level(),
                1
            );
        }
        event => panic!("unexpected event: {:?}", event),
    }
    assert!(session.cs2.try_next_event().unwrap().is_none());
}

#[tokio::test(start_paused = true)]
async fn greets_again_while_waiting_for_events() {
    let mut session = session().await;
    session.cm = reconnected_cm(&mut session).await;
    assert!(matches!(
        soon(session.cs2.next_event()).await,
        Ok(Cs2Event::SessionLost)
    ));
    assert!(!session.cs2.has_session());

    let account = session.handle.steam_id();
    let cm = &mut session.cm;
    let gc = async {
        let start = Instant::now();
        hello_read(cm).await;
        let first = start.elapsed();
        hello_read(cm).await;
        from_gc(cm, CLIENT_WELCOME, &welcome_with(account, &[inventory::tests::item(4)])).await;
        (first, start.elapsed())
    };
    let (event, hellos) = tokio::join!(soon(session.cs2.next_event()), gc);
    assert!(matches!(event, Ok(Cs2Event::Welcomed)));
    assert_eq!(hellos, (Duration::from_millis(500), Duration::from_millis(2500)));
    assert!(session.cs2.has_session());
    assert_eq!(ids(session.cs2.inventory()), [4]);
    // welcomed: waits without greeting
    let wait = timeout(Duration::from_secs(120), session.cs2.next_event());
    let (event, ()) = tokio::join!(wait, quiet(&mut session.cm, Duration::from_secs(110)));
    assert!(event.is_err());
}

#[tokio::test(start_paused = true)]
async fn a_status_without_session_is_an_event_then_a_greeting() {
    let mut session = session().await;
    from_gc(
        &mut session.cm,
        CONNECTION_STATUS,
        &status(GCConnectionStatus::GCConnectionStatus_NO_SESSION as i32),
    )
    .await;
    // an unasked welcome
    welcome(&mut session.cm).await;
    from_gc(
        &mut session.cm,
        CONNECTION_STATUS,
        &status(GCConnectionStatus::GCConnectionStatus_GC_GOING_DOWN as i32),
    )
    .await;
    let events = waiting(&mut session).await;
    assert!(
        matches!(
            events.as_slice(),
            [Cs2Event::SessionLost, Cs2Event::Welcomed, Cs2Event::SessionLost]
        ),
        "{:?}",
        events
    );

    let cm = &mut session.cm;
    let gc = async {
        hello_read(cm).await;
        let mut error = CMsgGCCStrike15_v2_ClientLogonFatalError::new();
        error.set_errorcode(4);
        from_gc(cm, LOGON_FATAL_ERROR, &error).await;
    };
    let (refused, ()) = tokio::join!(soon(session.cs2.next_event()), gc);
    assert!(matches!(refused, Err(ClientError::GcRefused { code: 4, .. })));
    // greets again at the next call
    let cm = &mut session.cm;
    let gc = async {
        hello_read(cm).await;
        welcome(cm).await;
    };
    let (event, ()) = tokio::join!(soon(session.cs2.next_event()), gc);
    assert!(matches!(event, Ok(Cs2Event::Welcomed)));
}

#[tokio::test(start_paused = true)]
async fn the_events_end_once_the_client_stopped() {
    let Session { mut cs2, handle, .. } = session().await;
    assert!(cs2.try_next_event().unwrap().is_none());
    drop(handle);
    // the client stopping may end the session first
    let mut event = soon(cs2.next_event()).await;
    if let Ok(Cs2Event::SessionLost) = event {
        event = soon(cs2.next_event()).await;
    }
    assert!(matches!(event, Err(ClientError::Closed)), "{:?}", event);
    assert!(matches!(cs2.try_next_event(), Err(ClientError::Closed)));
}

#[tokio::test(start_paused = true)]
async fn counts_the_messages_the_channel_dropped() {
    let (commands, _sent) = mpsc::unbounded_channel();
    let (channel, route) = gc::channel(APP_ID, commands.downgrade());
    let mut cs2 = Cs2::new(channel, SteamID::from_account_id(1));
    cs2.session = super::Session::Welcomed;
    route.queue.lock().dropped = 3;
    assert!(route.queue.push(gc_message(9999, &stats(1, 1))));
    assert!(matches!(cs2.try_next_event(), Ok(Some(Cs2Event::Lagged(3)))));
    assert_eq!(cs2.try_next_event().unwrap().as_ref().and_then(other_type), Some(9999));
    assert!(cs2.try_next_event().unwrap().is_none());
}

#[test]
fn keeps_events_up_to_the_capacity() {
    let mut events = EventQueue::default();
    let message = gc_message(9999, &stats(1, 1));
    for _ in 0..EVENT_CAPACITY + 2 {
        events.push(Cs2Event::Other(message.clone()));
    }
    // about the session: never dropped, one is enough
    events.push(Cs2Event::SessionLost);
    events.push(Cs2Event::SessionLost);
    events.push(Cs2Event::Welcomed);
    // still full
    events.push(Cs2Event::Other(message.clone()));
    let mut others = 0;
    while let Some(Cs2Event::Other(_)) = events.events.front() {
        events.pop();
        others += 1;
    }
    assert_eq!(others, EVENT_CAPACITY);
    let rest: Vec<_> = std::iter::from_fn(|| events.pop()).collect();
    assert!(
        matches!(
            rest.as_slice(),
            [
                Cs2Event::Lagged(2),
                Cs2Event::SessionLost,
                Cs2Event::Welcomed,
                Cs2Event::Lagged(1)
            ]
        ),
        "{:?}",
        rest
    );

    // counted where they were missed, even with nothing after them
    events.missed(5);
    assert!(matches!(events.pop(), Some(Cs2Event::Lagged(5))));
    assert!(events.pop().is_none());
}

#[tokio::test(start_paused = true)]
async fn greets_from_the_start_once_the_connection_to_steam_was_lost() {
    let (commands, mut sent) = mpsc::unbounded_channel();
    let (channel, route) = gc::channel(APP_ID, commands.downgrade());
    let mut cs2 = Cs2::new(channel, SteamID::from_account_id(1));
    cs2.welcomed_before = true;
    // greeting for a while: the next hello in half a minute
    cs2.session = super::Session::Greeting {
        next_hello: Instant::now() + Duration::from_secs(30),
        interval: MAX_HELLO_INTERVAL,
    };
    route.queue.session_lost();
    let start = Instant::now();
    let gc = async {
        let Some(Command::ToGc(hello)) = sent.recv().await else {
            panic!("expected a message to the game coordinator");
        };
        assert_eq!(hello.msgtype(), CLIENT_HELLO | 0x8000_0000);
        let at = start.elapsed();
        assert!(route.queue.push(gc_message(CLIENT_WELCOME, &CMsgClientWelcome::new())));
        at
    };
    let (event, at) = tokio::join!(soon(cs2.next_event()), gc);
    assert!(matches!(event, Ok(Cs2Event::Welcomed)), "{:?}", event);
    // hellos sent while connecting again were dropped: from the first one again
    assert_eq!(at, FIRST_HELLO);
}
