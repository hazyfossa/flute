#[cfg(feature = "kanal")]
pub mod in_memory {
    pub use crate::{
        compat::kanal::{self, KanalWire},
        ops::cross::*,
    };

    type InMemoryChannel<In, Out> = Crossed<KanalWire<In>, KanalWire<Out>>;

    type InMemoryPair<In, Out> = (InMemoryChannel<In, Out>, InMemoryChannel<Out, In>);

    pub fn unbounded_pair<In, Out>() -> InMemoryPair<In, Out> {
        let a: KanalWire<In> = kanal::unbounded();
        let b: KanalWire<Out> = kanal::unbounded();
        cross(a, b)
    }

    pub fn bounded_pair<In, Out>(size: usize) -> InMemoryPair<In, Out> {
        let a: KanalWire<In> = kanal::bounded(size);
        let b: KanalWire<Out> = kanal::bounded(size);
        cross(a, b)
    }
}

pub mod error_wrap {
    use crate::{error, transform::TransformRx};
    use std::marker::PhantomData;

    use crate::{error::ErrorProvider, transform::TransformFraming};

    pub struct Fallible<T, E>(PhantomData<(T, E)>);
    pub fn fallible<T, E>() -> Fallible<T, E> {
        Fallible(PhantomData)
    }

    impl<T, E: error::Typed> ErrorProvider for Fallible<T, E> {
        type Error = E;
    }

    // TODO: this is unintuitive, i feel like In/Out here should be swapped
    // but our transform trait disagrees
    impl<T, E: error::Typed> TransformFraming for Fallible<T, E> {
        type In = T;
        type Out = Result<T, E>;
    }

    impl<T, E: error::Typed> TransformRx for Fallible<T, E> {
        fn decode(&mut self, data: Self::Out) -> Result<Self::In, Self::Error> {
            let inner = data?;
            Ok(inner)
        }
    }
}

// pub mod split_any {
//     pub struct BiLockedTx<T> {
//         inner: T,
//     }

//     pub struct BiLockedRx<T> {
//         inner: T,
//     }
// }

// NOTE: blocked on v3 (unframed streams, substrates)

// pub mod batching {
//     use std::any::Any;

//     use serde::{Deserialize, Serialize, de::DeserializeOwned};

//     #[derive(Serialize, Deserialize)]
//     enum BatchingTag<T> {
//         Single(T),
//         Batch(Vec<T>),
//     }

//     pub struct WithBatching<F: Flow> {
//         flow: F,
//         buf: Vec<Box<dyn Any>>,
//     }

//     impl<F: Flow> Flow for WithBatching<F> {
//         type Format = F::Format;

//         async fn recv<V: DeserializeOwned + 'static>(&mut self) -> Result<V, FlowError> {
//             if let Some(previous_batch_unroll) = self.buf.pop() {
//                 return Ok(*previous_batch_unroll.downcast().unwrap());
//             }

//             let tagged: BatchingTag<V> = self.flow.recv().await?;
//             match tagged {
//                 BatchingTag::Single(data) => Ok(data),
//                 BatchingTag::Batch(batch) => {
//                     for x in batch {
//                         self.buf.push(Box::new(x));
//                     }
//                     self.recv().await
//                 }
//             }
//         }

//         async fn send<V: Serialize + 'static>(&mut self, value: V) -> Result<(), FlowError> {
//             self.flow.send(BatchingTag::Single(value)).await
//         }
//     }
// }

// #[cfg(feature = "dyn")]
// pub mod disperse {
//     use crate::{
//         dynamic::{DynChannel, DynChannelExt},
//         ops::split::Split,
//         *,
//     };

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

//     pub struct Builder<T: Split> {
//         inner: Vec<DynChannel<T>>,
//     }

//     impl<T: Channel + Split + DynChannelExt> Builder<T>
//     where
//         C::Tx: 'static,
//         C::Rx: 'static,
//     {
//         fn new() -> Self {
//             Self { inner: Vec::new() }
//         }

//         pub fn add<C>(mut self, channel: C) -> Self
//         where
//             C: Channel + DynChannelExt,
//         {
//             self.inner.push(channel.into_dynamic());
//             self
//         }

//         pub fn finish(self, switch_threshold: u8) -> Dispersed<T> {
//             Dispersed {
//                 round_robin: RoundRobin::new(self.inner, switch_threshold),
//             }
//         }
//     }

//     pub struct Dispersed<T> {
//         round_robin: RoundRobin<DynChannel<T>>,
//     }

//     impl<T> Dispersed<T> {
//         pub fn build() -> Builder<T> {
//             Builder::new()
//         }
//     }

//     impl<T: Channel + Split> Tx for Dispersed<T> {
//         type In = T::In;
//         fn send(&mut self, data: Self::In) -> impl Future<Output = Result<(), Error>> {
//             self.round_robin.access().send(data)
//         }
//     }
// }
