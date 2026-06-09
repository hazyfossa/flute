use crate::{Channel, Rx, Tx, error::ErrorProvider, ops::split::Split};

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
        let transformed = self.transform.encode(data).erase_with_provider::<T>()?;

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

        let transformed = self.transform.decode(data).erase_with_provider::<T>()?;

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
    fn name() -> Option<&'static str> {
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
    I: Split + Channel<In = T::After, Out = T::After>,
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

// variadic
use crate::utils::error::EraseResultExt;

impl<A, B> Transform for (A, B)
where
    A: Transform,
    B: Transform<Before = A::After>,
{
    type Before = A::Before;
    type After = B::After;
    fn encode(&mut self, data: Self::Before) -> Result<Self::After, Self::Error> {
        let (a, b) = self;
        let data = a.encode(data).erase_with_provider::<A>()?;
        let data = b.encode(data).erase_with_provider::<B>()?;
        Ok(data)
    }

    fn decode(&mut self, data: Self::After) -> Result<Self::Before, Self::Error> {
        let (a, b) = self;
        // Decodes are in reverse order
        let data = b.decode(data).erase_with_provider::<B>()?;
        let data = a.decode(data).erase_with_provider::<A>()?;
        Ok(data)
    }
}

impl<A, B, C> Transform for (A, B, C)
where
    A: Transform,
    B: Transform<Before = A::After>,
    C: Transform<Before = B::After>,
{
    type Before = A::Before;
    type After = C::After;
    fn encode(&mut self, data: Self::Before) -> Result<Self::After, Self::Error> {
        let (a, b, c) = self;
        let data = a.encode(data).erase_with_provider::<A>()?;
        let data = b.encode(data).erase_with_provider::<B>()?;
        let data = c.encode(data).erase_with_provider::<C>()?;
        Ok(data)
    }

    fn decode(&mut self, data: Self::After) -> Result<Self::Before, Self::Error> {
        let (a, b, c) = self;
        // Decodes are in reverse order
        let data = c.decode(data).erase_with_provider::<C>()?;
        let data = b.decode(data).erase_with_provider::<B>()?;
        let data = a.decode(data).erase_with_provider::<A>()?;
        Ok(data)
    }
}

pub mod map {
    use crate::{Rx, Tx, error::BoxedError};

    // TODO (in order of complexity): MapTx, const generic Map, unification of Map and Transform

    pub struct MapRx<I, F> {
        pub inner: I,
        pub f: F,
    }

    impl<I, F, T, E> Rx for MapRx<I, F>
    where
        I: Rx,
        F: FnMut(I::Out) -> Result<T, E>,
        E: Into<BoxedError>,
    {
        type Out = T;

        async fn recv(&mut self) -> Result<Self::Out, crate::ChannelError> {
            let data = self.inner.recv().await?;
            (self.f)(data)
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
