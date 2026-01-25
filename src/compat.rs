#[cfg(feature = "futures")]
pub mod futures {
    use futures_util::{
        SinkExt as Sink, StreamExt as Stream,
        stream::{SplitSink, SplitStream},
    };

    use crate::{modifiers::split::*, primitives::*};

    pub struct Adapter<T>(pub T);

    impl<Wire, T: Sink<Wire> + Unpin> Tx<Wire> for Adapter<T>
    where
        Error: From<T::Error>,
    {
        async fn send(&mut self, data: Wire) -> Result<(), Error> {
            Ok(self.0.send(data).await?)
        }
    }

    impl<T: Stream + Unpin> Rx<T::Item> for Adapter<T> {
        async fn recv(&mut self) -> Result<T::Item, Error> {
            match self.0.next().await {
                Some(data) => Ok(data),
                None => Err(Error::Closed),
            }
        }
    }

    impl<Wire, T> Split<Wire> for Adapter<T>
    where
        T: Stream<Item = Wire> + Sink<Wire> + Unpin,
        Error: From<T::Error>,
    {
        type Rx = Adapter<SplitStream<T>>;
        type Tx = Adapter<SplitSink<T, Wire>>;

        fn split(self) -> (Self::Tx, Self::Rx) {
            let (tx, rx) = self.0.split();
            (Adapter(tx), Adapter(rx))
        }
    }
}

#[cfg(feature = "kanal")]
pub mod kanal {
    use kanal::{AsyncReceiver, AsyncSender};

    use crate::{modifiers::merge::*, primitives::*};

    impl<T> Tx<T> for AsyncSender<T> {
        async fn send(&mut self, data: T) -> Result<(), Error> {
            AsyncSender::send(self, data)
                .await
                .map_err(|_| Error::Closed)
        }
    }

    impl<T> Rx<T> for AsyncReceiver<T> {
        async fn recv(&mut self) -> Result<T, Error> {
            AsyncReceiver::recv(self).await.map_err(|_| Error::Closed)
        }
    }

    pub type KanalChannel<T> = Merged<AsyncSender<T>, AsyncReceiver<T>>;

    pub fn unbounded<T>() -> KanalChannel<T> {
        merge(kanal::unbounded_async())
    }

    pub fn bounded<T>(size: usize) -> KanalChannel<T> {
        merge(kanal::bounded_async(size))
    }
}

#[cfg(feature = "data-json")]
mod json {}

#[cfg(feature = "data-postcard")]
mod postcard {}
