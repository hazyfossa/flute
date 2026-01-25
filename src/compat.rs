#[cfg(feature = "futures")]
pub mod futures {
    use futures_util::{
        SinkExt as Sink, StreamExt as Stream,
        stream::{SplitSink, SplitStream},
    };

    use crate::{Channel, Error, define::*};

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

    // TODO: this code could've been a generic wrapper of Tx + Rx => Channel
    impl<Wire, T> Channel<Wire> for Adapter<T>
    where
        T: Stream<Item = Wire> + Sink<Wire> + Unpin,
        Error: From<T::Error>,
    {
        fn recv(&mut self) -> impl Future<Output = Result<Wire, Error>> {
            Rx::recv(self)
        }

        fn send(&mut self, data: Wire) -> impl Future<Output = Result<(), Error>> {
            Tx::send(self, data)
        }
    }

    impl<Wire, T> split::Split<Wire> for Adapter<T>
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

    use crate::{Error, define::*};

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

    pub type KanalChannel<T> = merge::Merged<AsyncSender<T>, AsyncReceiver<T>>;

    pub fn unbounded<T>() -> KanalChannel<T> {
        merge::merge(kanal::unbounded_async())
    }

    pub fn bounded<T>(size: usize) -> KanalChannel<T> {
        merge::merge(kanal::bounded_async(size))
    }
}

#[cfg(feature = "data-json")]
mod json {}

#[cfg(feature = "data-postcard")]
mod postcard {}
