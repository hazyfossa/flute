pub use crate::{Error, Rx, Tx};

pub mod split {
    // TODO: generic split via BiLock
    use crate::Channel;

    use super::*;

    pub trait Split: Sized + Channel {
        type Tx: Tx<In = Self::In>;
        type Rx: Rx<Out = Self::Out>;
        fn split(self) -> (Self::Tx, Self::Rx);
    }

    impl<A, B> Split for super::merge::Merged<A, B>
    where
        A: Tx,
        B: Rx,
    {
        type Tx = A;
        type Rx = B;

        fn split(self) -> (Self::Tx, Self::Rx) {
            (self.tx, self.rx)
        }
    }
}

pub mod merge {
    use super::*;

    pub struct Merged<Tx, Rx> {
        pub tx: Tx,
        pub rx: Rx,
    }

    impl<T: Tx, U> Tx for Merged<T, U> {
        type In = T::In;

        fn send(&mut self, data: Self::In) -> impl Future<Output = Result<(), Error>> {
            self.tx.send(data)
        }
    }

    impl<T: Rx, U> Rx for Merged<U, T> {
        type Out = T::Out;

        fn recv(&mut self) -> impl Future<Output = Result<Self::Out, Error>> {
            self.rx.recv()
        }
    }

    pub fn merge<A, B>(s: (A, B)) -> Merged<A, B>
    where
        A: Tx,
        B: Rx,
    {
        Merged { tx: s.0, rx: s.1 }
    }
}

pub mod cross {
    use super::{merge::*, split::*};

    #[allow(type_alias_bounds)]
    pub type Crossed<A: Split, B: Split> = Merged<A::Tx, B::Rx>;

    pub fn cross<'a, A, B>(a: A, b: B) -> (Crossed<A, B>, Crossed<B, A>)
    where
        A: Split,
        B: Split,
    {
        let (a_tx, a_rx) = a.split();
        let (b_tx, b_rx) = b.split();

        (merge((a_tx, b_rx)), merge((b_tx, a_rx)))
    }
}
