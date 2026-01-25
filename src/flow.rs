use std::marker::PhantomData;

use serde::{Serialize, de::DeserializeOwned};
use snafu::Snafu;

use crate::primitives::*;

use data_format::*;
pub mod data_format {
    // TODO: decouple from serde
    use std::error::Error;

    use serde::{Serialize, de::DeserializeOwned};
    use snafu::Snafu;

    #[derive(Debug, Snafu)]
    #[snafu(transparent)]
    pub struct DataFormatError {
        source: Box<dyn Error>,
    }

    pub trait DataFormat {
        type Repr;

        fn encode<T: Serialize>(value: T) -> Result<Self::Repr, DataFormatError>;
        fn decode<T: DeserializeOwned>(data: Self::Repr) -> Result<T, DataFormatError>;
    }
}

#[derive(Debug, Snafu)]
pub enum FlowError {
    #[snafu(transparent)]
    ChannelError { source: Error },
    #[snafu(transparent)]
    DataFormatError { source: DataFormatError },
}

#[allow(async_fn_in_trait)]
pub trait Flow {
    type Format: DataFormat;

    async fn recv<V: DeserializeOwned + 'static>(&mut self) -> Result<V, FlowError>;
    async fn send<V: Serialize + 'static>(&mut self, value: V) -> Result<(), FlowError>;
}

pub struct DirectFlow<C, F> {
    channel: C,
    _format: PhantomData<F>,
}

impl<C, F> DirectFlow<C, F> {
    pub fn new(channel: C) -> Self {
        Self {
            channel,
            _format: PhantomData,
        }
    }
}

impl<Wire, C: Channel<Wire>, F: DataFormat<Repr = Wire>> Flow for DirectFlow<C, F> {
    type Format = F;

    async fn recv<V: DeserializeOwned + 'static>(&mut self) -> Result<V, FlowError> {
        let data = self.channel.recv().await?;
        Ok(Self::Format::decode(data)?)
    }

    async fn send<V: Serialize + 'static>(&mut self, value: V) -> Result<(), FlowError> {
        let data = Self::Format::encode(value)?;
        Ok(self.channel.send(data).await?)
    }
}

pub trait UseFlow<Wire>: Channel<Wire> {
    fn with_data_format<F>(self) -> impl Flow
    where
        Self: Sized,
        F: DataFormat<Repr = Wire>,
    {
        DirectFlow::<Self, F>::new(self)
    }
}

impl<Wire, T: Channel<Wire>> UseFlow<Wire> for T {}
