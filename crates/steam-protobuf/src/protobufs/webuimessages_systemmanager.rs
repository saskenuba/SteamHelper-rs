// This file is generated by rust-protobuf 3.3.0. Do not edit
// .proto file is parsed by protoc 3.19.4
// @generated

// https://github.com/rust-lang/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy::all)]

#![allow(unused_attributes)]
#![cfg_attr(rustfmt, rustfmt::skip)]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unused_results)]
#![allow(unused_mut)]

//! Generated file from `webuimessages_systemmanager.proto`

// @@protoc_insertion_point(message:CSystemManager_Hibernate_Request)
#[derive(PartialEq,Clone,Default,Debug)]
pub struct CSystemManager_Hibernate_Request {
    // special fields
    // @@protoc_insertion_point(special_field:CSystemManager_Hibernate_Request.special_fields)
    pub special_fields: crate::SpecialFields,
}

impl<'a> ::std::default::Default for &'a CSystemManager_Hibernate_Request {
    fn default() -> &'a CSystemManager_Hibernate_Request {
        <CSystemManager_Hibernate_Request as crate::Message>::default_instance()
    }
}

impl CSystemManager_Hibernate_Request {
    pub fn new() -> CSystemManager_Hibernate_Request {
        ::std::default::Default::default()
    }

    fn generated_message_descriptor_data() -> crate::reflect::GeneratedMessageDescriptorData {
        let mut fields = ::std::vec::Vec::with_capacity(0);
        let mut oneofs = ::std::vec::Vec::with_capacity(0);
        crate::reflect::GeneratedMessageDescriptorData::new_2::<CSystemManager_Hibernate_Request>(
            "CSystemManager_Hibernate_Request",
            fields,
            oneofs,
        )
    }
}

impl crate::Message for CSystemManager_Hibernate_Request {
    const NAME: &'static str = "CSystemManager_Hibernate_Request";

    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut crate::CodedInputStream<'_>) -> crate::Result<()> {
        while let Some(tag) = is.read_raw_tag_or_eof()? {
            match tag {
                tag => {
                    crate::rt::read_unknown_or_skip_group(tag, is, self.special_fields.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u64 {
        let mut my_size = 0;
        my_size += crate::rt::unknown_fields_size(self.special_fields.unknown_fields());
        self.special_fields.cached_size().set(my_size as u32);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut crate::CodedOutputStream<'_>) -> crate::Result<()> {
        os.write_unknown_fields(self.special_fields.unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn special_fields(&self) -> &crate::SpecialFields {
        &self.special_fields
    }

    fn mut_special_fields(&mut self) -> &mut crate::SpecialFields {
        &mut self.special_fields
    }

    fn new() -> CSystemManager_Hibernate_Request {
        CSystemManager_Hibernate_Request::new()
    }

    fn clear(&mut self) {
        self.special_fields.clear();
    }

    fn default_instance() -> &'static CSystemManager_Hibernate_Request {
        static instance: CSystemManager_Hibernate_Request = CSystemManager_Hibernate_Request {
            special_fields: crate::SpecialFields::new(),
        };
        &instance
    }
}

impl crate::MessageFull for CSystemManager_Hibernate_Request {
    fn descriptor() -> crate::reflect::MessageDescriptor {
        static descriptor: crate::rt::Lazy<crate::reflect::MessageDescriptor> = crate::rt::Lazy::new();
        descriptor.get(|| file_descriptor().message_by_package_relative_name("CSystemManager_Hibernate_Request").unwrap()).clone()
    }
}

impl ::std::fmt::Display for CSystemManager_Hibernate_Request {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        crate::text_format::fmt(self, f)
    }
}

impl crate::reflect::ProtobufValue for CSystemManager_Hibernate_Request {
    type RuntimeType = crate::reflect::rt::RuntimeTypeMessage<Self>;
}

// @@protoc_insertion_point(message:CSystemManager_Hibernate_Response)
#[derive(PartialEq,Clone,Default,Debug)]
pub struct CSystemManager_Hibernate_Response {
    // special fields
    // @@protoc_insertion_point(special_field:CSystemManager_Hibernate_Response.special_fields)
    pub special_fields: crate::SpecialFields,
}

impl<'a> ::std::default::Default for &'a CSystemManager_Hibernate_Response {
    fn default() -> &'a CSystemManager_Hibernate_Response {
        <CSystemManager_Hibernate_Response as crate::Message>::default_instance()
    }
}

impl CSystemManager_Hibernate_Response {
    pub fn new() -> CSystemManager_Hibernate_Response {
        ::std::default::Default::default()
    }

    fn generated_message_descriptor_data() -> crate::reflect::GeneratedMessageDescriptorData {
        let mut fields = ::std::vec::Vec::with_capacity(0);
        let mut oneofs = ::std::vec::Vec::with_capacity(0);
        crate::reflect::GeneratedMessageDescriptorData::new_2::<CSystemManager_Hibernate_Response>(
            "CSystemManager_Hibernate_Response",
            fields,
            oneofs,
        )
    }
}

impl crate::Message for CSystemManager_Hibernate_Response {
    const NAME: &'static str = "CSystemManager_Hibernate_Response";

    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut crate::CodedInputStream<'_>) -> crate::Result<()> {
        while let Some(tag) = is.read_raw_tag_or_eof()? {
            match tag {
                tag => {
                    crate::rt::read_unknown_or_skip_group(tag, is, self.special_fields.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u64 {
        let mut my_size = 0;
        my_size += crate::rt::unknown_fields_size(self.special_fields.unknown_fields());
        self.special_fields.cached_size().set(my_size as u32);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut crate::CodedOutputStream<'_>) -> crate::Result<()> {
        os.write_unknown_fields(self.special_fields.unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn special_fields(&self) -> &crate::SpecialFields {
        &self.special_fields
    }

    fn mut_special_fields(&mut self) -> &mut crate::SpecialFields {
        &mut self.special_fields
    }

    fn new() -> CSystemManager_Hibernate_Response {
        CSystemManager_Hibernate_Response::new()
    }

    fn clear(&mut self) {
        self.special_fields.clear();
    }

    fn default_instance() -> &'static CSystemManager_Hibernate_Response {
        static instance: CSystemManager_Hibernate_Response = CSystemManager_Hibernate_Response {
            special_fields: crate::SpecialFields::new(),
        };
        &instance
    }
}

impl crate::MessageFull for CSystemManager_Hibernate_Response {
    fn descriptor() -> crate::reflect::MessageDescriptor {
        static descriptor: crate::rt::Lazy<crate::reflect::MessageDescriptor> = crate::rt::Lazy::new();
        descriptor.get(|| file_descriptor().message_by_package_relative_name("CSystemManager_Hibernate_Response").unwrap()).clone()
    }
}

impl ::std::fmt::Display for CSystemManager_Hibernate_Response {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        crate::text_format::fmt(self, f)
    }
}

impl crate::reflect::ProtobufValue for CSystemManager_Hibernate_Response {
    type RuntimeType = crate::reflect::rt::RuntimeTypeMessage<Self>;
}

static file_descriptor_proto_data: &'static [u8] = b"\
    \n!webuimessages_systemmanager.proto\x1a\x18steammessages_base.proto\x1a\
    \x18webuimessages_base.proto\"\"\n\x20CSystemManager_Hibernate_Request\"\
    #\n!CSystemManager_Hibernate_Response2i\n\rSystemManager\x12R\n\tHiberna\
    te\x12!.CSystemManager_Hibernate_Request\x1a\".CSystemManager_Hibernate_\
    Response\x1a\x04\x80\x97\"\x02B\x05H\x01\x80\x01\x01\
";

/// `FileDescriptorProto` object which was a source for this generated file
fn file_descriptor_proto() -> &'static crate::descriptor::FileDescriptorProto {
    static file_descriptor_proto_lazy: crate::rt::Lazy<crate::descriptor::FileDescriptorProto> = crate::rt::Lazy::new();
    file_descriptor_proto_lazy.get(|| {
        crate::Message::parse_from_bytes(file_descriptor_proto_data).unwrap()
    })
}

/// `FileDescriptor` object which allows dynamic access to files
pub fn file_descriptor() -> &'static crate::reflect::FileDescriptor {
    static generated_file_descriptor_lazy: crate::rt::Lazy<crate::reflect::GeneratedFileDescriptor> = crate::rt::Lazy::new();
    static file_descriptor: crate::rt::Lazy<crate::reflect::FileDescriptor> = crate::rt::Lazy::new();
    file_descriptor.get(|| {
        let generated_file_descriptor = generated_file_descriptor_lazy.get(|| {
            let mut deps = ::std::vec::Vec::with_capacity(2);
            deps.push(super::steammessages_base::file_descriptor().clone());
            deps.push(super::webuimessages_base::file_descriptor().clone());
            let mut messages = ::std::vec::Vec::with_capacity(2);
            messages.push(CSystemManager_Hibernate_Request::generated_message_descriptor_data());
            messages.push(CSystemManager_Hibernate_Response::generated_message_descriptor_data());
            let mut enums = ::std::vec::Vec::with_capacity(0);
            crate::reflect::GeneratedFileDescriptor::new_generated(
                file_descriptor_proto(),
                deps,
                messages,
                enums,
            )
        });
        crate::reflect::FileDescriptor::new_generated_2(generated_file_descriptor)
    })
}