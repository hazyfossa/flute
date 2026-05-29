use snafu::ResultExt;

use crate::{Rx, Tx, Wire, error::ErrorProvider, ops::split::Split};

pub trait Transform: ErrorProvider {
    type Before;
    type After;

    fn encode(&mut self, before: Self::Before) -> Result<Self::After, Self::Error>;
    fn decode(&mut self, after: Self::After) -> Result<Self::Before, Self::Error>;
}

pub struct Transformed<I, T> {
    inner: I,
    transform: T,
}

impl<I, T> Tx for Transformed<I, T>
where
    I: Tx,
    T: Transform<After = I::In>,
{
    type In = T::Before;

    async fn send(&mut self, data: Self::In) -> Result<(), crate::ChannelError> {
        let transformed = self
            .transform
            .encode(data)
            .whatever_context(format!("{} transform error", T::name()))?;

        self.inner.send(transformed).await
    }
}

impl<I, T> Rx for Transformed<I, T>
where
    I: Rx,
    T: Transform<After = I::Out>,
{
    type Out = T::Before;

    async fn recv(&mut self) -> Result<Self::Out, crate::ChannelError> {
        let data = self.inner.recv().await?;

        let transformed = self
            .transform
            .decode(data)
            .whatever_context(format!("{} transform error", T::name()))?;

        Ok(transformed)
    }
}

// inverse
// TODO: consider if this is useful
pub struct Inverse<T>(T);

impl<T> ErrorProvider for Inverse<T>
where
    T: ErrorProvider,
{
    type Error = T::Error;
    fn name() -> &'static str {
        T::name()
    }
}

impl<T> Stateless for Inverse<T>
where
    T: Stateless,
{
    fn stateless() -> Self {
        Self(T::stateless())
    }
}

impl<T> Transform for Inverse<T>
where
    T: Transform,
{
    type Before = T::After;
    type After = T::Before;

    fn encode(&mut self, before: Self::Before) -> Result<Self::After, Self::Error> {
        Ok(self.0.decode(before)?)
    }

    fn decode(&mut self, after: Self::After) -> Result<Self::Before, Self::Error> {
        Ok(self.0.encode(after)?)
    }
}

// stateless transform optimizations

// TODO: ponder
pub trait Stateless: Sized {
    fn stateless() -> Self {
        const { assert!(core::mem::size_of::<Self>() == 0) }

        // SAFETY: The `assert!` above guarantees `T` is zero-sized.
        unsafe { core::mem::zeroed() }
    }
}

impl<I, T> Transformed<I, T> {
    pub fn new(inner: I) -> Self
    where
        T: Stateless,
    {
        Self {
            inner,
            transform: T::stateless(),
        }
    }

    pub fn new_with_state(inner: I, state: T) -> Self {
        Self {
            inner,
            transform: state,
        }
    }
}

// Common case optimization:
// splitting a transformed wire is cheap if there is no state to synchronize
impl<I, T> Split for Transformed<I, T>
where
    I: Split + Wire<T::After>,
    T: Transform + Stateless,
{
    type Rx = Transformed<I::Rx, T>;
    type Tx = Transformed<I::Tx, T>;

    fn split(self) -> (Self::Tx, Self::Rx) {
        let (tx, rx) = self.inner.split();

        let tx = Transformed::<I::Tx, T>::new(tx);
        let rx = Transformed::<I::Rx, T>::new(rx);

        (tx, rx)
    }
}

pub mod map {
    use snafu::ResultExt;

    use crate::{Rx, Tx, error::BoxedErr};

    // TODO (in order of complexity): MapTx, const generic Map, unification of Map and Transform

    pub struct MapRx<I, F> {
        pub inner: I,
        pub f: F,
    }

    impl<I, F, T, E> Rx for MapRx<I, F>
    where
        I: Rx,
        F: FnMut(I::Out) -> Result<T, E>,
        E: Into<BoxedErr>,
    {
        type Out = T;

        async fn recv(&mut self) -> Result<Self::Out, crate::ChannelError> {
            let data = self.inner.recv().await?;
            (self.f)(data).whatever_context("map error")
        }
    }

    impl<I, Any> Tx for MapRx<I, Any>
    where
        I: Tx,
    {
        type In = I::In;

        async fn send(&mut self, data: Self::In) -> Result<(), crate::ChannelError> {
            self.inner.send(data).await
        }
    }
}
