#[cfg(feature = "kanal")]
pub mod in_memory {
    pub use crate::compat::kanal::*;
}

pub mod batching {
    use std::marker::PhantomData;

    use serde::{Deserialize, Serialize};

    use crate::{Channel, DataFormat};

    #[derive(Serialize, Deserialize)]
    enum BatchingTag<T> {
        Single(T),
        Batch(Vec<T>),
    }

    pub struct WithBatching<C: Channel, F> {
        _f: PhantomData<F>,
        inner: C,
        buf: Vec<C::Wire>,
    }

    impl<C: Channel, F: DataFormat> Channel for WithBatching<C, F> {
        type Wire = C::Wire;

        async fn recv(&mut self) -> Result<Self::Wire, crate::ChannelError> {
            if let Some(batch_unroll) = self.buf.pop() {
                return Ok(batch_unroll);
            };

            let data = self.inner.recv();
            let tagged: BatchingTag<T> = self.inner.recv();
            match tagged {
                BatchingTag::Single(data) => Ok(data),
                BatchingTag::Batch(batch) => {
                    self.buf.extend(batch);
                    self.recv().await
                }
            }
        }

        async fn send(&mut self, data: Self::Wire) -> Result<(), crate::ChannelError> {
            self.inner.send(BatchingTag::Single(data))
        }
    }
}
