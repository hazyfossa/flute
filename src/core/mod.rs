#![allow(async_fn_in_trait)]

use crate::error::{ErrorProvider, Typed};

pub mod dynamic;
pub mod error;
pub mod ops;
pub mod transform;

pub trait Tx {
    type In;
    async fn send(&mut self, data: Self::In) -> Result<(), ChannelError>;
}

pub trait Rx {
    type Out;
    async fn recv(&mut self) -> Result<Self::Out, ChannelError>;
}

#[derive(Debug, snafu::Snafu)]
pub enum ChannelError {
    #[snafu(display("channel closed"))]
    Closed,

    #[snafu(whatever)]
    Other {
        message: String,
        #[snafu(source(from(Box<dyn Typed>, Some)))]
        source: Option<Box<dyn Typed>>,
    },
}

crate::trait_alias!(
    #[doc = "A channel is a combination of a Receiver and Transmitter"]
    pub trait Channel: Tx + Rx
);

impl<T> ErrorProvider for T
where
    T: Channel,
{
    type Error = ChannelError;
}

crate::trait_alias!(
    #[doc = "A wire is a channel, for which input and output are the same"]
    pub trait Wire<T>: Channel<In = T, Out = T>
);
