// TODO: channel setup flow. Server/Client-tagged channels?

use std::fmt::Debug;

use snafu::Snafu;

use crate::{
    Channel,
    error::{ErrorProvider, Typed},
};

#[macro_export]
macro_rules! define_rpc {
    ($vis:vis $service:ident {
        $(fn $function:ident ( $($field:ident: $field_type:ty),* ) -> $response:ty),* $(,)?
    }) => {

    #[allow(non_snake_case)]
    $vis mod $service {
        use super::*;
        use $crate::rpc::*;
        use snafu::ResultExt;

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

        #[allow(non_camel_case_types)]
        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum Request {
            $($function { $($field: $field_type),* }),*
        }

        #[allow(non_camel_case_types)]
        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum Response {
            _Error { message: String },
            $($function($response)),*
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
        where
            C: Caller<
                Request = Request,
                Response = Response,
            >
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

                    _ => Err(ClientError::ProtocolError { expected: stringify!($function) })
                }
            })*
        }

        pub fn client<C>(channel: C) -> Client<C>
        where
        C: Caller<
            Request = Request,
            Response = Response,
        >
        { Client(channel) }
    }};
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
pub trait Caller: ErrorProvider {
    type Request;
    type Response;

    async fn call(&mut self, request: Self::Request) -> Result<Self::Response, Self::Error>;
}

impl<T> Caller for T
where
    T: Channel,
{
    type Request = T::In;
    type Response = T::Out;

    async fn call(&mut self, request: Self::Request) -> Result<Self::Response, Self::Error> {
        self.send(request).await?;
        self.recv().await
    }
}
