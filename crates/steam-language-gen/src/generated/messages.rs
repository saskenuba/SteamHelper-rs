use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::generated::enums::*;

pub trait SerializableMessageBody {
	fn serialize(&self) -> Vec<u8>;
	fn deserialize_struct(packet_data: &[u8]) -> Self;
}


#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientJustStrings {
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientGenericResponse {
	result: EResult,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgChannelEncryptRequest {
	protocol_version: u32,
	universe: EUniverse,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgChannelEncryptResponse {
	protocol_version: u32,
	key_size: u32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgChannelEncryptResult {
	result: EResult,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientNewLoginKey {
	unique_id: u32,
	login_key: [u8; 20],
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientNewLoginKeyAccepted {
	unique_id: u32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientLogon {
	obfuscation_mask: u32,
	current_protocol: u32,
	protocol_ver_major_mask: u32,
	protocol_ver_minor_mask: u32,
	protocol_ver_minor_min_game_servers: u16,
	protocol_ver_minor_min_for_supporting_e_msg_multi: u16,
	protocol_ver_minor_min_for_supporting_e_msg_client_encrypt_pct: u16,
	protocol_ver_minor_min_for_extended_msg_hdr: u16,
	protocol_ver_minor_min_for_cell_id: u16,
	protocol_ver_minor_min_for_session_id_last: u16,
	protocol_ver_minor_min_for_server_availablity_msgs: u16,
	protocol_ver_minor_min_clients: u16,
	protocol_ver_minor_min_for_os_type_: u16,
	protocol_ver_minor_min_for_ceg_apply_pe_sig: u16,
	protocol_ver_minor_min_for_marketing_messages_2: u16,
	protocol_ver_minor_min_for_any_proto_buf_messages: u16,
	protocol_ver_minor_min_for_proto_buf_logged_off_message: u16,
	protocol_ver_minor_min_for_proto_buf_multi_messages: u16,
	protocol_ver_minor_min_for_sending_protocol_to_ufs: u16,
	protocol_ver_minor_min_for_machine_auth: u16,
	protocol_ver_minor_min_for_session_id_last_anon: u16,
	protocol_ver_minor_min_for_enhanced_app_list: u16,
	protocol_ver_minor_min_for_steam_guard_notification_ui: u16,
	protocol_ver_minor_min_for_proto_buf_service_module_calls: u16,
	protocol_ver_minor_min_for_gzip_multi_messages: u16,
	protocol_ver_minor_min_for_new_voice_call_authorize: u16,
	protocol_ver_minor_min_for_client_instance_i_ds: u16,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientVACBanStatus {
	num_bans: u32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientAppUsageEvent {
	app_usage_event: EAppUsageEvent,
	game_id: u64,
	offline: u16,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientEmailAddrInfo {
	password_strength: u32,
	flags_account_security_policy: u32,
	validated: u8,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientUpdateGuestPassesList {
	result: EResult,
	count_guest_passes_to_give: i32,
	count_guest_passes_to_redeem: i32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientRequestedClientStats {
	count_stats: i32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientP2PIntroducerMessage {
	steam_id: u64,
	routing_type_: EIntroducerRouting,
	data: [u8; 1450],
	data_len: u32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientOGSBeginSession {
	account_type_: u8,
	account_id: u64,
	app_id: u32,
	time_started: u32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientOGSBeginSessionResponse {
	result: EResult,
	collecting_any: u8,
	collecting_details: u8,
	session_id: u64,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientOGSEndSession {
	session_id: u64,
	time_ended: u32,
	reason_code: i32,
	count_attributes: i32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientOGSEndSessionResponse {
	result: EResult,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientOGSWriteRow {
	session_id: u64,
	count_attributes: i32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientGetFriendsWhoPlayGame {
	game_id: u64,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientGetFriendsWhoPlayGameResponse {
	result: EResult,
	game_id: u64,
	count_friends: u32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgGSPerformHardwareSurvey {
	flags: u32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgGSGetPlayStatsResponse {
	result: EResult,
	rank: i32,
	lifetime_connects: u32,
	lifetime_minutes_played: u32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgGSGetReputationResponse {
	result: EResult,
	reputation_score: u32,
	banned: u8,
	banned_ip: u32,
	banned_port: u16,
	banned_game_id: u64,
	time_ban_expires: u32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgGSDeny {
	steam_id: u64,
	deny_reason: EDenyReason,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgGSApprove {
	steam_id: u64,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgGSKick {
	steam_id: u64,
	deny_reason: EDenyReason,
	wait_til_map_change: i32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgGSGetUserGroupStatus {
	steam_id_user: u64,
	steam_id_group: u64,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgGSGetUserGroupStatusResponse {
	steam_id_user: u64,
	steam_id_group: u64,
	clan_relationship: EClanRelationship,
	clan_rank: EClanRank,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientJoinChat {
	steam_id_chat: u64,
	is_voice_speaker: u8,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientChatEnter {
	steam_id_chat: u64,
	steam_id_friend: u64,
	chat_room_type_: EChatRoomType,
	steam_id_owner: u64,
	steam_id_clan: u64,
	chat_flags: u8,
	enter_response: EChatRoomEnterResponse,
	num_members: i32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientChatMsg {
	steam_id_chatter: u64,
	steam_id_chat_room: u64,
	chat_msg_type_: EChatEntryType,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientChatMemberInfo {
	steam_id_chat: u64,
	type_: EChatInfoType,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientChatAction {
	steam_id_chat: u64,
	steam_id_user_to_act_on: u64,
	chat_action: EChatAction,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientChatActionResult {
	steam_id_chat: u64,
	steam_id_user_acted_on: u64,
	chat_action: EChatAction,
	action_result: EChatActionResult,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientChatRoomInfo {
	steam_id_chat: u64,
	type_: EChatInfoType,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientSetIgnoreFriend {
	my_steam_id: u64,
	steam_id_friend: u64,
	ignore: u8,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientSetIgnoreFriendResponse {
	friend_id: u64,
	result: EResult,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientLoggedOff {
	result: EResult,
	sec_min_reconnect_hint: i32,
	sec_max_reconnect_hint: i32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientLogOnResponse {
	result: EResult,
	out_of_game_heartbeat_rate_sec: i32,
	in_game_heartbeat_rate_sec: i32,
	client_supplied_steam_id: u64,
	ip_public: u32,
	server_real_time: u32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientSendGuestPass {
	gift_id: u64,
	gift_type_: u8,
	account_id: u32,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientSendGuestPassResponse {
	result: EResult,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientServerUnavailable {
	jobid_sent: u64,
	e_msg_sent: u32,
	e_server_type_unavailable: EServerType,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientCreateChat {
	chat_room_type_: EChatRoomType,
	game_id: u64,
	steam_id_clan: u64,
	permission_officer: EChatPermission,
	permission_member: EChatPermission,
	permission_all: EChatPermission,
	members_max: u32,
	chat_flags: u8,
	steam_id_friend_chat: u64,
	steam_id_invited: u64,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientCreateChatResponse {
	result: EResult,
	steam_id_chat: u64,
	chat_room_type_: EChatRoomType,
	steam_id_friend_chat: u64,
}

#[derive(Debug, Serialize, Deserialize, SteamMsg)]
struct MsgClientMarketingMessageUpdate2 {
	marketing_message_update_time: u32,
	count: u32,
}

