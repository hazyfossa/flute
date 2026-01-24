#[cfg(feature = "futures")]
mod futures {
    use futures_util::{SinkExt as Sink, StreamExt as Stream};

    use crate::{Channel, ChannelError};

    impl<Wire, T: Stream<Item = Wire> + Sink<Wire> + Unpin> Channel for T
    where
        ChannelError: From<T::Error>,
    {
        type Wire = Wire;

        async fn recv(&mut self) -> Result<Self::Wire, crate::ChannelError> {
            match self.next().await {
                Some(data) => Ok(data),
                None => Err(ChannelError::Closed),
            }
        }

        async fn send(&mut self, data: Self::Wire) -> Result<(), crate::ChannelError> {
            Ok(self.send(data).await?)
        }
    }
}

#[cfg(feature = "kanal")]
pub mod kanal {
    use crate::{Channel, ChannelError};

    use kanal::{AsyncReceiver, AsyncSender};

    pub struct KanalChannel<T> {
        tx: AsyncSender<T>,
        rx: AsyncReceiver<T>,
    }

    impl<T> Channel for KanalChannel<T> {
        type Wire = T;
        async fn recv(&mut self) -> Result<Self::Wire, ChannelError> {
            self.rx.recv().await.map_err(|_| ChannelError::Closed)
        }

        async fn send(&mut self, data: Self::Wire) -> Result<(), ChannelError> {
            self.tx.send(data).await.map_err(|_| ChannelError::Closed)
        }
    }

    pub fn pair<T>(size: usize) -> (KanalChannel<T>, KanalChannel<T>) {
        let (a_tx, a_rx) = kanal::bounded_async(size);
        let (b_tx, b_rx) = kanal::bounded_async(size);

        let a = KanalChannel { tx: a_tx, rx: b_rx };
        let b = KanalChannel { tx: b_tx, rx: a_rx };

        (a, b)
    }

    pub fn pair_unbounded<T>() -> (KanalChannel<T>, KanalChannel<T>) {
        let (a_tx, a_rx) = kanal::unbounded_async();
        let (b_tx, b_rx) = kanal::unbounded_async();

        let a = KanalChannel { tx: a_tx, rx: b_rx };
        let b = KanalChannel { tx: b_tx, rx: a_rx };

        (a, b)
    }
}

#[cfg(feature = "data-json")]
mod json {}

#[cfg(feature = "data-postcard")]
mod postcard {}
