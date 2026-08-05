pub mod dynamic;
pub mod ops;
pub mod transform;

// TODO: re-integrate Tx, Rx with ErrorProvider. Implementations should never self-erase.

pub trait Tx {
    type In;
    async fn send(&mut self, data: Self::In) -> Result<(), ChannelError>;
}

pub trait Rx {
    type Out;
    async fn recv(&mut self) -> Result<Self::Out, ChannelError>;
}

#[derive(Debug)]
pub enum ChannelError {
    Closed,
    Other(eyre::Error),
}

impl<T> From<T> for ChannelError
where
    T: Into<eyre::Error>,
{
    fn from(value: T) -> Self {
        Self::Other(value.into())
    }
}

hazymacros::trait_alias!(
    #[doc = "A channel is a combination of a Receiver and Transmitter"]
    pub Channel: Tx + Rx
);

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
