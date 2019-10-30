// This file is generated by rust-protobuf 2.8.1. Do not edit
// @generated

// https://github.com/Manishearth/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy::all)]

#![cfg_attr(rustfmt, rustfmt_skip)]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unsafe_code)]
#![allow(unused_imports)]
#![allow(unused_results)]
//! Generated file from `steammessages_lobbymatchmaking.steamclient.proto`

use protobuf::Message as Message_imported_for_functions;
use protobuf::ProtobufEnum as ProtobufEnum_imported_for_functions;

/// Generated files are compatible only with the same version
/// of protobuf runtime.
const _PROTOBUF_VERSION_CHECK: () = ::protobuf::VERSION_2_8_1;

#[derive(PartialEq,Clone,Default)]
#[cfg_attr(feature = "with-serde", derive(Serialize, Deserialize))]
pub struct LobbyMatchmakingLegacy_GetLobbyStatus_Request {
    // message fields
    app_id: ::std::option::Option<u32>,
    steamid_lobby: ::std::option::Option<u64>,
    claim_ownership: ::std::option::Option<bool>,
    claim_membership: ::std::option::Option<bool>,
    version_num: ::std::option::Option<u32>,
    // special fields
    #[cfg_attr(feature = "with-serde", serde(skip))]
    pub unknown_fields: ::protobuf::UnknownFields,
    #[cfg_attr(feature = "with-serde", serde(skip))]
    pub cached_size: ::protobuf::CachedSize,
}

impl<'a> ::std::default::Default for &'a LobbyMatchmakingLegacy_GetLobbyStatus_Request {
    fn default() -> &'a LobbyMatchmakingLegacy_GetLobbyStatus_Request {
        <LobbyMatchmakingLegacy_GetLobbyStatus_Request as ::protobuf::Message>::default_instance()
    }
}

impl LobbyMatchmakingLegacy_GetLobbyStatus_Request {
    pub fn new() -> LobbyMatchmakingLegacy_GetLobbyStatus_Request {
        ::std::default::Default::default()
    }

    // optional uint32 app_id = 1;


    pub fn get_app_id(&self) -> u32 {
        self.app_id.unwrap_or(0)
    }
    pub fn clear_app_id(&mut self) {
        self.app_id = ::std::option::Option::None;
    }

    pub fn has_app_id(&self) -> bool {
        self.app_id.is_some()
    }

    // Param is passed by value, moved
    pub fn set_app_id(&mut self, v: u32) {
        self.app_id = ::std::option::Option::Some(v);
    }

    // optional fixed64 steamid_lobby = 2;


    pub fn get_steamid_lobby(&self) -> u64 {
        self.steamid_lobby.unwrap_or(0)
    }
    pub fn clear_steamid_lobby(&mut self) {
        self.steamid_lobby = ::std::option::Option::None;
    }

    pub fn has_steamid_lobby(&self) -> bool {
        self.steamid_lobby.is_some()
    }

    // Param is passed by value, moved
    pub fn set_steamid_lobby(&mut self, v: u64) {
        self.steamid_lobby = ::std::option::Option::Some(v);
    }

    // optional bool claim_ownership = 3;


    pub fn get_claim_ownership(&self) -> bool {
        self.claim_ownership.unwrap_or(false)
    }
    pub fn clear_claim_ownership(&mut self) {
        self.claim_ownership = ::std::option::Option::None;
    }

    pub fn has_claim_ownership(&self) -> bool {
        self.claim_ownership.is_some()
    }

    // Param is passed by value, moved
    pub fn set_claim_ownership(&mut self, v: bool) {
        self.claim_ownership = ::std::option::Option::Some(v);
    }

    // optional bool claim_membership = 4;


    pub fn get_claim_membership(&self) -> bool {
        self.claim_membership.unwrap_or(false)
    }
    pub fn clear_claim_membership(&mut self) {
        self.claim_membership = ::std::option::Option::None;
    }

    pub fn has_claim_membership(&self) -> bool {
        self.claim_membership.is_some()
    }

    // Param is passed by value, moved
    pub fn set_claim_membership(&mut self, v: bool) {
        self.claim_membership = ::std::option::Option::Some(v);
    }

    // optional uint32 version_num = 5;


    pub fn get_version_num(&self) -> u32 {
        self.version_num.unwrap_or(0)
    }
    pub fn clear_version_num(&mut self) {
        self.version_num = ::std::option::Option::None;
    }

    pub fn has_version_num(&self) -> bool {
        self.version_num.is_some()
    }

    // Param is passed by value, moved
    pub fn set_version_num(&mut self, v: u32) {
        self.version_num = ::std::option::Option::Some(v);
    }
}

impl ::protobuf::Message for LobbyMatchmakingLegacy_GetLobbyStatus_Request {
    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint32()?;
                    self.app_id = ::std::option::Option::Some(tmp);
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeFixed64 {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_fixed64()?;
                    self.steamid_lobby = ::std::option::Option::Some(tmp);
                },
                3 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_bool()?;
                    self.claim_ownership = ::std::option::Option::Some(tmp);
                },
                4 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_bool()?;
                    self.claim_membership = ::std::option::Option::Some(tmp);
                },
                5 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint32()?;
                    self.version_num = ::std::option::Option::Some(tmp);
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        if let Some(v) = self.app_id {
            my_size += ::protobuf::rt::value_size(1, v, ::protobuf::wire_format::WireTypeVarint);
        }
        if let Some(v) = self.steamid_lobby {
            my_size += 9;
        }
        if let Some(v) = self.claim_ownership {
            my_size += 2;
        }
        if let Some(v) = self.claim_membership {
            my_size += 2;
        }
        if let Some(v) = self.version_num {
            my_size += ::protobuf::rt::value_size(5, v, ::protobuf::wire_format::WireTypeVarint);
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        if let Some(v) = self.app_id {
            os.write_uint32(1, v)?;
        }
        if let Some(v) = self.steamid_lobby {
            os.write_fixed64(2, v)?;
        }
        if let Some(v) = self.claim_ownership {
            os.write_bool(3, v)?;
        }
        if let Some(v) = self.claim_membership {
            os.write_bool(4, v)?;
        }
        if let Some(v) = self.version_num {
            os.write_uint32(5, v)?;
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &dyn (::std::any::Any) {
        self as &dyn (::std::any::Any)
    }
    fn as_any_mut(&mut self) -> &mut dyn (::std::any::Any) {
        self as &mut dyn (::std::any::Any)
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        Self::descriptor_static()
    }

    fn new() -> LobbyMatchmakingLegacy_GetLobbyStatus_Request {
        LobbyMatchmakingLegacy_GetLobbyStatus_Request::new()
    }

    fn descriptor_static() -> &'static ::protobuf::reflect::MessageDescriptor {
        static mut descriptor: ::protobuf::lazy::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const ::protobuf::reflect::MessageDescriptor,
        };
        unsafe {
            descriptor.get(|| {
                let mut fields = ::std::vec::Vec::new();
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeUint32>(
                    "app_id",
                    |m: &LobbyMatchmakingLegacy_GetLobbyStatus_Request| { &m.app_id },
                    |m: &mut LobbyMatchmakingLegacy_GetLobbyStatus_Request| { &mut m.app_id },
                ));
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeFixed64>(
                    "steamid_lobby",
                    |m: &LobbyMatchmakingLegacy_GetLobbyStatus_Request| { &m.steamid_lobby },
                    |m: &mut LobbyMatchmakingLegacy_GetLobbyStatus_Request| { &mut m.steamid_lobby },
                ));
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeBool>(
                    "claim_ownership",
                    |m: &LobbyMatchmakingLegacy_GetLobbyStatus_Request| { &m.claim_ownership },
                    |m: &mut LobbyMatchmakingLegacy_GetLobbyStatus_Request| { &mut m.claim_ownership },
                ));
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeBool>(
                    "claim_membership",
                    |m: &LobbyMatchmakingLegacy_GetLobbyStatus_Request| { &m.claim_membership },
                    |m: &mut LobbyMatchmakingLegacy_GetLobbyStatus_Request| { &mut m.claim_membership },
                ));
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeUint32>(
                    "version_num",
                    |m: &LobbyMatchmakingLegacy_GetLobbyStatus_Request| { &m.version_num },
                    |m: &mut LobbyMatchmakingLegacy_GetLobbyStatus_Request| { &mut m.version_num },
                ));
                ::protobuf::reflect::MessageDescriptor::new::<LobbyMatchmakingLegacy_GetLobbyStatus_Request>(
                    "LobbyMatchmakingLegacy_GetLobbyStatus_Request",
                    fields,
                    file_descriptor_proto()
                )
            })
        }
    }

    fn default_instance() -> &'static LobbyMatchmakingLegacy_GetLobbyStatus_Request {
        static mut instance: ::protobuf::lazy::Lazy<LobbyMatchmakingLegacy_GetLobbyStatus_Request> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const LobbyMatchmakingLegacy_GetLobbyStatus_Request,
        };
        unsafe {
            instance.get(LobbyMatchmakingLegacy_GetLobbyStatus_Request::new)
        }
    }
}

impl ::protobuf::Clear for LobbyMatchmakingLegacy_GetLobbyStatus_Request {
    fn clear(&mut self) {
        self.app_id = ::std::option::Option::None;
        self.steamid_lobby = ::std::option::Option::None;
        self.claim_ownership = ::std::option::Option::None;
        self.claim_membership = ::std::option::Option::None;
        self.version_num = ::std::option::Option::None;
        self.unknown_fields.clear();
    }
}

impl ::std::fmt::Debug for LobbyMatchmakingLegacy_GetLobbyStatus_Request {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for LobbyMatchmakingLegacy_GetLobbyStatus_Request {
    fn as_ref(&self) -> ::protobuf::reflect::ProtobufValueRef {
        ::protobuf::reflect::ProtobufValueRef::Message(self)
    }
}

#[derive(PartialEq,Clone,Default)]
#[cfg_attr(feature = "with-serde", derive(Serialize, Deserialize))]
pub struct LobbyMatchmakingLegacy_GetLobbyStatus_Response {
    // message fields
    app_id: ::std::option::Option<u32>,
    steamid_lobby: ::std::option::Option<u64>,
    lobby_status: ::std::option::Option<ELobbyStatus>,
    // special fields
    #[cfg_attr(feature = "with-serde", serde(skip))]
    pub unknown_fields: ::protobuf::UnknownFields,
    #[cfg_attr(feature = "with-serde", serde(skip))]
    pub cached_size: ::protobuf::CachedSize,
}

impl<'a> ::std::default::Default for &'a LobbyMatchmakingLegacy_GetLobbyStatus_Response {
    fn default() -> &'a LobbyMatchmakingLegacy_GetLobbyStatus_Response {
        <LobbyMatchmakingLegacy_GetLobbyStatus_Response as ::protobuf::Message>::default_instance()
    }
}

impl LobbyMatchmakingLegacy_GetLobbyStatus_Response {
    pub fn new() -> LobbyMatchmakingLegacy_GetLobbyStatus_Response {
        ::std::default::Default::default()
    }

    // optional uint32 app_id = 1;


    pub fn get_app_id(&self) -> u32 {
        self.app_id.unwrap_or(0)
    }
    pub fn clear_app_id(&mut self) {
        self.app_id = ::std::option::Option::None;
    }

    pub fn has_app_id(&self) -> bool {
        self.app_id.is_some()
    }

    // Param is passed by value, moved
    pub fn set_app_id(&mut self, v: u32) {
        self.app_id = ::std::option::Option::Some(v);
    }

    // optional fixed64 steamid_lobby = 2;


    pub fn get_steamid_lobby(&self) -> u64 {
        self.steamid_lobby.unwrap_or(0)
    }
    pub fn clear_steamid_lobby(&mut self) {
        self.steamid_lobby = ::std::option::Option::None;
    }

    pub fn has_steamid_lobby(&self) -> bool {
        self.steamid_lobby.is_some()
    }

    // Param is passed by value, moved
    pub fn set_steamid_lobby(&mut self, v: u64) {
        self.steamid_lobby = ::std::option::Option::Some(v);
    }

    // optional .ELobbyStatus lobby_status = 3;


    pub fn get_lobby_status(&self) -> ELobbyStatus {
        self.lobby_status.unwrap_or(ELobbyStatus::k_ELobbyStatusInvalid)
    }
    pub fn clear_lobby_status(&mut self) {
        self.lobby_status = ::std::option::Option::None;
    }

    pub fn has_lobby_status(&self) -> bool {
        self.lobby_status.is_some()
    }

    // Param is passed by value, moved
    pub fn set_lobby_status(&mut self, v: ELobbyStatus) {
        self.lobby_status = ::std::option::Option::Some(v);
    }
}

impl ::protobuf::Message for LobbyMatchmakingLegacy_GetLobbyStatus_Response {
    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint32()?;
                    self.app_id = ::std::option::Option::Some(tmp);
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeFixed64 {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_fixed64()?;
                    self.steamid_lobby = ::std::option::Option::Some(tmp);
                },
                3 => {
                    ::protobuf::rt::read_proto2_enum_with_unknown_fields_into(wire_type, is, &mut self.lobby_status, 3, &mut self.unknown_fields)?
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        if let Some(v) = self.app_id {
            my_size += ::protobuf::rt::value_size(1, v, ::protobuf::wire_format::WireTypeVarint);
        }
        if let Some(v) = self.steamid_lobby {
            my_size += 9;
        }
        if let Some(v) = self.lobby_status {
            my_size += ::protobuf::rt::enum_size(3, v);
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        if let Some(v) = self.app_id {
            os.write_uint32(1, v)?;
        }
        if let Some(v) = self.steamid_lobby {
            os.write_fixed64(2, v)?;
        }
        if let Some(v) = self.lobby_status {
            os.write_enum(3, v.value())?;
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &dyn (::std::any::Any) {
        self as &dyn (::std::any::Any)
    }
    fn as_any_mut(&mut self) -> &mut dyn (::std::any::Any) {
        self as &mut dyn (::std::any::Any)
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        Self::descriptor_static()
    }

    fn new() -> LobbyMatchmakingLegacy_GetLobbyStatus_Response {
        LobbyMatchmakingLegacy_GetLobbyStatus_Response::new()
    }

    fn descriptor_static() -> &'static ::protobuf::reflect::MessageDescriptor {
        static mut descriptor: ::protobuf::lazy::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const ::protobuf::reflect::MessageDescriptor,
        };
        unsafe {
            descriptor.get(|| {
                let mut fields = ::std::vec::Vec::new();
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeUint32>(
                    "app_id",
                    |m: &LobbyMatchmakingLegacy_GetLobbyStatus_Response| { &m.app_id },
                    |m: &mut LobbyMatchmakingLegacy_GetLobbyStatus_Response| { &mut m.app_id },
                ));
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeFixed64>(
                    "steamid_lobby",
                    |m: &LobbyMatchmakingLegacy_GetLobbyStatus_Response| { &m.steamid_lobby },
                    |m: &mut LobbyMatchmakingLegacy_GetLobbyStatus_Response| { &mut m.steamid_lobby },
                ));
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeEnum<ELobbyStatus>>(
                    "lobby_status",
                    |m: &LobbyMatchmakingLegacy_GetLobbyStatus_Response| { &m.lobby_status },
                    |m: &mut LobbyMatchmakingLegacy_GetLobbyStatus_Response| { &mut m.lobby_status },
                ));
                ::protobuf::reflect::MessageDescriptor::new::<LobbyMatchmakingLegacy_GetLobbyStatus_Response>(
                    "LobbyMatchmakingLegacy_GetLobbyStatus_Response",
                    fields,
                    file_descriptor_proto()
                )
            })
        }
    }

    fn default_instance() -> &'static LobbyMatchmakingLegacy_GetLobbyStatus_Response {
        static mut instance: ::protobuf::lazy::Lazy<LobbyMatchmakingLegacy_GetLobbyStatus_Response> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const LobbyMatchmakingLegacy_GetLobbyStatus_Response,
        };
        unsafe {
            instance.get(LobbyMatchmakingLegacy_GetLobbyStatus_Response::new)
        }
    }
}

impl ::protobuf::Clear for LobbyMatchmakingLegacy_GetLobbyStatus_Response {
    fn clear(&mut self) {
        self.app_id = ::std::option::Option::None;
        self.steamid_lobby = ::std::option::Option::None;
        self.lobby_status = ::std::option::Option::None;
        self.unknown_fields.clear();
    }
}

impl ::std::fmt::Debug for LobbyMatchmakingLegacy_GetLobbyStatus_Response {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for LobbyMatchmakingLegacy_GetLobbyStatus_Response {
    fn as_ref(&self) -> ::protobuf::reflect::ProtobufValueRef {
        ::protobuf::reflect::ProtobufValueRef::Message(self)
    }
}

#[derive(Clone,PartialEq,Eq,Debug,Hash)]
#[cfg_attr(feature = "with-serde", derive(Serialize, Deserialize))]
pub enum ELobbyStatus {
    k_ELobbyStatusInvalid = 0,
    k_ELobbyStatusExists = 1,
    k_ELobbyStatusDoesNotExist = 2,
    k_ELobbyStatusNotAMember = 3,
}

impl ::protobuf::ProtobufEnum for ELobbyStatus {
    fn value(&self) -> i32 {
        *self as i32
    }

    fn from_i32(value: i32) -> ::std::option::Option<ELobbyStatus> {
        match value {
            0 => ::std::option::Option::Some(ELobbyStatus::k_ELobbyStatusInvalid),
            1 => ::std::option::Option::Some(ELobbyStatus::k_ELobbyStatusExists),
            2 => ::std::option::Option::Some(ELobbyStatus::k_ELobbyStatusDoesNotExist),
            3 => ::std::option::Option::Some(ELobbyStatus::k_ELobbyStatusNotAMember),
            _ => ::std::option::Option::None
        }
    }

    fn values() -> &'static [Self] {
        static values: &'static [ELobbyStatus] = &[
            ELobbyStatus::k_ELobbyStatusInvalid,
            ELobbyStatus::k_ELobbyStatusExists,
            ELobbyStatus::k_ELobbyStatusDoesNotExist,
            ELobbyStatus::k_ELobbyStatusNotAMember,
        ];
        values
    }

    fn enum_descriptor_static() -> &'static ::protobuf::reflect::EnumDescriptor {
        static mut descriptor: ::protobuf::lazy::Lazy<::protobuf::reflect::EnumDescriptor> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const ::protobuf::reflect::EnumDescriptor,
        };
        unsafe {
            descriptor.get(|| {
                ::protobuf::reflect::EnumDescriptor::new("ELobbyStatus", file_descriptor_proto())
            })
        }
    }
}

impl ::std::marker::Copy for ELobbyStatus {
}

impl ::std::default::Default for ELobbyStatus {
    fn default() -> Self {
        ELobbyStatus::k_ELobbyStatusInvalid
    }
}

impl ::protobuf::reflect::ProtobufValue for ELobbyStatus {
    fn as_ref(&self) -> ::protobuf::reflect::ProtobufValueRef {
        ::protobuf::reflect::ProtobufValueRef::Enum(self.descriptor())
    }
}

static file_descriptor_proto_data: &'static [u8] = b"\
    \n0steammessages_lobbymatchmaking.steamclient.proto\x1a,steammessages_un\
    ified_base.steamclient.proto\"\xe0\x01\n-LobbyMatchmakingLegacy_GetLobby\
    Status_Request\x12\x15\n\x06app_id\x18\x01\x20\x01(\rR\x05appId\x12#\n\r\
    steamid_lobby\x18\x02\x20\x01(\x06R\x0csteamidLobby\x12'\n\x0fclaim_owne\
    rship\x18\x03\x20\x01(\x08R\x0eclaimOwnership\x12)\n\x10claim_membership\
    \x18\x04\x20\x01(\x08R\x0fclaimMembership\x12\x1f\n\x0bversion_num\x18\
    \x05\x20\x01(\rR\nversionNum\"\xb5\x01\n.LobbyMatchmakingLegacy_GetLobby\
    Status_Response\x12\x15\n\x06app_id\x18\x01\x20\x01(\rR\x05appId\x12#\n\
    \rsteamid_lobby\x18\x02\x20\x01(\x06R\x0csteamidLobby\x12G\n\x0clobby_st\
    atus\x18\x03\x20\x01(\x0e2\r.ELobbyStatus:\x15k_ELobbyStatusInvalidR\x0b\
    lobbyStatus*\x81\x01\n\x0cELobbyStatus\x12\x19\n\x15k_ELobbyStatusInvali\
    d\x10\0\x12\x18\n\x14k_ELobbyStatusExists\x10\x01\x12\x1e\n\x1ak_ELobbyS\
    tatusDoesNotExist\x10\x02\x12\x1c\n\x18k_ELobbyStatusNotAMember\x10\x032\
    \xc6\x01\n\x16LobbyMatchmakingLegacy\x12\x85\x01\n\x0eGetLobbyStatus\x12\
    ..LobbyMatchmakingLegacy_GetLobbyStatus_Request\x1a/.LobbyMatchmakingLeg\
    acy_GetLobbyStatus_Response\"\x12\x82\xb5\x18\x0eGetLobbyStatus\x1a$\x82\
    \xb5\x18\x20Lobby\x20matchmaking\x20legacy\x20serviceB\x03\x80\x01\x01\
";

static mut file_descriptor_proto_lazy: ::protobuf::lazy::Lazy<::protobuf::descriptor::FileDescriptorProto> = ::protobuf::lazy::Lazy {
    lock: ::protobuf::lazy::ONCE_INIT,
    ptr: 0 as *const ::protobuf::descriptor::FileDescriptorProto,
};

fn parse_descriptor_proto() -> ::protobuf::descriptor::FileDescriptorProto {
    ::protobuf::parse_from_bytes(file_descriptor_proto_data).unwrap()
}

pub fn file_descriptor_proto() -> &'static ::protobuf::descriptor::FileDescriptorProto {
    unsafe {
        file_descriptor_proto_lazy.get(|| {
            parse_descriptor_proto()
        })
    }
}