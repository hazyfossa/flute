use crate::primitives::*;

pub mod merge {
    use super::*;

    pub struct Merged<Tx, Rx> {
        pub tx: Tx,
        pub rx: Rx,
    }

    impl<Wire, A: Tx<Wire>, B> Tx<Wire> for Merged<A, B> {
        fn send(&mut self, data: Wire) -> impl Future<Output = Result<(), Error>> {
            self.tx.send(data)
        }
    }

    impl<Wire, A, B: Rx<Wire>> Rx<Wire> for Merged<A, B> {
        fn recv(&mut self) -> impl Future<Output = Result<Wire, Error>> {
            self.rx.recv()
        }
    }

    pub fn merge_remap<Wire, A, B>(tx: A, rx: B) -> Merged<A, B>
    where
        A: Tx<Wire>,
        B: Rx<Wire>,
    {
        Merged { tx, rx }
    }

    pub fn merge<Wire, A, B>(s: (A, B)) -> Merged<A, B>
    where
        A: Tx<Wire>,
        B: Rx<Wire>,
    {
        merge_remap(s.0, s.1)
    }
}

pub mod split {
    use super::*;

    pub trait Split<Wire>: Channel<Wire> {
        type Tx: Tx<Wire>;
        type Rx: Rx<Wire>;
        fn split(self) -> (Self::Tx, Self::Rx);
    }

    impl<Wire, A, B> Split<Wire> for super::merge::Merged<A, B>
    where
        A: Tx<Wire>,
        B: Rx<Wire>,
    {
        type Tx = A;
        type Rx = B;

        fn split(self) -> (Self::Tx, Self::Rx) {
            (self.tx, self.rx)
        }
    }
}

pub mod add {
    use super::{merge::*, split::*};

    pub trait AddReceiver<Wire>: Split<Wire> {
        fn add_receiver(self) -> (Merged<Self::Tx, Self::Rx>, Self::Rx);
    }

    impl<Wire, T: Split<Wire>> AddReceiver<Wire> for T
    where
        T::Rx: Clone,
    {
        fn add_receiver(self) -> (Merged<Self::Tx, Self::Rx>, Self::Rx) {
            let (tx, rx) = self.split();
            let new_rx = rx.clone();
            (merge((tx, rx)), new_rx)
        }
    }

    pub trait AddSender<Wire>: Split<Wire> {
        fn add_sender(self) -> (Merged<Self::Tx, Self::Rx>, Self::Tx);
    }

    impl<Wire, T: Split<Wire>> AddSender<Wire> for T
    where
        T::Tx: Clone,
    {
        fn add_sender(self) -> (Merged<Self::Tx, Self::Rx>, Self::Tx) {
            let (tx, rx) = self.split();
            let new_tx = tx.clone();
            (merge((tx, rx)), new_tx)
        }
    }
}

pub mod cross {
    use super::{merge::*, split::*, *};

    #[allow(type_alias_bounds)]
    pub type Crossed<Wire, A: Split<Wire>, B: Split<Wire>> = Merged<A::Tx, B::Rx>;

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
