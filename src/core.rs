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

pub mod channel {
    use snafu::Snafu;

    use crate::{
        data_format::DataFormat,
        flow::{DirectFlow, Flow},
    };

    #[derive(Debug, Snafu)]
    pub enum ChannelError {
        #[snafu(display("channel closed"))]
        Closed,
        #[snafu(whatever)]
        Other {
            message: String,
            #[snafu(source(from(Box<dyn std::error::Error>, Some)))]
            source: Option<Box<dyn std::error::Error>>,
        },
    }

    #[allow(async_fn_in_trait)]
    pub trait Channel {
        type Wire;

        async fn recv(&mut self) -> Result<Self::Wire, ChannelError>;
        async fn send(&mut self, data: Self::Wire) -> Result<(), ChannelError>;

        // TODO: consider making this an extension
        fn with_data_format<F>(self) -> impl Flow
        where
            Self: Sized,
            F: DataFormat<Repr = Self::Wire>,
        {
            DirectFlow::<Self, F>::new(self)
        }
    }
}

pub mod flow {
    use std::marker::PhantomData;

    use serde::{Serialize, de::DeserializeOwned};
    use snafu::Snafu;

    use crate::{channel::*, data_format::*};

    #[derive(Debug, Snafu)]
    pub enum FlowError {
        #[snafu(transparent)]
        ChannelError { source: ChannelError },
        #[snafu(transparent)]
        DataFormatError { source: DataFormatError },
    }

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

    impl<Wire, C: Channel<Wire = Wire>, F: DataFormat<Repr = Wire>> Flow for DirectFlow<C, F> {
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
}

pub mod transform {
    use std::{any::type_name, marker::PhantomData};

    use super::channel::{Channel, ChannelError};
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

    impl<C, T, In, Out> Channel for Transformed<C, T>
    where
        C: Channel<Wire = Out>,
        T: Transform<In = In, Out = Out>,
    {
        type Wire = In;

        async fn recv(&mut self) -> Result<Self::Wire, ChannelError> {
            let data = self.channel.recv().await?;
            Ok(self.transform.decode(data)?)
        }

        async fn send(&mut self, data: Self::Wire) -> Result<(), ChannelError> {
            let data = self.transform.encode(data)?;
            Ok(self.channel.send(data).await?)
        }
    }

    impl<T: Transform> From<TransformError<T>> for ChannelError {
        fn from(value: TransformError<T>) -> Self {
            ChannelError::Other {
                message: value.to_string(),
                source: Some(value.source),
            }
        }
    }

    pub trait ChannelTransformExt: Channel {
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

    impl<T: Channel> ChannelTransformExt for T {}
}
