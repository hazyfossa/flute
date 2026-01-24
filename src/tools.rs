#[cfg(feature = "kanal")]
pub mod in_memory {
    pub use crate::compat::kanal::*;
}

// TODO: make use of this in rpc client (need BatchedFlow trait?)
pub mod batching {
    use std::any::Any;

    use serde::{Deserialize, Serialize, de::DeserializeOwned};

    use crate::flow::*;

    #[derive(Serialize, Deserialize)]
    enum BatchingTag<T> {
        Single(T),
        Batch(Vec<T>),
    }

    pub struct WithBatching<F: Flow> {
        flow: F,
        buf: Vec<Box<dyn Any>>,
    }

    impl<F: Flow> Flow for WithBatching<F> {
        type Format = F::Format;

        async fn recv<V: DeserializeOwned + 'static>(&mut self) -> Result<V, FlowError> {
            if let Some(previous_batch_unroll) = self.buf.pop() {
                return Ok(*previous_batch_unroll.downcast().unwrap());
            }

            let tagged: BatchingTag<V> = self.flow.recv().await?;
            match tagged {
                BatchingTag::Single(data) => Ok(data),
                BatchingTag::Batch(batch) => {
                    for x in batch {
                        self.buf.push(Box::new(x));
                    }
                    self.recv().await
                }
            }
        }

        async fn send<V: Serialize + 'static>(&mut self, value: V) -> Result<(), FlowError> {
            self.flow.send(BatchingTag::Single(value)).await
        }
    }
}

pub mod cross {
    use crate::{
        merge::{Merged, merge_remap},
        primitives::*,
        split::Split,
    };

    #[allow(type_alias_bounds)]
    type Crossed<Wire, A: Split<Wire>, B: Split<Wire>> = Merged<A::Tx, B::Rx>;

    pub fn cross<'a, Wire, A, B>(a: A, b: B) -> (Crossed<Wire, A, B>, Crossed<Wire, B, A>)
    where
        A: Channel<Wire> + Split<Wire>,
        B: Channel<Wire> + Split<Wire>,
    {
        let (a_tx, a_rx) = a.split();
        let (b_tx, b_rx) = b.split();

        (merge_remap(a_tx, b_rx), merge_remap(b_tx, a_rx))
    }
}
