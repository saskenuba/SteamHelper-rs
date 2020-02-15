//! Handle events through [PacketMessage] matching.
//!
//!


pub enum SteamEvents {
    SteamFriends,
    SteamUser,
    SteamClient,
}

trait HandlerKind {
    fn handle_msg() {}
}

struct SteamClient {
}

/// client of lib should implement this
impl UserCallbacks for SteamClient {

    fn on_login(callback: Box<dyn FnOnce() -> ()>) { println!("wot!") }
}


trait UserCallbacks {
    fn on_login(callback: Box<dyn FnOnce() -> ()>) {
        println!("wot!")
    }
    fn on_logout(callback: Box<dyn FnOnce() -> ()>) { unimplemented!() }
}

trait FriendsCallbacks {
    fn etc(callback: Box<dyn FnOnce() -> ()>) { unimplemented!() }
}
