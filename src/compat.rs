#[cfg(feature = "futures")]
pub mod futures {
    use std::marker::PhantomData;

    use futures_util::{
        SinkExt as Sink, StreamExt as Stream,
        stream::{SplitSink, SplitStream},
    };

    use crate::{Error, Rx, Tx, ops::split};

    pub struct Adapter<T, Wire> {
        inner: T,
        // TODO: is there really no way
        // to bridge sink without this?
        _wire: PhantomData<Wire>,
    }

    impl<T, Wire> Adapter<T, Wire> {
        pub fn wrap(future: T) -> Self {
            Self {
                inner: future,
                _wire: PhantomData,
            }
        }
    }

    impl<Wire, T: Sink<Wire> + Unpin> Tx for Adapter<T, Wire>
    where
        Error: From<T::Error>,
    {
        type In = Wire;
        async fn send(&mut self, data: Wire) -> Result<(), Error> {
            Ok(self.inner.send(data).await?)
        }
    }

    impl<T: Stream + Unpin> Rx for Adapter<T, T::Item> {
        type Out = T::Item;

        async fn recv(&mut self) -> Result<T::Item, Error> {
            match self.inner.next().await {
                Some(data) => Ok(data),
                None => Err(Error::Closed),
            }
        }
    }

    impl<Wire, T> split::Split for Adapter<T, Wire>
    where
        T: Stream<Item = Wire> + Sink<Wire> + Unpin,
        Error: From<T::Error>,
    {
        type Rx = Adapter<SplitStream<T>, Wire>;
        type Tx = Adapter<SplitSink<T, Wire>, Wire>;

        fn split(self) -> (Self::Tx, Self::Rx) {
            let (tx, rx) = self.inner.split();
            (Adapter::wrap(tx), Adapter::wrap(rx))
        }
    }
}

#[cfg(feature = "kanal")]
pub mod kanal {
    use kanal::{AsyncReceiver, AsyncSender};

    use crate::{Error, Rx, Tx, ops::merge};

    impl<T> Tx for AsyncSender<T> {
        type In = T;

        async fn send(&mut self, data: T) -> Result<(), Error> {
            AsyncSender::send(self, data)
                .await
                .map_err(|_| Error::Closed)
        }
    }

    impl<T> Rx for AsyncReceiver<T> {
        type Out = T;

        async fn recv(&mut self) -> Result<T, Error> {
            AsyncReceiver::recv(self).await.map_err(|_| Error::Closed)
        }
    }

    pub type KanalWire<T> = merge::Merged<AsyncSender<T>, AsyncReceiver<T>>;

    pub fn unbounded<T>() -> KanalWire<T> {
        merge::merge(kanal::unbounded_async())
    }

    pub fn bounded<T>(size: usize) -> KanalWire<T> {
        merge::merge(kanal::bounded_async(size))
    }
}

#[cfg(feature = "data-json")]
pub mod json {
    use std::marker::PhantomData;

    use serde::{Serialize, de::DeserializeOwned};

    use crate::{error::ErrorProvider, transform::*};

    pub struct Json<Value>(PhantomData<Value>);

    impl<Value> ErrorProvider for Json<Value> {
        type Error = serde_json::error::Error;
    }

    impl<Value: Serialize> TransformTx for Json<Value> {
        type In = Value;
        type Out = Vec<u8>;

        fn encode(&mut self, data: Self::In) -> Result<Self::Out, Self::Error> {
            Ok(serde_json::to_vec(&data)?)
        }
    }

    // TODO: slices are blocked on v3
    impl<'de, Value: DeserializeOwned> TransformRx for Json<Value> {
        type In = Value;
        type Out = Vec<u8>;

        fn decode(&mut self, data: Self::Out) -> Result<Self::In, Self::Error> {
            Ok(serde_json::from_slice(&data)?)
        }
    }
}

#[cfg(feature = "data-postcard")]
mod postcard {}
