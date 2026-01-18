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
