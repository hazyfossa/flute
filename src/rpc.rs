// TODO: channel setup flow. Server/Client-tagged channels?

use std::fmt::Debug;

use eyre::eyre;
use hazymacros::trait_alias;
use thiserror::Error;

use crate::{Channel, ChannelError, error::ErrorProvider};

trait_alias!(Data: serde::Serialize + serde::de::DeserializeOwned);

#[allow(private_bounds)]
pub trait Service {
    type Request: Data;
    type Response: Data;

    type Client<C: Caller<Self>>: From<C>;
}

// Ordered channel caller

pub trait Caller<S: Service>: ErrorProvider {
    async fn call(&mut self, request: S::Request) -> Result<S::Response, Self::Error>;
}

// If we ever implement unordered channels, here is the place to put a bound
pub struct OrderedCaller<C>(pub C);

pub struct OrderedCallerError(eyre::Error);
impl From<ChannelError> for OrderedCallerError {
    fn from(value: ChannelError) -> Self {
        Self(match value {
            ChannelError::Other(e) => e,
            ChannelError::Closed => eyre!("channel closed in the middle of RPC call"),
        })
    }
}

impl Into<eyre::Error> for OrderedCallerError {
    fn into(self) -> eyre::Error {
        self.0
    }
}

impl<C> ErrorProvider for OrderedCaller<C> {
    type Error = OrderedCallerError;
}

impl<T, S> Caller<S> for OrderedCaller<T>
where
    S: Service,
    T: Channel<In = S::Request, Out = S::Response>,
{
    async fn call(&mut self, request: S::Request) -> Result<S::Response, Self::Error> {
        self.0.send(request).await?;
        Ok(self.0.recv().await?)
    }
}

// TODO: consider making this an ext trait
pub fn open_channel<S: Service>(
    channel: impl Channel<In = S::Request, Out = S::Response>,
) -> S::Client<OrderedCaller<impl Channel<In = S::Request, Out = S::Response>>> {
    S::Client::from(OrderedCaller(channel))
}

// Error handling

// TODO: this hatch is naive.
// With color-eyre, it will send color codes over the wire.
// With snafu, snafu::Report is not applied.
//
// TODO: this hatch is not a proper std Error
// we will most likely need a special case in client codegen
#[derive(serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("{0}")]
    CallerError(eyre::Error),

    #[error("{message}")]
    FunctionError { message: String },

    #[error("got invalid response: expected {expected}, got: {got}")]
    ProtocolError {
        expected: &'static str,
        got: &'static str,
    },
}

// Codegen

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

        pub trait Handler
        {
            $(
                async fn $function(&self, $($field: $field_type),*) -> $response;
            )*
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum Request {
            $($function { $($field: $field_type),* }),*
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum Response {
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

        impl<T> flute::error::ErrorProvider for Route<T> {
            type Error = std::convert::Infallible;
        }

        impl<T: Handler> rpc::Caller<Service> for Route<T> {
            async fn call(&mut self, request: Request) -> Result<Response, Self::Error> {
                Ok(match request {
                    $(Request::$function { $($field),* } => {
                        let ret = self.0.$function($($field),*).await;
                        Response::$function(ret)
                    }),*
                })
            }
        }

        pub struct Client<C>(C);

        impl<C> From<C> for Client<C>
        where C: Caller<Service>
        {
            fn from(caller: C) -> Self {
                Self(caller)
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
                    .map_err(|e| ClientError::CallerError(e.into()))?
                {
                    Response::$function(ret) => Ok(ret),

                    other => Err(ClientError::ProtocolError {
                        expected: stringify!($function),
                        got: std::any::type_name_of_val(&other)
                    })
                }
            })*
        }

        $crate::_h::feature_select!([$($feat)*]
        split_handler => {
            pub mod split_handler {
                use super::*;
                $(pub trait $function {
                    async fn handle(&self, $($field: $field_type),*) -> $response;
                })*

                impl<T> super::Handler for T where T: $($function +)* {
                    $(fn $function(&self, $($field: $field_type),*) -> impl Future<Output = $response> {
                        <Self as $function>::handle(self, $($field),*)
                    })*
                }
            }
        }
        use_eyre => {
            impl Into<Result<Self, ::eyre::Error>> for Response {
                fn into(self) -> Result<Self, ::eyre::Error> {
                    match self {
                        $(Self::$function(value) => match value {
                            Ok(v) => ::eyre::Ok(v),
                            Err(v) => ::eyre::Err(v),
                            not_result => Ok(not_result),
                        }),*
                    }
                }
            }
        }
        );
    }};
}
