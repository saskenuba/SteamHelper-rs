#[cfg(feature = "async")]
use async_trait::async_trait;
#[cfg(feature = "async")]
use futures::future;

#[derive(serde::Serialize, Debug)]
/// We use this struct to wrap this endpoint parameters struct, plus our API Key
pub(crate) struct FormWrapper<'a, T: serde::Serialize> {
    #[serde(flatten)]
    pub(crate) parameters: T,
    pub(crate) key: &'a str,
}

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
                let request = api
                    .client
                    .request(reqwest::Method::$m, "http://api.steampowered.com")
                    .build()
                    .unwrap();

                Self {
                    client: &api.client,
                    key: &*api.key,
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
                use paste::paste;

                let query: String = self.recover_params();
                let api_key_parameter = format!("key={}", self.key);
                let mut req = self.request;
                let url = req.url_mut();
                url.set_query(Some(&(api_key_parameter + "&" + &query)));

                paste! {
                    self.client
                        .execute(req)
                        .map(|res| res.json::<$ret>().unwrap())
                }
            }
        }

        #[$crate::async_trait]
        #[cfg(feature = "async")]
        impl<'a> ExecutorResponse<$ret> for $base<'a> {
            async fn execute_with_response(self) -> crate::Result<$ret> {
                use futures::future::TryFutureExt;

                let query: String = self.recover_params();
                let api_key_parameter = format!("key={}", self.key);
                let mut req = self.request;
                let url = req.url_mut();
                url.set_query(Some(&(api_key_parameter + "&" + &query)));

                self.client
                    .execute(req)
                    .and_then(|res| res.json::<$ret>())
                    .await
                    .map_err(|e| e.into())
            }
        }

        // also implements for raw response
        impl_executor!($base);
    };
    ($base:ident) => {
        #[cfg(feature = "blocking")]
        impl<'a> Executor for $base<'a> {
            fn execute(self) -> crate::Result<String> {
                let query: String = self.recover_params();
                let api_key_parameter = format!("key={}", self.key);
                let mut req = self.request;
                let url = req.url_mut();
                url.set_query(Some(&(api_key_parameter + "&" + &query)));

                self.client.execute(req).text()
            }
        }

        #[$crate::async_trait]
        #[cfg(feature = "async")]
        impl<'a> Executor for $base<'a> {
            async fn execute(self) -> crate::Result<String> {
                use futures::future::TryFutureExt;

                match self.request.method() {
                    &reqwest::Method::GET => {
                        let query: String = self.recover_params();
                        let api_key_parameter = format!("key={}", self.key);
                        let mut req = self.request;
                        let url = req.url_mut();
                        url.set_query(Some(&(api_key_parameter + "&" + &query)));

                        let response = self.client.execute(req).await?;
                        let headers = response.headers();
                        let status_code = response.status();
                        headers_error_check(status_code, headers)?;
                        response.text().await.map_err(|e| e.into())
                    }

                    &reqwest::Method::POST => {
                        let data = self.recover_params_as_form();
                        let url = self.request.url().to_owned();

                        let data = crate::macros::FormWrapper {
                            parameters: data,
                            key: self.key,
                        };

                        let new_req = self.client.post(url).form(&data).build().unwrap();
                        let response = self.client.execute(new_req).await?;
                        let headers = response.headers();
                        let status_code = response.status();
                        headers_error_check(status_code, headers)?;
                        response.text().await.map_err(|e| e.into())
                    }
                    _ => unimplemented!(),
                }
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
                let new_url = api.request.url_mut();
                new_url.set_path(&[stringify!($b), "/"].join(""));

                let request = api.client.request(method, new_url.clone()).build().unwrap();
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
                let old_url = api.request.url();
                let new_url = old_url.join($path).unwrap();
                let request = api.client.request(method, new_url).build().unwrap();

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
