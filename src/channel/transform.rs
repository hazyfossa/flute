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

// variadics
use crate::utils::error::EraseResultExt;

macro_rules! var_impl {
    (#[$($t:ident)*] { $($bounds:tt)* }) => {
        impl< $($t),* > Transform for ( $($t),* )
        where $($bounds)*
        {
            type Before = var_impl!(@before $($t),*);
            type After =  var_impl!(@after $($t),*);

            fn encode(&mut self, data: Self::Before) -> Result<Self::After, Self::Error> {
                let ( $($t),* ) = self;
                var_impl!(@encode_chain {data} $($t),*);
                Ok(data)
            }

            fn decode(&mut self, data: Self::After) -> Result<Self::Before, Self::Error> {
                let ( $($t),* ) = self;
                var_impl!(@decode_chain {data} $($t),*);
                Ok(data)
            }
        }
    };

    // @bounds
    // TODO: currently unused, as macros cannot expand inside where

    // When no ident passed as previous, treat the first separately, then forward
    (@bounds /* [ret=None] (previous=None) */ $first:ident, $($next:tt),*) => {
        var_impl!(
            @bounds
            /* ret = */ [$first: Transform,]
            /* previous = */ ($first)
            $($next),*
        )

    };

    // Munch with a rolling context (previous -> current)
    (@bounds [$($ret:tt)*] ($previous:ident) $current:ident, $($next:tt),*) => {
        var_impl!(@bounds
            /* ret = */ [
                $($ret)*
                $current: Transform<Before = $previous::After>,
            ]
            /* previous = */ ($current)
            $($next),*
        )
    };

    (@bounds [$($ret:tt)*] ($last:ident)) => { [$($ret)*] };

    // @before

    (@before $first:ident, $($next:tt),*) => {
        $first::Before
    };

    // @after

    // If have more than one token, discard and forward
    (@after $current:tt, $($next:tt),*) => {
        var_impl!(@after $($next),*)
    };

    // Match last token
    (@after $last:ident) => {
        $last::After
    };

    // @encode_chain

    (@encode_chain {$data:ident} $($t:ident),*) => {
        $(
            let $data = $t.encode($data).erase_with_provider::<$t>()?;
        )*
    };

    // @decode_chain

    // The following code is best read backwards
    // It actually makes sense, because decodes are in reverse order of encodes :)

    // 3. Return $ret once passed all identifiers
    (@decode_chain [$($ret:tt)*] {$data:ident}) => {
        $($ret)*
    };

    // 2. Keeping a return value $ret, for each next ident
    // prepend a line to the beginning of the return value
    // effectively building $ret in reverse order
    (@decode_chain [$($ret:tt)*] {$data:ident} $head:ident $($tail:ident)*) => {
        var_impl!(@decode_chain [
            let $data = $head.decode($data).erase_with_provider::<$head>()?;
            $($ret)*
        ] {$data} $($tail)*);
    };

    // 1. Initialize the macro with $ret as an empty value
    (@decode_chain {$data:ident} $($t:ident),*) => {
        var_impl!(@decode_chain [] {$data} $($t)*);
    };
}

#[allow(non_snake_case)]
mod variadics {
    use super::*;
    var_impl!(#[A B] {A: Transform, B: Transform<Before = A::After>});
    var_impl!(#[A B C] {A: Transform, B: Transform<Before = A::After>, C: Transform<Before = B::After>});
    var_impl!(#[A B C D] {A: Transform, B: Transform<Before = A::After>, C: Transform<Before = B::After>, D: Transform<Before = C::After>});
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
