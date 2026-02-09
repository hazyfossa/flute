// TODO: channel setup flow. Server/Client-tagged channels?

use snafu::Snafu;

#[macro_export]
macro_rules! define_rpc {
    ($service:ident { $($function:ident { $($field:ident: $field_type:ty),* } -> $response:ty),* $(,)? }) => {

    paste::paste!{
        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum [<$service Request>] {
            $($function { $($field: $field_type),* }),*
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum [<$service Response>] {
            _Error { message: String },
            $($function($response)),*
        }


        #[allow(async_fn_in_trait)]
        pub trait [<$service Handler>]
        {
            $(
                fn [<$function:snake>](&self, $($field: $field_type),*) -> $crate::rpc::RpcResult<$response>;
            )*

            async fn serve<C>(&mut self, mut channel: C) -> Result<(), $crate::Error>
            where
                C: $crate::Channel<
                    In = [<$service Response>],
                    Out = [<$service Request>],
                >
                {
                    loop {
                        match channel.recv().await? {
                            $([<$service Request>]::$function { $($field),* } => {
                                let ret = self.[<$function:snake>]($($field),*);

                                let response = match ret {
                                    Ok(value) => [<$service Response>]::$function(value),
                                    Err(e) => [<$service Response>]::_Error { message: e.message },
                                };

                                channel.send( response ).await?;
                            }),*
                        }
                    }
                }
        }


        pub struct $service<C>(C);

        impl<C> $service<C>
        where
            C: $crate::Channel<
                In = [<$service Request>],
                Out = [<$service Response>],
            >
        {
            pub fn bind(channel: C) -> Self {
                Self(channel)
            }

            $(pub async fn [<$function:snake>](
                &mut self, $($field: $field_type),*
            ) -> Result<$response, $crate::rpc::ClientError> {
                let request = [<$service Request>]::$function { $($field),* };

                self.0.send(request).await?;

                match self.0.recv().await? {
                    // Ok
                    [<$service Response>]::$function(ret) => Ok(ret),

                    // Remote function failed
                    [<$service Response>]::_Error { message } => Err($crate::rpc::ClientError::FunctionError { message }),

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
