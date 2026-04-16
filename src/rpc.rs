// TODO: channel setup flow. Server/Client-tagged channels?

use std::fmt::Debug;

use snafu::Snafu;

use crate::{
    Channel,
    error::{ErrorProvider, Typed},
};

#[macro_export]
macro_rules! define_rpc {
    (
        $(#[$feat:tt])*
        $vis:vis $service:ident {
            $(fn $function:ident ( $($field:ident: $field_type:ty),* ) -> $response:ty;)*
        }
    ) => {

    #[allow(non_snake_case)]
    #[allow(non_camel_case_types)]
    $vis mod $service {
        use super::*;
        use $crate::rpc::*;
        use snafu::ResultExt;

        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum Request {
            $($function { $($field: $field_type),* }),*
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum Response {
            _Error { message: String },
            $($function($response)),*
        }

        #[allow(async_fn_in_trait)]
        pub trait Handler
        {
            $(
                fn $function(&self, $($field: $field_type),*) -> RpcResult<$response>;
            )*

            // TODO: make it clear you don't have to implement this
            fn handle(&self, request: Request) -> Response {
                match request {
                    $(Request::$function { $($field),* } => {
                        let ret = self.$function($($field),*);

                        let response = match ret {
                            Ok(value) => Response::$function(value),
                            Err(e) => Response::_Error { message: e.message },
                        };

                        response
                    }),*
                }
            }
        }


        pub async fn server<C>(handler: impl Handler, mut channel: C) -> Result<(), $crate::ChannelError>
        where
        C: $crate::Channel<
            In = Response,
            Out = Request,
        >
        {
            loop {
                let request = channel.recv().await?;
                let response = handler.handle(request);
                channel.send( response ).await?;
            }
        }


        pub struct Client<C>(C);

        impl<C> Client<C>
        where C: Caller<Request, Response>
        {
            pub fn new(caller: C) -> Self {
                Self(caller)
            }

            $(pub async fn $function(
                &mut self, $($field: $field_type),*
            ) -> Result<$response, ClientError> {
                let request = Request::$function { $($field),* };

                match self.0.call(request).await
                    .map_err(|e| ClientError::CallerError { source: e.into() })?
                {
                    Response::$function(ret) => Ok(ret),

                    Response::_Error { message } => Err(ClientError::FunctionError { message }),

                    _ => Err(ClientError::ProtocolError { expected: stringify!($function) })
                }
            })*
        }

        $crate::define_rpc!(@feature_select split_handler from $(#[$feat])*  => {
            pub mod split_handler {
                use super::*;
                $(pub trait $function {
                    fn handle(&self, $($field: $field_type),*) -> super::RpcResult<$response>;
                })*

                impl<T> super::Handler for T where T: $($function +)* {
                    $(fn $function(&self, $($field: $field_type),*) -> super::RpcResult<$response> {
                        <Self as $function>::handle(self, $($field),*)
                    })*
                }
            }
        });


    }};

    // TODO: While this is fun, seriously consider switching to proc-macros
    (@feature_select $feature:tt from $(#[$features:tt])* => { $($body:tt)* }) => {
        $crate::define_rpc!(@scope ($s:tt) => {
            macro_rules! selector {
                ($feature, $s(other:tt)*) => { $($body)* };
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

    #[snafu(display("got invalid response for {expected}"))]
    ProtocolError {
        expected: &'static str,
        // TODO
        // got: String,
    },
}

#[allow(async_fn_in_trait)]
pub trait Caller<Request, Response>: ErrorProvider {
    async fn call(&mut self, request: Request) -> Result<Response, Self::Error>;
}

impl<T> Caller<T::In, T::Out> for T
where
    T: Channel,
{
    async fn call(&mut self, request: T::In) -> Result<T::Out, Self::Error> {
        self.send(request).await?;
        self.recv().await
    }
}
