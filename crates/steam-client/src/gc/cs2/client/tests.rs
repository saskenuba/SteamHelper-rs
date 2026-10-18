use bytes::Bytes;
use steam_language_gen::generated::enums::EMsg;
use steam_protobuf::protobuf::EnumOrUnknown;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::cecon_item_preview_data_block::Sticker as ProtoSticker;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CEconItemPreviewDataBlock;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgClientWelcome;
use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::protobufs::steammessages_clientserver::CMsgClientGamesPlayed;
use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgGCClient;
use tokio::sync::mpsc;
use tokio::time::timeout;

use super::*;
use crate::connection::tests::FakeCm;
use crate::events::Event;
use crate::gc;
use crate::gc::cs2::InspectLinkError;
use crate::handle::tests::next;
use crate::handle::tests::reconnected;
use crate::handle::tests::recv;
use crate::handle::tests::relogs;
use crate::handle::tests::round_trip;
use crate::connection::tests::soon;
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
    let Spawned {
        handle,
        events,
        mut cm,
        reconnects,
        ..
    } = spawned_as(user()).await;
    let gc = async {
        hello_read(&mut cm).await;
        welcome(&mut cm).await;
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
    let stickers: Vec<_> = details.stickers().map(|sticker| (sticker.slot, sticker.id, sticker.wear)).collect();
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
    let Session { cs2, mut cm, handle, .. } = session().await;
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
    let Session { cs2, mut cm, handle, .. } = session().await;
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
    let old = std::mem::replace(&mut session.cm, placeholder());
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
    let details = soon(inspect(&mut session.cs2, EMBEDDED.parse().unwrap())).await.unwrap();
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
    let mut cs2 = Cs2::new(channel);
    cs2.ready = true;
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
    assert!(cs2.ready);
}

#[tokio::test(start_paused = true)]
async fn a_session_lost_while_waiting_is_greeted_again_by_the_next_inspect() {
    let (commands, mut sent) = mpsc::unbounded_channel();
    let (channel, route) = gc::channel(APP_ID, commands.downgrade());
    let mut cs2 = Cs2::new(channel).with_timeout(Duration::from_secs(1));
    cs2.ready = true;
    let gc = async {
        assert!(matches!(sent.recv().await, Some(Command::ToGc(_))));
        route.queue.session_lost();
    };
    let (result, ()) = tokio::join!(soon(inspect(&mut cs2, inventory(ASSET))), gc);
    assert!(matches!(result, Err(ClientError::GcTimeout(PREVIEW_RESPONSE))));
    assert!(!cs2.ready);
    drop(route);
    // the channel ended: greeting fails at once
    assert!(matches!(
        soon(inspect(&mut cs2, inventory(ASSET))).await,
        Err(ClientError::Closed)
    ));
}
