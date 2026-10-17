use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::AttributeArgs;
use syn::DeriveInput;

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

    let gen = quote! {
        impl SerializableBytes for #name {
            fn to_bytes(&self) -> Vec<u8> {
                bincode::serialize(&self).unwrap()
            }
        }
        impl MessageBodyExt for #name {
            // bincode reads exactly the bytes it needs, and leaves the rest in the slice
            fn parse(data: &[u8]) -> Result<(Self, &[u8]), MessageError> {
                let mut rest = data;
                let parsed = bincode::deserialize_from(&mut rest)?;
                Ok((parsed, rest))
            }
        }
    };
    gen.into()
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
    let gen = quote! {

        impl SerializableBytes for #name {
            fn to_bytes(&self) -> Vec<u8> {
                bincode::serialize(&self).unwrap()
            }
        }

        impl MessageHeaderExt for #name {
            // bincode reads exactly the bytes it needs, and leaves the rest in the slice
            fn parse(data: &[u8]) -> Result<(Self, &[u8]), MessageError> {
                let mut rest = data;
                let parsed = bincode::deserialize_from(&mut rest)?;
                Ok((parsed, rest))
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
    gen.into()
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
