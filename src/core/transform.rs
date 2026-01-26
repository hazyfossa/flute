use std::{any::type_name, marker::PhantomData};

use crate::*;
use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(display("{} transform error", type_name::<T>()))]
pub struct TransformError<T: Transform + ?Sized> {
    _t: PhantomData<T>,
    source: Box<dyn std::error::Error>,
}

pub trait Transform {
    type In;
    type Out;

    fn encode(&mut self, data: Self::In) -> Result<Self::Out, TransformError<Self>>;
    fn decode(&mut self, data: Self::Out) -> Result<Self::In, TransformError<Self>>;
}

pub struct Transformed<C, T> {
    channel: C,
    transform: T,
}

impl<Wire, C, T> Channel<T::In> for Transformed<C, T>
where
    C: Channel<Wire>,
    T: Transform<Out = Wire>,
{
    async fn send(&mut self, data: T::In) -> Result<(), Error> {
        let data = self.transform.encode(data)?;
        Ok(self.channel.send(data).await?)
    }

    async fn recv(&mut self) -> Result<T::In, Error> {
        let data = self.channel.recv().await?;
        Ok(self.transform.decode(data)?)
    }
}

impl<T: Transform> From<TransformError<T>> for Error {
    fn from(value: TransformError<T>) -> Self {
        Error::Other {
            message: value.to_string(),
            source: Some(value.source),
        }
    }
}

pub trait ChannelTransformExt<Wire>: Channel<Wire> {
    fn transform<T: Transform>(self, transform: T) -> Transformed<Self, T>
    where
        Self: Sized,
    {
        Transformed {
            channel: self,
            transform,
        }
    }
}

impl<Wire, T: Channel<Wire>> ChannelTransformExt<Wire> for T {}
