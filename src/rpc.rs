#![allow(async_fn_in_trait)]
// TODO: channel setup flow. Server/Client-tagged channels?

use std::fmt::Debug;

use snafu::Snafu;

use crate::{
    Channel,
    error::{ErrorProvider, Typed},
    trait_alias,
};

trait_alias!(trait Data: serde::Serialize + serde::de::DeserializeOwned);

#[allow(private_bounds)]
pub trait Service {
    type Request: Data;
    type Response: Data;

    type Client<C: Caller<Self>>: From<C>;
}

pub trait Caller<S: Service>: ErrorProvider {
    async fn call(&mut self, request: S::Request) -> Result<S::Response, Self::Error>;
}

impl<T, S> Caller<S> for T
where
    T: Channel,
    S: Service<Request = T::In, Response = T::Out>,
{
    async fn call(&mut self, request: S::Request) -> Result<S::Response, Self::Error> {
        self.send(request).await?;
        self.recv().await
    }
}

pub trait Handler<S: Service + ?Sized> {
    async fn handle(&self, request: S::Request) -> S::Response;
}

#[macro_export]
macro_rules! define_rpc {
    (
        $(#[$feat:tt])*
        $vis:vis $service:ident {
            $(fn $function:ident ( $($field:ident: $field_type:ty),* ) -> $response:ty;)*
        }
    ) => {

    #[allow(non_snake_case, non_camel_case_types, async_fn_in_trait)]
    $vis mod $service {
        use super::*;
        use $crate::rpc::{self,*};
        use snafu::ResultExt;

        pub trait Handler
        {
            $(
                async fn $function(&self, $($field: $field_type),*) -> RpcResult<$response>;
            )*
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum Request {
            $($function { $($field: $field_type),* }),*
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum Response {
            _Error { message: String },
            $($function($response)),*
        }

        pub struct Service;
        impl rpc::Service for Service {
            type Request = Request;
            type Response = Response;

            type Client<C: Caller<Service>> = Client<C>;
        }

        pub struct Route<T>(pub T);
        impl<T> From<T> for Route<T> {
            fn from(value: T) -> Self {
                Self(value)
            }
        }

        impl<T: Handler> rpc::Handler<Service> for Route<T> {
            async fn handle(&self, request: Request) -> Response {
                match request {
                    $(Request::$function { $($field),* } => {
                        let ret = self.0.$function($($field),*).await;

                        let response = match ret {
                            Ok(value) => Response::$function(value),
                            Err(e) => Response::_Error { message: e.message },
                        };

                        response
                    }),*
                }
            }
        }

        pub struct Client<C>(C);
        impl<C> From<C> for Client<C>
        where C: Caller<Service>
        {
            fn from(value: C) -> Self {
                Self(value)
            }
        }

        impl<C> Client<C>
        where C: Caller<Service>
        {
            $(pub async fn $function(
                &mut self, $($field: $field_type),*
            ) -> Result<$response, ClientError> {
                let request = Request::$function { $($field),* };

                match self.0.call(request).await
                    .map_err(|e| ClientError::CallerError { source: e.into() })?
                {
                    Response::$function(ret) => Ok(ret),

                    Response::_Error { message } => Err(ClientError::FunctionError { message }),

                    other => Err(ClientError::ProtocolError {
                        expected: stringify!($function),
                        got: std::any::type_name_of_val(&other)
                    })
                }
            })*
        }

        $crate::define_rpc!(@feature_select from $(#[$feat])* :
        split_handler => {
            pub mod split_handler {
                use super::*;
                $(pub trait $function {
                    async fn handle(&self, $($field: $field_type),*) -> super::RpcResult<$response>;
                })*

                impl<T> super::Handler for T where T: $($function +)* {
                    $(fn $function(&self, $($field: $field_type),*) -> impl Future<Output = super::RpcResult<$response>> {
                        <Self as $function>::handle(self, $($field),*)
                    })*
                }
            }
        }
        );


    }};

    // TODO: While this is fun, seriously consider switching to proc-macros
    (@feature_select from $(#[$features:tt])* : $($feature:tt => { $($body:tt)* })+) => {
        $crate::define_rpc!(@scope ($s:tt) => {
            macro_rules! selector {
                $(
                    ($feature, $s(other:tt)*) => { $($body)* }
                )+;
                ($not_feature:tt, $s($other:tt)*) => { selector!($s($other)*); };
                () => {}
            }
        });

        selector!($($features,)*);
    };

    // See https://github.com/rust-lang/rust/issues/35853#issuecomment-415993963
    (@scope $($body:tt)*) => {
        macro_rules! __with_dollar_sign { $($body)* }
        __with_dollar_sign!($);
    }
}

pub struct RpcErrorHatch {
    pub message: String,
}

impl<T: Debug> From<T> for RpcErrorHatch {
    fn from(value: T) -> Self {
        Self {
            message: format!("{value:?}"),
        }
    }
}

pub type RpcResult<T> = std::result::Result<T, RpcErrorHatch>;

#[derive(Debug, Snafu)]
pub enum ClientError {
    #[snafu(display("error in caller"))]
    CallerError { source: Box<dyn Typed> },

    #[snafu(display("{message}"))]
    FunctionError { message: String },

    #[snafu(display("got invalid response: expected {expected}, got: {got}"))]
    ProtocolError {
        expected: &'static str,
        got: &'static str,
    },
}
