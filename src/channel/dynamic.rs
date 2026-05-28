#![allow(dead_code)]
// TODO: v3 should allow more efficient dispatch
// on the r/w level, which hopefully reduces boxing

use std::pin::Pin;

use crate::{Channel, ChannelError, Rx, Tx, ops::split::Split};

trait BoxedTx {
    type In;
    fn send_dyn<'life0, 'a>(
        &'life0 mut self,
        data: Self::In,
    ) -> Pin<Box<dyn Future<Output = Result<(), ChannelError>> + 'a>>
    where
        'life0: 'a,
        Self: 'a;
}

impl<T: Tx> BoxedTx for T {
    type In = T::In;
    fn send_dyn<'life0, 'a>(
        &'life0 mut self,
        data: T::In,
    ) -> Pin<Box<dyn Future<Output = Result<(), ChannelError>> + 'a>>
    where
        'life0: 'a,
        Self: 'a,
    {
        Box::pin(self.send(data))
    }
}

trait BoxedRx {
    type Out;
    fn recv_dyn<'life0, 'a>(
        &'life0 mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Out, ChannelError>> + 'a>>
    where
        'life0: 'a,
        Self: 'a;
}

impl<T: Rx> BoxedRx for T {
    type Out = T::Out;
    fn recv_dyn<'life0, 'a>(
        &'life0 mut self,
    ) -> Pin<Box<dyn Future<Output = Result<T::Out, ChannelError>> + 'a>>
    where
        'life0: 'a,
        Self: 'a,
    {
        Box::pin(self.recv())
    }
}

pub struct DynChannel<In, Out> {
    tx: Box<dyn BoxedTx<In = In>>,
    rx: Box<dyn BoxedRx<Out = Out>>,
}

pub trait DynChannelExt: Channel + Split
where
    Self::Tx: 'static,
    Self::Rx: 'static,
{
    fn into_dynamic(self) -> DynChannel<Self::In, Self::Out> {
        let (tx, rx) = self.split();
        DynChannel {
            tx: Box::new(tx),
            rx: Box::new(rx),
        }
    }
}

impl<T: Channel + Split> DynChannelExt for T
where
    Self::Tx: 'static,
    Self::Rx: 'static,
{
}
