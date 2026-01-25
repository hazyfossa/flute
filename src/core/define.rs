pub use crate::Error;

pub trait Tx<Wire> {
    async fn send(&mut self, data: Wire) -> Result<(), Error>;
}

pub trait BatchTx<Wire> {
    // TODO: feed API
    async fn send_batch(&mut self, data: Vec<Wire>) -> Result<(), Error>;
}

pub trait Rx<Wire> {
    async fn recv(&mut self) -> Result<Wire, Error>;
}

pub mod split {
    use super::*;

    pub trait Split<Wire> {
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

pub mod merge {
    use super::*;
    use crate::Channel;

    pub struct Merged<Tx, Rx> {
        pub tx: Tx,
        pub rx: Rx,
    }

    impl<Wire, A: Tx<Wire>, B: Rx<Wire>> Channel<Wire> for Merged<A, B> {
        fn send(&mut self, data: Wire) -> impl Future<Output = Result<(), Error>> {
            self.tx.send(data)
        }

        fn recv(&mut self) -> impl Future<Output = Result<Wire, Error>> {
            self.rx.recv()
        }
    }

    pub fn merge<Wire, A, B>(s: (A, B)) -> Merged<A, B>
    where
        A: Tx<Wire>,
        B: Rx<Wire>,
    {
        Merged { tx: s.0, rx: s.1 }
    }
}

pub mod cross {
    use crate::Channel;

    use super::{merge::*, split::*};

    #[allow(type_alias_bounds)]
    pub type Crossed<Wire, A: Split<Wire>, B: Split<Wire>> = Merged<A::Tx, B::Rx>;

    pub fn cross<'a, Wire, A, B>(a: A, b: B) -> (Crossed<Wire, A, B>, Crossed<Wire, B, A>)
    where
        A: Channel<Wire> + Split<Wire>,
        B: Channel<Wire> + Split<Wire>,
    {
        let (a_tx, a_rx) = a.split();
        let (b_tx, b_rx) = b.split();

        (merge((a_tx, b_rx)), merge((b_tx, a_rx)))
    }
}
