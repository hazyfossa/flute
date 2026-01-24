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

pub mod merge {
    use crate::primitives::*;

    pub struct Merged<Tx, Rx> {
        pub tx: Tx,
        pub rx: Rx,
    }

    impl<Wire, A: Tx<Wire>, B> Tx<Wire> for Merged<A, B> {
        fn send(&mut self, data: Wire) -> impl Future<Output = Result<(), Error>> {
            self.tx.send(data)
        }
    }

    impl<Wire, A, B: Rx<Wire>> Rx<Wire> for Merged<A, B> {
        fn recv(&mut self) -> impl Future<Output = Result<Wire, Error>> {
            self.rx.recv()
        }
    }

    pub fn merge_remap<Wire, A, B>(tx: A, rx: B) -> Merged<A, B>
    where
        A: Tx<Wire>,
        B: Rx<Wire>,
    {
        Merged { tx, rx }
    }

    pub fn merge<Wire, A, B>(s: (A, B)) -> Merged<A, B>
    where
        A: Tx<Wire>,
        B: Rx<Wire>,
    {
        merge_remap(s.0, s.1)
    }
}

pub mod split {
    use crate::primitives::*;

    pub trait Split<Wire>: Channel<Wire> {
        type Tx: Tx<Wire>;
        type Rx: Rx<Wire>;
        fn split(self) -> (Self::Tx, Self::Rx);
    }

    impl<Wire, A, B> Split<Wire> for super::merge::Merged<A, B>
    where
        A: Tx<Wire>,
        B: Rx<Wire>,
    {
        type Tx = A;
        type Rx = B;

        fn split(self) -> (Self::Tx, Self::Rx) {
            (self.tx, self.rx)
        }
    }
}

pub mod downcast {
    use crate::primitives::*;

    pub struct DowncastTx<C>(C);
    impl<Wire, C: Channel<Wire>> Tx<Wire> for DowncastTx<C> {
        fn send(&mut self, data: Wire) -> impl Future<Output = Result<(), Error>> {
            self.0.send(data)
        }
    }

    pub struct DowncastRx<C>(C);
    impl<Wire, C: Channel<Wire>> Rx<Wire> for DowncastRx<C> {
        fn recv(&mut self) -> impl Future<Output = Result<Wire, Error>> {
            self.0.recv()
        }
    }

    pub trait ChannelDowncast<Wire>: Sized + Channel<Wire> {
        fn downcast_tx(self) -> impl Tx<Wire> {
            DowncastTx(self)
        }

        fn downcast_rx(self) -> impl Rx<Wire> {
            DowncastRx(self)
        }
    }
}

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

pub mod flow {
    use std::marker::PhantomData;

    use serde::{Serialize, de::DeserializeOwned};
    use snafu::Snafu;

    use crate::{data_format::*, primitives::*};

    #[derive(Debug, Snafu)]
    pub enum FlowError {
        #[snafu(transparent)]
        ChannelError { source: Error },
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
}

// TODO: allow transform on tx/rx (not only channel)
pub mod transform {
    use std::{any::type_name, marker::PhantomData};

    use crate::primitives::*;
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

    impl<Wire, C, T> Tx<T::In> for Transformed<C, T>
    where
        C: Channel<Wire>,
        T: Transform<Out = Wire>,
    {
        async fn send(&mut self, data: T::In) -> Result<(), Error> {
            let data = self.transform.encode(data)?;
            Ok(self.channel.send(data).await?)
        }
    }

    impl<Wire, C, T> Rx<T::In> for Transformed<C, T>
    where
        C: Channel<Wire>,
        T: Transform<Out = Wire>,
    {
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
}
