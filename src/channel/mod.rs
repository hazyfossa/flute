#![allow(async_fn_in_trait)]

use crate::error::{ErasedError, ErrorProvider};

pub mod dynamic;
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

    #[snafu(context(false))]
    Other { source: ErasedError },
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

// A wire is a channel, for which input and output are the same
pub trait Wire {
    type Repr;
}

impl<T, Repr> Wire for T
where
    T: Channel<In = Repr, Out = Repr>,
{
    type Repr = Repr;
}
