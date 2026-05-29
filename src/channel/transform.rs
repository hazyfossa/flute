use snafu::ResultExt;

use crate::{Rx, Tx, error::ErrorProvider};

pub trait Transform: ErrorProvider {
    type Before;
    type After;

    fn encode(&mut self, before: Self::Before) -> Result<Self::After, Self::Error>;
    fn decode(&mut self, after: Self::After) -> Result<Self::Before, Self::Error>;
}

// transformed channel

pub struct Transformed<I, T> {
    inner: I,
    transform: T,
}

impl<I, T> Tx for Transformed<I, T>
where
    I: Tx,
    T: Transform<After = I::In>,
{
    type In = T::Before;

    async fn send(&mut self, data: Self::In) -> Result<(), crate::ChannelError> {
        let transformed = self
            .transform
            .encode(data)
            .whatever_context(format!("{} transform error", T::name()))?;

        self.inner.send(transformed).await
    }
}

impl<I, T> Rx for Transformed<I, T>
where
    I: Rx,
    T: Transform<After = I::Out>,
{
    type Out = T::Before;

    async fn recv(&mut self) -> Result<Self::Out, crate::ChannelError> {
        let data = self.inner.recv().await?;

        let transformed = self
            .transform
            .decode(data)
            .whatever_context(format!("{} transform error", T::name()))?;

        Ok(transformed)
    }
}
