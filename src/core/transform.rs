use std::any::type_name;

use crate::{error::ErrorProvider, *};
use snafu::ResultExt;

pub trait TransformFraming: ErrorProvider {
    type In;
    type Out;
}

// Tx

pub trait TransformTx: TransformFraming {
    fn encode(&mut self, data: Self::In) -> Result<Self::Out, Self::Error>;
}

pub struct TransformedTx<T, I> {
    transform: T,
    inner: I,
}

impl<T, I> Tx for TransformedTx<T, I>
where
    T: TransformTx,
    I: Tx<In = T::Out>,
{
    type In = T::In;

    async fn send(&mut self, data: T::In) -> Result<(), Error> {
        let transformed = self
            .transform
            .encode(data)
            .whatever_context(format!("{} transform error", type_name::<T>()))?;

        Ok(self.inner.send(transformed).await?)
    }
}

// passthrough
impl<T, I> Rx for TransformedTx<T, I>
where
    T: TransformTx,
    I: Rx,
{
    type Out = I::Out;

    fn recv(&mut self) -> impl Future<Output = Result<Self::Out, Error>> {
        self.inner.recv()
    }
}

// Rx

pub trait TransformRx: TransformFraming {
    fn decode(&mut self, data: Self::Out) -> Result<Self::In, Self::Error>;
}

pub struct TransformedRx<T, I> {
    transform: T,
    inner: I,
}

impl<T, I> Rx for TransformedRx<T, I>
where
    T: TransformRx,
    I: Rx<Out = T::Out>,
{
    type Out = T::In;
    async fn recv(&mut self) -> Result<T::In, Error> {
        let data = self.inner.recv().await?;

        let transformed = self
            .transform
            .decode(data)
            .whatever_context(format!("{} transform error", type_name::<T>()))?;

        Ok(transformed)
    }
}

// passthrough
impl<T, I> Tx for TransformedRx<T, I>
where
    T: TransformRx,
    I: Tx,
{
    type In = I::In;

    fn send(&mut self, data: Self::In) -> impl Future<Output = Result<(), Error>> {
        self.inner.send(data)
    }
}

// Ext (interface)

pub trait TransformExt: Sized {
    fn transform<T: TransformTx + TransformRx + Copy>(
        self,
        transform: T,
    ) -> TransformedTx<T, TransformedRx<T, Self>> {
        TransformedTx {
            transform,
            inner: TransformedRx {
                transform,
                inner: self,
            },
        }
    }

    fn transform_tx<T: TransformTx>(self, transform: T) -> TransformedTx<T, Self> {
        TransformedTx {
            transform,
            inner: self,
        }
    }

    fn transform_rx<T: TransformRx>(self, transform: T) -> TransformedRx<T, Self> {
        TransformedRx {
            transform,
            inner: self,
        }
    }
}

impl<T: Channel> TransformExt for T {}

// mod variadic {
//     use std::fmt::Debug;

//     use snafu::Snafu;

//     use crate::error::ErrorProvider;

//     use super::Transform;

//     #[derive(Snafu)]
//     enum CombinedError<A: ErrorProvider, B: ErrorProvider> {
//         A { source: A::Error },
//         B { source: B::Error },
//     }

//     impl<A: ErrorProvider, B: ErrorProvider> Debug for CombinedError<A, B> {
//         fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//             match self {
//                 CombinedError::A { source } => write!(f, "{} error: {}", A::name(), source),
//                 CombinedError::B { source } => write!(f, "{} error: {}", B::name(), source),
//             }
//         }
//     }

//     impl<A: ErrorProvider, B: ErrorProvider> ErrorProvider for (A, B) {
//         type Error = CombinedError<A, B>;
//     }

//     impl<A: Transform, B: Transform<In = A::Out, Out = A::In>> Transform for (A, B) {
//         type In = A::In;
//         type Out = A::Out;

//         fn encode(&mut self, data: Self::In) -> Result<Self::Out, Self::Error> {
//             Ok(self.0.encode(self.1.encode(data)?)?)
//         }
//     }
// }
