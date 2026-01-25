use std::error::Error;

use snafu::Snafu;

use crate::flow::{Flow, FlowError};

#[macro_export]
macro_rules! define_rpc {
    ($service:ident { $($function:ident { $($field:ident: $field_type:ty),* } -> $response:ty),* $(,)? }) => {
    paste::paste!{
        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum [<$service Request>] {
            $($function { $($field: $field_type),* }),*
        }


        #[allow(async_fn_in_trait)]
        pub trait [<$service Handler>] {
            type Error;
            $(
                fn [<$function:snake>](&self, $($field: $field_type),*) -> Result<$response, Self::Error>;
            )*
        }

        impl<T: [<$service Handler>], Wire> $crate::rpc::Handler<Wire> for T {
            async fn handle<C, F>(&mut self, mut flow: impl $crate::flow::Flow) -> $crate::rpc::Result<()>
            where
                C: $crate::Channel<Wire>,
                F: $crate::DataFormat<Repr = Wire>,
                {
                    loop {
                        match rpc.recv().await? {
                            $([<$service Request>]::$function { $($field),* } => {
                                let response = self.[<$function:snake>]($($field),*)
                                    .map_err(|e| $crate::rpc::RpcError::HandlerError(e.into()))?;

                                rpc.send(response).await?;
                            }),*
                        }
                    }
                }
        }


        pub struct $service<F>(F);

        impl<F: $crate::Flow> $service<F>
        {
            pub fn bind(flow: F) -> Self {
                Self(rpc)
            }

            $(pub async fn [<$function:snake>](
                &mut self, $($field: $field_type),*
            ) -> Result<$response, $crate::FlowError> {
                let request = [<$service Request>]::$function { $($field),* };
                self.0.send(request).await?;
                Ok(self.0.recv().await?)
            })*
        }
    }};
}

#[derive(Debug, Snafu)]
pub enum RPCError {
    #[snafu(transparent)]
    FlowError { source: FlowError },
    // TODO: in band error handling, remove this
    #[snafu(transparent)]
    HandlerError { source: Box<dyn Error> },
}

#[allow(async_fn_in_trait)]
pub trait Handler<Wire> {
    async fn serve<C, F>(&mut self, flow: impl Flow) -> Result<(), RPCError>;
}
