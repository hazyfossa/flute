use std::any::type_name;

use crate::{error::ErrorProvider, *};
use snafu::ResultExt;

pub trait Transform: ErrorProvider {
    type In;
    type Out;

    fn encode(&mut self, data: Self::In) -> Result<Self::Out, Self::Error>;
    fn decode(&mut self, data: Self::Out) -> Result<Self::In, Self::Error>;
}

pub struct Transformed<T, I> {
    transform: T,
    inner: I,
}

impl<T, I> Rx for Transformed<T, I>
where
    T: Transform,
    I: Rx<Out = T::Out>,
{
    type Out = T::In;
    async fn recv(&mut self) -> Result<T::In, Error> {
        let data = self.inner.recv().await?;

        let transformed = self
            .transform
            .decode(data)
            .whatever_context(format!("{} transform error", type_name::<T>()))?;

        Ok(transformed)
    }
}

impl<T, I> Tx for Transformed<T, I>
where
    T: Transform,
    I: Tx<In = T::Out>,
{
    type In = T::In;

    async fn send(&mut self, data: T::In) -> Result<(), Error> {
        let transformed = self
            .transform
            .encode(data)
            .whatever_context(format!("{} transform error", type_name::<T>()))?;

        Ok(self.inner.send(transformed).await?)
    }
}

pub trait TransformExt {
    fn transform<T: Transform>(self, transform: T) -> Transformed<T, Self>
    where
        Self: Sized,
    {
        Transformed {
            transform,
            inner: self,
        }
    }
}

impl<T: Channel> TransformExt for T {}
