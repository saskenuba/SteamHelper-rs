#[cfg(feature = "async")]
use async_trait::async_trait;

#[cfg(feature = "async")]
use futures::future;

#[cfg(feature = "async")]
macro_rules! import {
    () => {
        use crate::{
            helpers::{comma_delimited, indexed_array, querify},
            async_client::{Executor, ExecutorResponse, GetQueryBuilder, PostQueryBuilder},
        };
    };
}

#[cfg(feature = "blocking")]
macro_rules! import {
    () => {
        use crate::{
            blocking::{Executor, ExecutorResponse, GetQueryBuilder, PostQueryBuilder},
            helpers::{comma_delimited, indexed_array, querify},
        };
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

#[allow(unused)]
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

#[allow(unused)]
macro_rules! exec {
    ($base:ident -> $ret:ident) => {
        #[cfg(feature = "blocking")]
        impl<'a> ExecutorResponse<$ret> for $base<'a> {
            fn execute_with_response(self) -> reqwest::Result<$ret> {
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
            async fn execute_with_response(self) -> reqwest::Result<$ret> {
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
            }
        }

        // also implements for raw response
        exec!($base);
    };
    ($base:ident) => {
        #[cfg(feature = "blocking")]
        impl<'a> Executor for $base<'a> {
            fn execute(self) -> Result<String, reqwest::Error> {
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
            async fn execute(self) -> Result<String, reqwest::Error> {
                use futures::future::TryFutureExt;

                let query: String = self.recover_params();
                let api_key_parameter = format!("key={}", self.key);
                let mut req = self.request;
                let url = req.url_mut();
                url.set_query(Some(&(api_key_parameter + "&" + &query)));

                self.client.execute(req).and_then(|res| res.text()).await
            }
        }
    };
}

#[allow(unused)]
/// Creates the struct with appropriate parameters for the determined endpoint.
///
/// Also creates the conversion method for the Steam interface that implements the endpoint.
macro_rules! impl_conversions {
    (@$base:ident -> @$interface:ident) => {
        impl<'a> $base<'a> {
            #[allow(non_snake_case)]
            pub fn $interface(self) -> $interface<'a> {
                self.into()
            }
        }
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
