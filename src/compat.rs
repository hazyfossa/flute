#[cfg(feature = "futures")]
mod futures {
    use futures_util::{SinkExt as Sink, StreamExt as Stream};

    use crate::{Transport, TransportError};

    impl<Wire, T: Stream<Item = Wire> + Sink<Wire> + Unpin> Transport for T
    where
        TransportError: From<T::Error>,
    {
        type Wire = Wire;

        async fn recv(&mut self) -> Result<Self::Wire, crate::TransportError> {
            match self.next().await {
                Some(data) => Ok(data),
                None => Err(TransportError::Closed),
            }
        }

        async fn send(&mut self, data: Self::Wire) -> Result<(), crate::TransportError> {
            Ok(self.send(data).await?)
        }
    }
}
