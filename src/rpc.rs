use super::traits::*;
use eyre::Result;
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

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
            $(
                fn [<$function:snake>](&self, $($field: $field_type),*) -> eyre::Result<$response>;
            )*
        }

        impl<T: [<$service Handler>], Wire> flute::rpc::Handler<Wire> for T {
            async fn handle<C, F>(&mut self, mut rpc: flute::rpc::RPC<C, F>) -> Result<(), flute::rpc::RpcError>
            where
                C: Channel<Wire = Wire>,
                F: flute::DataFormat<Repr = Wire>,
                {
                    loop {
                        match rpc.recv().await? {
                            $([<$service Request>]::$function { $($field),* } => {
                                let response = self.[<$function:snake>]($($field),*)
                                    .map_err(|e| flute::rpc::RpcError::HandlerError(e.into()))?;

                                rpc.send(response).await?;
                            }),*
                        }
                    }
                }
        }


        pub struct $service<C, F>(flute::rpc::RPC<C, F>);

        impl<Wire, C, F> $service<C, F>
        where
            C: flute::Channel<Wire = Wire>,
            F: flute::DataFormat<Repr = Wire>
        {
            pub fn bind(rpc: flute::rpc::RPC<C, F>) -> Self {
                Self(rpc)
            }

            $(pub async fn [<$function:snake>](&mut self, $($field: $field_type),*) -> eyre::Result<$response> {
                let request = [<$service Request>]::$function { $($field),* };
                self.0.send(request).await?;
                Ok(self.0.recv().await?)
            })*
        }
    }};
}

#[allow(async_fn_in_trait)]
pub trait Handler<Wire> {
    async fn handle<C, F>(&mut self, rpc: RPC<C, F>) -> Result<(), RpcError>
    where
        C: Channel<Wire = Wire>,
        F: DataFormat<Repr = Wire>;
}

pub struct RPC<C, F> {
    channel: C,
    format: F,
}

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("channel closed")]
    ChannelClosed,
    #[error("handler error: {0}")]
    HandlerError(eyre::Error),
    #[error(transparent)]
    Other(#[from] eyre::Error),
}

impl From<ChannelError> for RpcError {
    fn from(value: ChannelError) -> Self {
        match value {
            ChannelError::Closed => Self::ChannelClosed,
            ChannelError::Transport(e) => Self::Other(e.wrap_err("channel error")),
        }
    }
}

impl<Wire, C: Channel<Wire = Wire>, F: DataFormat<Repr = Wire>> RPC<C, F> {
    pub fn new(channel: C, format: F) -> Self {
        Self { channel, format }
    }

    pub async fn recv<T: DeserializeOwned>(&mut self) -> Result<T, RpcError> {
        let data = self.channel.recv().await?;
        Ok(self.format.decode(data)?)
    }

    pub async fn send<T: Serialize>(&mut self, value: T) -> Result<(), RpcError> {
        let data = self.format.encode(value)?;
        Ok(self.channel.send(data).await?)
    }

    pub async fn serve(self, mut handler: impl Handler<Wire>) -> Result<(), RpcError> {
        handler.handle(self).await
    }
}
