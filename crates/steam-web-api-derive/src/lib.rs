use proc_macro::TokenStream;

use proc_macro2::Span;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields, Ident, Meta};

const META_COMMA: &str = "comma";
const META_INDEXED: &str = "indexed";

/// To retrieve the original name of the endpoint, we subtract the length of the word "Parameters".
const PARAMETERS_LITERAL_LENGTH: usize = 10;

type SimpleFields = Punctuated<syn::Field, syn::token::Comma>;

#[derive(Debug, PartialEq)]
enum VecMethod {
    Comma,
    Indexed,
    None,
}

#[proc_macro_attribute]
pub fn interface(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let attribute = _attr.to_string();

    let name = &input.ident;
    let my_ident = name.to_string();
    let new_ident = Ident::new(
        &my_ident[..my_ident.len() - PARAMETERS_LITERAL_LENGTH],
        Span::call_site(),
    );
    let new_ident_stringified = new_ident.to_string();

    let steam_interface_name = Ident::new(&attribute, Span::call_site());
    let struct_fields = get_fields(&input).unwrap();
    let field_idents: Vec<_> = struct_fields.iter().filter_map(|field| field.ident.as_ref()).collect();
    let field_types: Vec<_> = struct_fields.iter().map(|field| &field.ty).collect();

    let impl_convert = quote! {
          #input

        impl<'a> #steam_interface_name<'a> {
            #[doc = "A new request to the `"]
            #[doc = #new_ident_stringified]
            #[doc = "` endpoint.\n\n"]
            ///
            /// The returning struct implements:
            /// - `execute`, for a raw `String` response; Requires import of trait `Executor`;
            /// - `execute_with_response`, for a jsonified response into a struct. **Needs to be available for this endpoint**. Requires import of trait `ExecutorResponse`;
            /// - `inject_custom_key`, for injecting a custom api key different than the one used for instantiating `SteamAPI`, which
            /// then you can execute with the options above.
             pub fn #new_ident(self, #(#field_idents: #field_types),* ) -> #new_ident<'a> {
                 let mut into: #new_ident = self.into();
                 #(into.parameters.#field_idents = #field_idents;)*
                 into
             }
        }
    };

    impl_convert.into()
}

#[proc_macro_derive(Parameters, attributes(comma, indexed))]
pub fn derive_parameters(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    let struct_parameters_name = &input.ident;
    let ident_str = struct_parameters_name.to_string();
    let struct_fields = get_fields(&input).unwrap();
    let processed_fields = process_fields(struct_fields);

    let new_ident = Ident::new(
        &ident_str[..ident_str.len() - PARAMETERS_LITERAL_LENGTH],
        Span::call_site(),
    );

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {

        #[derive(Debug)]
        #[cfg(feature = "async")]
        pub struct #new_ident<'a> {
            pub(crate) key: &'a str,
            pub(crate) request: reqwest::Request,
            pub(crate) client: &'a reqwest::Client,
            pub(crate) parameters: #struct_parameters_name,
        }

        #[derive(Debug)]
        #[cfg(feature = "blocking")]
        pub struct #new_ident<'a> {
            pub(crate) key: &'a str,
            pub(crate) request: reqwest::blocking::Request,
            pub(crate) client: &'a reqwest::blocking::Client,
            pub(crate) parameters: #struct_parameters_name,
        }

        impl<'a> #new_ident<'a> {
            pub(crate) fn recover_params(&self) -> String {
                self.parameters.recover_params()
            }

            pub(crate) fn recover_params_as_form(&self) -> &#struct_parameters_name {
                &self.parameters
            }

            pub fn inject_custom_key(self, apikey: &'a str) -> Self {
                let mut endpoint = self;
                endpoint.key = apikey;
                endpoint
            }
        }

        impl #struct_parameters_name {
                pub(crate) fn recover_params(&self) -> String {
                    let mut query = String::new();
                    #(#processed_fields)*;
                    query
                }
        }
    };

    // Hand the output tokens back to the compiler
    TokenStream::from(expanded)
}

fn process_fields(fields: &SimpleFields) -> Vec<proc_macro2::TokenStream> {
    let mut tokens = Vec::new();

    for field in fields {
        let field_ident = &field.ident.as_ref().unwrap();
        let field_ident_as_str = &field.ident.as_ref().unwrap().to_string();

        let field_has_meta = is_comma_or_indexed(&field);
        let is_option = is_option(&field.ty);

        let vec_fn = match field_has_meta {
            VecMethod::Comma => quote! {
                &*querify(#field_ident_as_str, &comma_delimited(&#field_ident))
            },
            VecMethod::Indexed => quote! {
               &indexed_array(#field_ident_as_str, &#field_ident)
            },
            VecMethod::None => quote! { &*querify(#field_ident_as_str, &#field_ident)},
        };

        let output = if is_option {
            quote! {
                let #field_ident = &self.#field_ident;
                if let Some(#field_ident) = #field_ident {
                    query.push_str(#vec_fn);
                }
            }
        } else {
            quote! {
                let #field_ident = &self.#field_ident;
                query.push_str(#vec_fn);
            }
        };
        tokens.push(output);
    }
    tokens
}

fn is_comma_or_indexed(field: &&Field) -> VecMethod {
    let is_vec_marked = &field
        .attrs
        .iter()
        .filter_map(|attribute| attribute.parse_meta().ok())
        .map(|meta| match meta {
            Meta::Path(path) => path,
            _ => unimplemented!(),
        })
        .map(|path| path.get_ident().unwrap().to_string())
        .collect::<String>();

    match is_vec_marked {
        f if f == META_INDEXED => VecMethod::Indexed,
        f if f == META_COMMA => VecMethod::Comma,
        _ => VecMethod::None,
    }
}

fn get_fields(derive_input: &syn::DeriveInput) -> Option<&SimpleFields> {
    if let Data::Struct(data_struct) = &derive_input.data {
        if let Fields::Named(fields) = &data_struct.fields {
            Some(&fields.named)
        } else {
            None
        }
    } else {
        None
    }
}

fn is_option(kind: &syn::Type) -> bool {
    match kind {
        syn::Type::Path(t) => match t.path.segments.first() {
            Some(t) => t.ident == "Option",
            _ => false,
        },
        _ => false,
    }
}
