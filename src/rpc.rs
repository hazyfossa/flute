// TODO: channel setup flow. Server/Client-tagged channels?

use snafu::Snafu;

#[macro_export]
macro_rules! define_rpc {
    ($vis:vis $service:ident {
        $(fn $function:ident ( $($field:ident: $field_type:ty),* ) -> $response:ty),* $(,)?
    }) => {

    #[allow(non_snake_case)]
    $vis mod $service {
        use super::*;

        #[allow(async_fn_in_trait)]
        pub trait Handler
        {
            $(
                fn $function(&self, $($field: $field_type),*) -> $crate::rpc::RpcResult<$response>;
            )*
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

        pub async fn server<C>(handler: impl Handler, mut channel: C) -> Result<(), $crate::Error>
        where
        C: $crate::Channel<
            In = Response,
            Out = Request,
        >
        {
            loop {
                match channel.recv().await? {
                    $(Request::$function { $($field),* } => {
                        let ret = handler.$function($($field),*);

                        let response = match ret {
                            Ok(value) => Response::$function(value),
                            Err(e) => Response::_Error { message: e.message },
                        };

                        channel.send( response ).await?;
                    }),*
                }
            }
        }


        pub struct Client<C>(C);

        impl<C> Client<C>
        where
            C: $crate::Channel<
                In = Request,
                Out = Response,
            >
        {
            $(pub async fn $function(
                &mut self, $($field: $field_type),*
            ) -> Result<$response, $crate::rpc::ClientError> {
                let request = Request::$function { $($field),* };

                self.0.send(request).await?;

                match self.0.recv().await? {
                    // Ok
                    Response::$function(ret) => Ok(ret),

                    // Remote function failed
                    Response::_Error { message } => Err($crate::rpc::ClientError::FunctionError { message }),

                    // Invalid response
                    _ => Err($crate::Error::Other {
                        // TODO: we cannot print other here, as that forces Debug
                        // work around by codegen per field? Adds bloat...
                        message: format!("got invalid response for {}", stringify!($function)),
                        source: None
                    }.into()
                    )
                }
            })*
        }

        pub fn client<C>(channel: C) -> Client<C>
        where
        C: $crate::Channel<
            In = Request,
            Out = Response,
        >
        { Client(channel) }
    }};
}

pub struct RpcErrorHatch {
    pub message: String,
}

impl<T: ToString> From<T> for RpcErrorHatch {
    fn from(value: T) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

pub type RpcResult<T> = std::result::Result<T, RpcErrorHatch>;

#[derive(Debug, Snafu)]
pub enum ClientError {
    #[snafu(transparent)]
    ChannelError { source: crate::Error },

    #[snafu(display("{message}"))]
    FunctionError { message: String },
}
