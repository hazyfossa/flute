pub mod modifiers;
pub mod transform;

pub mod primitives {
    use snafu::Snafu;

    #[allow(async_fn_in_trait)]
    pub trait Tx<Wire> {
        async fn send(&mut self, data: Wire) -> Result<(), Error>;
    }

    #[allow(async_fn_in_trait)]
    pub trait Rx<Wire> {
        async fn recv(&mut self) -> Result<Wire, Error>;
    }

    // TODO: consider making Wire an associated type
    // pros: more concise code (at some places)
    // cons:
    // cannot make a channel what accepts different wire types
    // PhantomData<Wire> (at some places)
    pub trait Channel<Wire>: Tx<Wire> + Rx<Wire> {}
    impl<Wire, T: Tx<Wire> + Rx<Wire>> Channel<Wire> for T {}

    // TODO: better name, or never do primitives::*
    #[derive(Debug, Snafu)]
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
}
