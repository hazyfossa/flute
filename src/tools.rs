#[cfg(feature = "kanal")]
pub mod in_memory {
    pub use crate::{
        compat::kanal::{self, KanalChannel},
        define::cross::*,
    };

    type InMemoryChannel<Wire> = Crossed<Wire, KanalChannel<Wire>, KanalChannel<Wire>>;

    type InMemoryPair<Wire> = (InMemoryChannel<Wire>, InMemoryChannel<Wire>);

    pub fn unbounded_pair<Wire>() -> InMemoryPair<Wire> {
        let a = kanal::unbounded();
        let b = kanal::unbounded();
        cross(a, b)
    }

    pub fn bounded_pair<Wire>(size: usize) -> InMemoryPair<Wire> {
        let a = kanal::bounded(size);
        let b = kanal::bounded(size);
        cross(a, b)
    }
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

// #[cfg(feature = "dyn")]
// pub mod disperse {
//     use crate::*;

//     struct RoundRobin<T> {
//         values: Vec<T>,
//         current: usize,
//         cnt: u8,
//         switch_threshold: u8,
//     }

//     impl<T> RoundRobin<T> {
//         fn new(values: Vec<T>, switch_threshold: u8) -> Self {
//             Self {
//                 values,
//                 current: 0,
//                 cnt: 0,
//                 switch_threshold,
//             }
//         }

//         fn access(&mut self) -> &mut T {
//             self.cnt += 1;

//             if self.cnt >= self.switch_threshold {
//                 self.cnt = 0;
//                 self.current = (self.current + 1) % self.values.len();
//             }

//             &mut self.values[self.current]
//         }
//     }

//     pub struct Dispersed<'a, T> {
//         round_robin: RoundRobin<Box<DynChannel<'a, T>>>,
//     }

//     impl<'a, Wire> Dispersed<'a, Wire> {
//         pub fn new(channels: &[&'a DynChannel<Wire>], switch_threshold: u8) -> Self {
//             Self {
//                 round_robin: RoundRobin::new(channels, switch_threshold),
//             }
//         }
//     }

//     impl<'a, Wire> Channel<Wire> for Dispersed<'a, Wire> {
//         fn send(&mut self, data: Wire) -> impl Future<Output = Result<(), Error>> {
//             self.round_robin.access().send(data)
//         }

//         fn recv(&mut self) -> impl Future<Output = Result<Wire, Error>> {
//             self.round_robin.access().recv()
//         }
//     }
// }
