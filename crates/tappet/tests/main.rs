use compile_fail::compile_fail;

use tappet::SteamAPI;

// fn should_appear_in_post_namespace() {
//     let client = SteamAPI::new(std::env::var("STEAM_API").unwrap());
//     client.post().IEconService().DeclineTradeOffer(512565);
// }

#[compile_fail]
fn should_not_be_get_namespaced() {
    let client = SteamAPI::new(std::env!("STEAM_API"));
    client.get().IEconService().DeclineTradeOffer(512565);
}
