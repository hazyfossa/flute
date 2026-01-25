#![allow(async_fn_in_trait)]
pub mod define;
pub mod transform;

#[derive(Debug, snafu::Snafu)]
pub enum Error {
    #[snafu(display("channel closed"))]
    Closed,
    #[snafu(whatever)]
    Other {
        message: String,
        #[snafu(source(from(Box<dyn std::error::Error>, Some)))]
        source: Option<Box<dyn std::error::Error>>,
    },
}

// TODO: consider making Wire an associated type
// pros: more concise code (at some places)
// cons:
// cannot make a channel what accepts different wire types
// PhantomData<Wire> (at some places)
#[cfg_attr(feature = "dyn", dynosaur::dynosaur(pub DynChannel = dyn(box) Channel))]
pub trait Channel<Wire> {
    async fn send(&mut self, data: Wire) -> Result<(), Error>;
    async fn recv(&mut self) -> Result<Wire, Error>;
}
