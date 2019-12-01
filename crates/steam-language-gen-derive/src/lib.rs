use syn::DeriveInput;
use syn::export::TokenStream;

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
    let gen = quote! {
        impl SerializableMessageBody for #name {

//            fn new() -> Self {
//                unimplemented!();
//            }

            fn serialize(&self) -> Vec<u8> {
                bincode::serialize(&self).unwrap()
            }

            fn deserialize_struct(packet_data: &[u8]) -> Self {
                let decoded: Self = bincode::deserialize(packet_data).unwrap();
                decoded
            }
        }
    };
    gen.into()
}
