use std::{error::Error, marker::PhantomData};

use serde::{Serialize, de::DeserializeOwned};
use snafu::Snafu;

// TODO: named errors for DataFormat and Transport

#[derive(Debug, Snafu)]
#[snafu(transparent)]
pub struct DataFormatError {
    source: Box<dyn Error>,
}

pub trait DataFormat {
    const NAME: &str;

    type Repr;

    fn encode<T: Serialize>(&mut self, value: T) -> Result<Self::Repr, DataFormatError>;
    fn decode<T: DeserializeOwned>(&mut self, data: Self::Repr) -> Result<T, DataFormatError>;
}

#[derive(Debug, Snafu)]
#[snafu(display("{} codec error", C::NAME))]
pub struct CodecError<C: Codec + ?Sized> {
    _c: PhantomData<C>,
    source: Box<dyn std::error::Error>,
}

pub trait Codec {
    const NAME: &str;

    type In;
    type Out;

    fn encode(&mut self, data: Self::In) -> Result<Self::Out, CodecError<Self>>;
    fn decode(&mut self, data: Self::Out) -> Result<Self::In, CodecError<Self>>;
}

pub struct WithCodec<T, Co> {
    transport: T,
    codec: Co,
}

impl<T, Co, In, Out> Transport for WithCodec<T, Co>
where
    T: Transport<Wire = Out>,
    Co: Codec<In = In, Out = Out>,
{
    type Wire = In;

    async fn recv(&mut self) -> Result<Self::Wire, TransportError> {
        let data = self.transport.recv().await?;
        Ok(self.codec.decode(data)?)
    }

    async fn send(&mut self, data: Self::Wire) -> Result<(), TransportError> {
        let data = self.codec.encode(data)?;
        Ok(self.transport.send(data).await?)
    }
}

impl<C: Codec> From<CodecError<C>> for TransportError {
    fn from(value: CodecError<C>) -> Self {
        TransportError::Other {
            message: value.to_string(),
            source: Some(value.source),
        }
    }
}

#[derive(Debug, Snafu)]
pub enum TransportError {
    #[snafu(display("transport closed"))]
    Closed,
    #[snafu(whatever)]
    Other {
        message: String,
        #[snafu(source(from(Box<dyn std::error::Error>, Some)))]
        source: Option<Box<dyn std::error::Error>>,
    },
}

#[allow(async_fn_in_trait)]
pub trait Transport {
    type Wire;

    async fn recv(&mut self) -> Result<Self::Wire, TransportError>;
    async fn send(&mut self, data: Self::Wire) -> Result<(), TransportError>;
}

pub trait TransportExt: Transport {
    fn with_codec<C: Codec>(self, codec: C) -> WithCodec<Self, C>
    where
        Self: Sized,
    {
        WithCodec {
            transport: self,
            codec,
        }
    }
}

impl<T: Transport> TransportExt for T {}
