#[cfg(feature = "async")]
use async_trait::async_trait;
#[cfg(feature = "async")]
use futures::future;

use reqwest::header::{HeaderMap, HeaderValue};

#[derive(serde::Serialize)]
/// We use this struct to wrap this endpoint parameters struct, plus our credential.
///
/// Exactly one of `key` and `access_token` is set; with `key`, the form is the same as when `key` was the only field.
pub(crate) struct FormWrapper<'a, T: serde::Serialize> {
    #[serde(flatten)]
    pub(crate) parameters: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) key: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) access_token: Option<String>,
}

impl<'a, T: serde::Serialize> FormWrapper<'a, T> {
    pub(crate) fn new(parameters: T, credential: &CredentialParameter<'a>) -> Self {
        match credential {
            CredentialParameter::Key(key) => Self {
                parameters,
                key: Some(key),
                access_token: None,
            },
            CredentialParameter::AccessToken(encoded) => Self {
                parameters,
                key: None,
                // `.form()` encodes it again.
                access_token: url::form_urlencoded::parse(format!("t={}", encoded).as_bytes())
                    .next()
                    .map(|(_, token)| token.into_owned()),
            },
        }
    }
}

/// Carries an access token from `SteamAPI` to the endpoint structs, which have room only for a `key: &str` (generated
/// by `tappet-derive`). The endpoint structs derive `Debug`; a sensitive header value prints as `Sensitive`, so the
/// token never shows up there. Always removed before a request is sent (see [take_credential]).
pub(crate) const ACCESS_TOKEN_CARRIER: &str = "x-tappet-access-token";

/// The access token, form-urlencoded so that it is always a valid header value and can go straight into a query.
pub(crate) fn access_token_carrier(access_token: &str) -> HeaderValue {
    let encoded: String = url::form_urlencoded::byte_serialize(access_token.as_bytes()).collect();
    let mut value = HeaderValue::from_str(&encoded).expect("form-urlencoded output is visible ASCII");
    value.set_sensitive(true);
    value
}

/// The credential parameter a request is sent with.
pub(crate) enum CredentialParameter<'a> {
    Key(&'a str),
    /// Form-urlencoded.
    AccessToken(String),
}

impl CredentialParameter<'_> {
    pub(crate) fn query_pair(&self) -> String {
        match self {
            CredentialParameter::Key(key) => format!("key={}", key),
            CredentialParameter::AccessToken(encoded) => format!("access_token={}", encoded),
        }
    }
}

/// Removes the access token carrier from `headers` and picks the credential to send.
///
/// The endpoint's `key` is empty when `SteamAPI` holds an access token, so a key set with `inject_custom_key` wins over
/// the token, and a `SteamAPI` built with an API key sends `key=` exactly as before.
pub(crate) fn take_credential<'a>(key: &'a str, headers: &mut HeaderMap) -> CredentialParameter<'a> {
    match headers.remove(ACCESS_TOKEN_CARRIER) {
        Some(encoded) if key.is_empty() => {
            CredentialParameter::AccessToken(String::from_utf8_lossy(encoded.as_bytes()).into_owned())
        }
        _ => CredentialParameter::Key(key),
    }
}

#[cfg(feature = "async")]
pub(crate) type HttpRequest = reqwest::Request;
#[cfg(feature = "blocking")]
pub(crate) type HttpRequest = reqwest::blocking::Request;

#[cfg(feature = "async")]
macro_rules! import {
    () => {
        use crate::{
            async_client::{Executor, ExecutorResponse, GetQueryBuilder, PostQueryBuilder},
            errors::headers_error_check,
            helpers::{comma_delimited, indexed_array, querify},
        };
        use serde::{Deserialize, Serialize};
    };
}

#[cfg(feature = "blocking")]
macro_rules! import {
    () => {
        use crate::{
            blocking::{Executor, ExecutorResponse, GetQueryBuilder, PostQueryBuilder},
            errors::headers_error_check,
            helpers::{comma_delimited, indexed_array, querify},
        };
        use serde::{Deserialize, Serialize};
    };
}

#[allow(unused)]
macro_rules! func_client {
    ($i: ident, $t: ty) => {
        pub fn $i(self) -> $t {
            self.into()
        }
    };
}

#[allow(unused)]
macro_rules! from {
    ( @$f: ident => $m: ident ) => {
        impl<'a> From<&'a SteamAPI> for $f<'a> {
            fn from(api: &'a SteamAPI) -> Self {
                // https: the query carries the API key or the access token. node-steam-tradeoffer-manager sends the
                // same parameters over https (`lib/webapi.js`).
                let mut request = api
                    .client
                    .request(reqwest::Method::$m, "https://api.steampowered.com")
                    .build()
                    .unwrap();

                let key = match &api.credential {
                    crate::Credential::ApiKey(key) => key.as_str(),
                    crate::Credential::AccessToken(access_token) => {
                        request.headers_mut().insert(
                            crate::macros::ACCESS_TOKEN_CARRIER,
                            crate::macros::access_token_carrier(access_token),
                        );
                        ""
                    }
                };

                Self {
                    client: &api.client,
                    key,
                    request,
                }
            }
        }
    };
}

macro_rules! new_type {
    ( $f:ident) => {
        #[cfg(feature = "blocking")]
        pub struct $f<'a> {
            pub(crate) request: reqwest::blocking::Request,
            pub(crate) client: &'a reqwest::blocking::Client,
            pub(crate) key: &'a str,
        }

        #[cfg(feature = "async")]
        pub struct $f<'a> {
            pub(crate) request: reqwest::Request,
            pub(crate) client: &'a reqwest::Client,
            pub(crate) key: &'a str,
        }
    };
}

macro_rules! impl_executor {
    ($base:ident -> $ret:ident) => {
        #[cfg(feature = "blocking")]
        impl<'a> ExecutorResponse<$ret> for $base<'a> {
            fn execute_with_response(self) -> crate::Result<$ret> {
                let body = Executor::execute(self)?;
                crate::errors::decode_response(body)
            }
        }

        #[$crate::async_trait]
        #[cfg(feature = "async")]
        impl<'a> ExecutorResponse<$ret> for $base<'a> {
            async fn execute_with_response(self) -> crate::Result<$ret> {
                let body = Executor::execute(self).await?;
                crate::errors::decode_response(body)
            }
        }

        // also implements for raw response
        impl_executor!($base);
    };
    ($base:ident) => {
        impl<'a> $base<'a> {
            /// Builds the request that `execute` sends, without sending it. Its URL or form holds the credential.
            ///
            /// For tests that check what goes on the wire, here and in dependent crates; not a stable API.
            #[doc(hidden)]
            pub fn prepare(self) -> crate::Result<crate::macros::HttpRequest> {
                match self.request.method() {
                    &reqwest::Method::GET => {
                        let query: String = self.recover_params();
                        let mut req = self.request;
                        let credential = crate::macros::take_credential(self.key, req.headers_mut());
                        let url = req.url_mut();
                        url.set_query(Some(&(credential.query_pair() + "&" + &query)));
                        Ok(req)
                    }

                    &reqwest::Method::POST => {
                        let mut headers = self.request.headers().clone();
                        let credential = crate::macros::take_credential(self.key, &mut headers);
                        let url = self.request.url().to_owned();

                        let data = crate::macros::FormWrapper::new(self.recover_params_as_form(), &credential);
                        self.client.post(url).form(&data).build().map_err(|e| e.into())
                    }
                    _ => unimplemented!(),
                }
            }
        }

        #[cfg(feature = "blocking")]
        impl<'a> Executor for $base<'a> {
            fn execute(self) -> crate::Result<String> {
                let client = self.client;
                let response = client.execute(self.prepare()?)?;
                let status_code = response.status();
                let headers = response.headers().clone();
                let body = response.text()?;
                headers_error_check(status_code, &headers, &body)?;
                Ok(body)
            }
        }

        #[$crate::async_trait]
        #[cfg(feature = "async")]
        impl<'a> Executor for $base<'a> {
            async fn execute(self) -> crate::Result<String> {
                let client = self.client;
                let response = client.execute(self.prepare()?).await?;
                let status_code = response.status();
                let headers = response.headers().clone();
                let body = response.text().await?;
                headers_error_check(status_code, &headers, &body)?;
                Ok(body)
            }
        }
    };
}

#[allow(unused)]
/// Creates the struct with appropriate parameters for the determined endpoint.
///
/// Also creates the conversion method for the Steam interface that implements the endpoint.
macro_rules! impl_conversions {

    (@$base:ident -> @$interface:ident, $string_ident: expr) => {
    impl<'a> $base<'a> {
            #[allow(non_snake_case)]
            pub fn $interface(self) -> $interface<'a> {
                self.into()
            }
        }

    };

    (@$base:ident -> @$interface:ident) => {
        impl_conversions!(@$base -> @$interface, stringify!($interface));
    };
}

#[allow(unused)]
/// Creates appropriate implementation of `std::convert::From` for:
///
/// Base method -> Interface
///
/// Interface -> Endpoint
macro_rules! convert_with_endpoint {
    ( @$f:ident -> @$b:ident ) => {
        impl<'a> From<$f<'a>> for $b<'a> {
            fn from(mut api: $f<'a>) -> Self {
                let method = api.request.method().to_owned();
                // Keeps the access token carrier, if any.
                let headers = api.request.headers().clone();
                let new_url = api.request.url_mut();
                new_url.set_path(&[stringify!($b), "/"].join(""));

                let request = api
                    .client
                    .request(method, new_url.clone())
                    .headers(headers)
                    .build()
                    .unwrap();
                Self {
                    request,
                    key: api.key,
                    client: api.client,
                }
            }
        }
    };
    ( @$f:ident -> $b:ident |> $path:literal ) => {
        impl<'a> From<$f<'a>> for $b<'a> {
            fn from(api: $f<'a>) -> Self {
                let method = api.request.method().to_owned();
                let headers = api.request.headers().clone();
                let old_url = api.request.url();
                let new_url = old_url.join($path).unwrap();
                let request = api
                    .client
                    .request(method, new_url)
                    .headers(headers)
                    .build()
                    .unwrap();

                Self {
                    client: api.client,
                    key: api.key,
                    parameters: Default::default(),
                    request,
                }
            }
        }
    };
}
