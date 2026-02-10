#![allow(async_fn_in_trait)]

use crate::error::Typed;

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

// TODO: docs on trait_alias
// A channel is a combination of a Receiver and Transmitter
crate::trait_alias!(pub trait Channel: Tx + Rx);

/// A wire is a channel, for which input and output are the same
pub trait Wire<T>: Channel<In = T, Out = T> {}
impl<T, W: Channel<In = T, Out = T>> Wire<T> for W {}
