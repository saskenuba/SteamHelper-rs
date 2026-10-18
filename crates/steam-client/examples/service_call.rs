//! Calls service methods: one the crate knows before logon, and one it doesn't know once logged on anonymously.
//!
//! Run with `cargo run -p steam-client-lib --example service_call` (`RUST_LOG=steam_client=debug` for the connection's
//! log). No variables. Expect the size and exponent of the RSA key Steam encrypts a made-up account's password with,
//! then the first few content servers Steam lists for our cell, then `Logged off`.
//!
//! # Calling a method the crate doesn't know
//!
//! A method is named by a marker type that implements [`ServiceMethod`]: its name, and its request and response
//! types. The crate's own are in `steam_client::service::methods`, and `call_service_method(request)` finds them from
//! the request. For another method, declare a marker type in your crate, as `GetServersForSteamPipe` below, and call
//! `call::<GetServersForSteamPipe>(request)`. (The trait can't go on steam-protobuf's request type itself in your
//! crate: Rust's orphan rule forbids implementing a foreign trait for a foreign type.)

use steam_client::client::SteamClient;
use steam_client::errors::ClientError;
use steam_client::service::ServiceMethod;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Request;
use steam_protobuf::protobufs::steammessages_contentsystem_steamclient::CContentServerDirectory_GetServersForSteamPipe_Request;
use steam_protobuf::protobufs::steammessages_contentsystem_steamclient::CContentServerDirectory_GetServersForSteamPipe_Response;

/// `ContentServerDirectory.GetServersForSteamPipe`, which the crate doesn't wrap: the content servers to download
/// games from. Anonymous users may call it.
enum GetServersForSteamPipe {}

impl ServiceMethod for GetServersForSteamPipe {
    const METHOD: &'static str = "ContentServerDirectory.GetServersForSteamPipe#1";
    type Request = CContentServerDirectory_GetServersForSteamPipe_Request;
    type Response = CContentServerDirectory_GetServersForSteamPipe_Response;
}

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    env_logger::init();
    let mut client = SteamClient::connect().await?;
    println!("Connected to {}", client.endpoint());

    // before logon: a non-authed method the crate knows, for an account that doesn't exist
    let mut request = CAuthentication_GetPasswordRSAPublicKey_Request::new();
    request.set_account_name("steamhelper-rs-nonexistent-account".to_string());
    let key = client.call_service_method(request).await?;
    println!(
        "Authentication.GetPasswordRSAPublicKey: a {}-bit key, exponent 0x{}",
        key.publickey_mod().len() * 4,
        key.publickey_exp()
    );

    // once logged on: a method the crate doesn't know, through its marker type
    let mut client = client.login_anonymous().await?;
    let mut request = CContentServerDirectory_GetServersForSteamPipe_Request::new();
    request.set_cell_id(client.cell_id());
    request.set_max_servers(5);
    let response = client.call::<GetServersForSteamPipe>(request).await?;
    println!(
        "{}: {} servers for cell {}",
        GetServersForSteamPipe::METHOD,
        response.servers().len(),
        client.cell_id()
    );
    for server in response.servers() {
        println!(
            "  {} {} (cell {}, load {}, https {})",
            server.type_(),
            server.host(),
            server.cell_id(),
            server.load(),
            server.https_support()
        );
    }

    client.log_off().await?;
    println!("Logged off");
    Ok(())
}
