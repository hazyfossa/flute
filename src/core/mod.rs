#![allow(async_fn_in_trait)]

pub mod dynamic;
pub mod error;
pub mod ops;
pub mod transform;

pub trait Tx {
    type In;
    async fn send(&mut self, data: Self::In) -> Result<(), Error>;
}

pub trait Rx {
    type Out;
    async fn recv(&mut self) -> Result<Self::Out, Error>;
}

#[derive(Debug, snafu::Snafu)]
pub enum Error {
    #[snafu(display("channel closed"))]
    Closed,
    #[snafu(whatever)]
    Other {
        message: String,
        #[snafu(source(from(Box<dyn error::Typed>, Some)))]
        source: Option<Box<dyn error::Typed>>,
    },
}

crate::trait_alias!(pub trait Channel: Tx + Rx);
pub trait Wire<T>: Channel<In = T, Out = T> {}
impl<T, W: Channel<In = T, Out = T>> Wire<T> for W {}
