// This file is generated by rust-protobuf 3.0.0-pre. Do not edit
// .proto file is parsed by protoc 3.10.1
// @generated

// https://github.com/rust-lang/rust-clippy/issues/702
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
#![allow(unused_imports)]
#![allow(unused_results)]

//! Generated file from `encrypted_app_ticket.proto`

/// Generated files are compatible only with the same version
/// of protobuf runtime.
const _PROTOBUF_VERSION_CHECK: () = ::protobuf::VERSION_3_0_0_PRE;

#[derive(PartialEq,Clone,Default)]
pub struct EncryptedAppTicket {
    // message fields
    ticket_version_no: ::std::option::Option<u32>,
    crc_encryptedticket: ::std::option::Option<u32>,
    cb_encrypteduserdata: ::std::option::Option<u32>,
    cb_encrypted_appownershipticket: ::std::option::Option<u32>,
    encrypted_ticket: ::protobuf::SingularField<::std::vec::Vec<u8>>,
    // special fields
    pub unknown_fields: ::protobuf::UnknownFields,
    pub cached_size: ::protobuf::rt::CachedSize,
}

impl<'a> ::std::default::Default for &'a EncryptedAppTicket {
    fn default() -> &'a EncryptedAppTicket {
        <EncryptedAppTicket as ::protobuf::Message>::default_instance()
    }
}

impl EncryptedAppTicket {
    pub fn new() -> EncryptedAppTicket {
        ::std::default::Default::default()
    }

    // optional uint32 ticket_version_no = 1;

    pub fn get_ticket_version_no(&self) -> u32 {
        self.ticket_version_no.unwrap_or(0)
    }

    pub fn clear_ticket_version_no(&mut self) {
        self.ticket_version_no = ::std::option::Option::None;
    }

    pub fn has_ticket_version_no(&self) -> bool {
        self.ticket_version_no.is_some()
    }

    // Param is passed by value, moved
    pub fn set_ticket_version_no(&mut self, v: u32) {
        self.ticket_version_no = ::std::option::Option::Some(v);
    }

    // optional uint32 crc_encryptedticket = 2;

    pub fn get_crc_encryptedticket(&self) -> u32 {
        self.crc_encryptedticket.unwrap_or(0)
    }

    pub fn clear_crc_encryptedticket(&mut self) {
        self.crc_encryptedticket = ::std::option::Option::None;
    }

    pub fn has_crc_encryptedticket(&self) -> bool {
        self.crc_encryptedticket.is_some()
    }

    // Param is passed by value, moved
    pub fn set_crc_encryptedticket(&mut self, v: u32) {
        self.crc_encryptedticket = ::std::option::Option::Some(v);
    }

    // optional uint32 cb_encrypteduserdata = 3;

    pub fn get_cb_encrypteduserdata(&self) -> u32 {
        self.cb_encrypteduserdata.unwrap_or(0)
    }

    pub fn clear_cb_encrypteduserdata(&mut self) {
        self.cb_encrypteduserdata = ::std::option::Option::None;
    }

    pub fn has_cb_encrypteduserdata(&self) -> bool {
        self.cb_encrypteduserdata.is_some()
    }

    // Param is passed by value, moved
    pub fn set_cb_encrypteduserdata(&mut self, v: u32) {
        self.cb_encrypteduserdata = ::std::option::Option::Some(v);
    }

    // optional uint32 cb_encrypted_appownershipticket = 4;

    pub fn get_cb_encrypted_appownershipticket(&self) -> u32 {
        self.cb_encrypted_appownershipticket.unwrap_or(0)
    }

    pub fn clear_cb_encrypted_appownershipticket(&mut self) {
        self.cb_encrypted_appownershipticket = ::std::option::Option::None;
    }

    pub fn has_cb_encrypted_appownershipticket(&self) -> bool {
        self.cb_encrypted_appownershipticket.is_some()
    }

    // Param is passed by value, moved
    pub fn set_cb_encrypted_appownershipticket(&mut self, v: u32) {
        self.cb_encrypted_appownershipticket = ::std::option::Option::Some(v);
    }

    // optional bytes encrypted_ticket = 5;

    pub fn get_encrypted_ticket(&self) -> &[u8] {
        match self.encrypted_ticket.as_ref() {
            Some(v) => v,
            None => &[],
        }
    }

    pub fn clear_encrypted_ticket(&mut self) {
        self.encrypted_ticket.clear();
    }

    pub fn has_encrypted_ticket(&self) -> bool {
        self.encrypted_ticket.is_some()
    }

    // Param is passed by value, moved
    pub fn set_encrypted_ticket(&mut self, v: ::std::vec::Vec<u8>) {
        self.encrypted_ticket = ::protobuf::SingularField::some(v);
    }

    // Mutable pointer to the field.
    // If field is not initialized, it is initialized with default value first.
    pub fn mut_encrypted_ticket(&mut self) -> &mut ::std::vec::Vec<u8> {
        if self.encrypted_ticket.is_none() {
            self.encrypted_ticket.set_default();
        }
        self.encrypted_ticket.as_mut().unwrap()
    }

    // Take field
    pub fn take_encrypted_ticket(&mut self) -> ::std::vec::Vec<u8> {
        self.encrypted_ticket.take().unwrap_or_else(|| ::std::vec::Vec::new())
    }
}

impl ::protobuf::Message for EncryptedAppTicket {
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
                    self.ticket_version_no = ::std::option::Option::Some(is.read_uint32()?);
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    self.crc_encryptedticket = ::std::option::Option::Some(is.read_uint32()?);
                },
                3 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    self.cb_encrypteduserdata = ::std::option::Option::Some(is.read_uint32()?);
                },
                4 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    self.cb_encrypted_appownershipticket = ::std::option::Option::Some(is.read_uint32()?);
                },
                5 => {
                    ::protobuf::rt::read_singular_bytes_into(wire_type, is, &mut self.encrypted_ticket)?;
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
        if let Some(v) = self.ticket_version_no {
            my_size += ::protobuf::rt::value_size(1, v, ::protobuf::wire_format::WireTypeVarint);
        }
        if let Some(v) = self.crc_encryptedticket {
            my_size += ::protobuf::rt::value_size(2, v, ::protobuf::wire_format::WireTypeVarint);
        }
        if let Some(v) = self.cb_encrypteduserdata {
            my_size += ::protobuf::rt::value_size(3, v, ::protobuf::wire_format::WireTypeVarint);
        }
        if let Some(v) = self.cb_encrypted_appownershipticket {
            my_size += ::protobuf::rt::value_size(4, v, ::protobuf::wire_format::WireTypeVarint);
        }
        if let Some(v) = self.encrypted_ticket.as_ref() {
            my_size += ::protobuf::rt::bytes_size(5, &v);
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        if let Some(v) = self.ticket_version_no {
            os.write_uint32(1, v)?;
        }
        if let Some(v) = self.crc_encryptedticket {
            os.write_uint32(2, v)?;
        }
        if let Some(v) = self.cb_encrypteduserdata {
            os.write_uint32(3, v)?;
        }
        if let Some(v) = self.cb_encrypted_appownershipticket {
            os.write_uint32(4, v)?;
        }
        if let Some(v) = self.encrypted_ticket.as_ref() {
            os.write_bytes(5, v)?;
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

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        Self::descriptor_static()
    }

    fn new() -> EncryptedAppTicket {
        EncryptedAppTicket::new()
    }

    fn descriptor_static() -> &'static ::protobuf::reflect::MessageDescriptor {
        static descriptor: ::protobuf::rt::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::rt::Lazy::INIT;
        descriptor.get(|| {
            let mut fields = ::std::vec::Vec::new();
            fields.push(::protobuf::reflect::rt::make_option_get_copy_accessor::<_, ::protobuf::reflect::types::ProtobufTypeUint32, _>(
                "ticket_version_no",
                |m: &EncryptedAppTicket| { &m.ticket_version_no },
                |m: &mut EncryptedAppTicket| { &mut m.ticket_version_no },
                EncryptedAppTicket::get_ticket_version_no,
            ));
            fields.push(::protobuf::reflect::rt::make_option_get_copy_accessor::<_, ::protobuf::reflect::types::ProtobufTypeUint32, _>(
                "crc_encryptedticket",
                |m: &EncryptedAppTicket| { &m.crc_encryptedticket },
                |m: &mut EncryptedAppTicket| { &mut m.crc_encryptedticket },
                EncryptedAppTicket::get_crc_encryptedticket,
            ));
            fields.push(::protobuf::reflect::rt::make_option_get_copy_accessor::<_, ::protobuf::reflect::types::ProtobufTypeUint32, _>(
                "cb_encrypteduserdata",
                |m: &EncryptedAppTicket| { &m.cb_encrypteduserdata },
                |m: &mut EncryptedAppTicket| { &mut m.cb_encrypteduserdata },
                EncryptedAppTicket::get_cb_encrypteduserdata,
            ));
            fields.push(::protobuf::reflect::rt::make_option_get_copy_accessor::<_, ::protobuf::reflect::types::ProtobufTypeUint32, _>(
                "cb_encrypted_appownershipticket",
                |m: &EncryptedAppTicket| { &m.cb_encrypted_appownershipticket },
                |m: &mut EncryptedAppTicket| { &mut m.cb_encrypted_appownershipticket },
                EncryptedAppTicket::get_cb_encrypted_appownershipticket,
            ));
            fields.push(::protobuf::reflect::rt::make_option_get_ref_accessor::<_, ::protobuf::reflect::types::ProtobufTypeBytes, _>(
                "encrypted_ticket",
                |m: &EncryptedAppTicket| { &m.encrypted_ticket },
                |m: &mut EncryptedAppTicket| { &mut m.encrypted_ticket },
                EncryptedAppTicket::get_encrypted_ticket,
            ));
            ::protobuf::reflect::MessageDescriptor::new::<EncryptedAppTicket>(
                "EncryptedAppTicket",
                fields,
                file_descriptor_proto()
            )
        })
    }

    fn default_instance() -> &'static EncryptedAppTicket {
        static instance: ::protobuf::rt::Lazy<EncryptedAppTicket> = ::protobuf::rt::Lazy::INIT;
        instance.get(EncryptedAppTicket::new)
    }
}

impl ::protobuf::Clear for EncryptedAppTicket {
    fn clear(&mut self) {
        self.ticket_version_no = ::std::option::Option::None;
        self.crc_encryptedticket = ::std::option::Option::None;
        self.cb_encrypteduserdata = ::std::option::Option::None;
        self.cb_encrypted_appownershipticket = ::std::option::Option::None;
        self.encrypted_ticket.clear();
        self.unknown_fields.clear();
    }
}

impl ::std::fmt::Debug for EncryptedAppTicket {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for EncryptedAppTicket {
}

static file_descriptor_proto_data: &'static [u8] = b"\
    \n\x1aencrypted_app_ticket.proto\"\x97\x02\n\x12EncryptedAppTicket\x12*\
    \n\x11ticket_version_no\x18\x01\x20\x01(\rR\x0fticketVersionNo\x12/\n\
    \x13crc_encryptedticket\x18\x02\x20\x01(\rR\x12crcEncryptedticket\x121\n\
    \x14cb_encrypteduserdata\x18\x03\x20\x01(\rR\x13cbEncrypteduserdata\x12F\
    \n\x1fcb_encrypted_appownershipticket\x18\x04\x20\x01(\rR\x1dcbEncrypted\
    Appownershipticket\x12)\n\x10encrypted_ticket\x18\x05\x20\x01(\x0cR\x0fe\
    ncryptedTicketB\x05H\x01\x80\x01\0\
";

static file_descriptor_proto_lazy: ::protobuf::rt::Lazy<::protobuf::descriptor::FileDescriptorProto> = ::protobuf::rt::Lazy::INIT;

fn parse_descriptor_proto() -> ::protobuf::descriptor::FileDescriptorProto {
    ::protobuf::parse_from_bytes(file_descriptor_proto_data).unwrap()
}

/// `FileDescriptorProto` object which was a source for this generated file
pub fn file_descriptor_proto() -> &'static ::protobuf::descriptor::FileDescriptorProto {
    file_descriptor_proto_lazy.get(|| {
        parse_descriptor_proto()
    })
}
