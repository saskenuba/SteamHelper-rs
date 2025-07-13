use proc_macro::TokenStream;

use syn::{AttributeArgs, DeriveInput, parse_macro_input};

use quote::quote;

#[proc_macro_derive(SteamMsg)]
pub fn steammsg_derive(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_steammsg_macro(&ast)
}

/// We also need to accept attributes in an specific order, so we can
/// implement the "new" function, that set each attribute in order of members
fn impl_steammsg_macro(ast: &DeriveInput) -> TokenStream {
    let name = &ast.ident;

    let generated = quote! {
        impl SerializableBytes for #name {
            fn to_bytes(&self) -> Vec<u8> {
                bincode::serialize(&self).unwrap()
            }
        }
        impl DeserializableBytes for #name {
            fn from_bytes(packet_data: &[u8]) -> Self {
                bincode::deserialize(packet_data).unwrap()
            }
        }
        impl MessageBodyExt for #name {
            fn split_from_bytes(data: &[u8]) -> (&[u8], &[u8]) {
                let size = std::mem::size_of::<Self>();
                (&data[..size], &data[size..])
            }
        }
    };
    generated.into()
}


#[proc_macro_derive(MsgHeader)]
pub fn header_derive(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_header_macro(&ast)
}


fn impl_header_macro(ast: &DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let generated = quote! {

        impl SerializableBytes for #name {
            fn to_bytes(&self) -> Vec<u8> {
                bincode::serialize(&self).unwrap()
            }
        }
        impl DeserializableBytes for #name {
            fn from_bytes(packet_data: &[u8]) -> Self {
                let decoded: Self = bincode::deserialize(packet_data).unwrap();
                decoded
            }
        }

        impl MessageHeaderExt for #name {
            // we are taking out 4 bytes of the emsg
            fn split_from_bytes(data: &[u8]) -> (&[u8], &[u8]) {
                let size = std::mem::size_of::<Self>();
                (&data[..size], &data[size..])
            }
            fn create() -> Self {
                Self::new()
            }
        }

        impl HasJobId for #name {
            fn set_target(&mut self, new_target: u64) {
                self.target_job_id = new_target;
            }
            fn set_source(&mut self, new_source: u64) {
                self.source_job_id = new_source;
            }
            fn target(&self) -> u64 {
                self.target_job_id
            }
            fn source(&self) -> u64 {
                self.source_job_id
            }

        }
    };
    generated.into()
}

#[proc_macro_attribute]
pub fn linked_emsg(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(item).unwrap();
    let name = &ast.ident;

    let mut args: AttributeArgs = parse_macro_input!(attr as AttributeArgs);
    let attribute = args.pop().unwrap();

    let tokens = quote! {
        #ast

        impl HasEMsg for #name {
            fn emsg() -> EMsg {
                #attribute
            }
            fn create() -> Self {
                Self::new()
            }
        }

    };
    tokens.into()
}