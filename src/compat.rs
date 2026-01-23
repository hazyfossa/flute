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
    use crate::ChannelError;

    use kanal::{AsyncReceiver, AsyncSender};

    pub struct Channel<T> {
        tx: AsyncSender<T>,
        rx: AsyncReceiver<T>,
    }

    impl<T> crate::Channel for Channel<T> {
        type Wire = T;
        async fn recv(&mut self) -> Result<Self::Wire, ChannelError> {
            self.rx.recv().await.map_err(|_| ChannelError::Closed)
        }

        async fn send(&mut self, data: Self::Wire) -> Result<(), ChannelError> {
            self.tx.send(data).await.map_err(|_| ChannelError::Closed)
        }
    }

    pub fn channel_pair<T>(size: usize) -> (Channel<T>, Channel<T>) {
        let (a_tx, a_rx) = kanal::bounded_async(size);
        let (b_tx, b_rx) = kanal::bounded_async(size);

        let a = Channel { tx: a_tx, rx: b_rx };
        let b = Channel { tx: b_tx, rx: a_rx };

        (a, b)
    }
}
