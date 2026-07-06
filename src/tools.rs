pub mod server {

    use crate::{
        Channel, ChannelError,
        rpc::{Caller, Service},
    };

    // #[cfg(feature = "fast-server")]
    // use crate::{Rx, Tx, ops::split::Split};

    pub async fn serve<S: Service>(
        mut handler: impl Caller<S>,
        mut channel: impl Channel<In = S::Response, Out = S::Request>,
    ) -> Result<(), crate::ChannelError> {
        loop {
            let request = match channel.recv().await {
                Ok(req) => req,
                Err(ChannelError::Closed) => break Ok(()),
                Err(e) => return Err(e),
            };

            let response = handler.call(request).await?;
            channel.send(response).await?;
        }
    }

    // TODO: defer: blocked on flux
    // #[cfg(feature = "fast-server")]
    // pub async fn serve_fast<S: Service, H, C>(
    //     handler: H,
    //     channel: C,
    //     max_concurrent: usize,
    // ) -> Result<(), crate::ChannelError>
    // where
    //     C: Channel<In = S::Response, Out = S::Request> + Split,
    //     C::Tx: Clone + 'static,
    //     H: Caller<S> + 'static,
    //     S::Request: 'static,
    //     S::Response: 'static,
    // {
    //     use std::sync::Arc;
    //     use tokio::task::JoinSet;

    //     let handler = Arc::new(handler);
    //     let (tx, mut rx) = channel.split();
    //     let mut tasks = JoinSet::new();

    //     fn handle_task_result(
    //         res: Result<Result<(), crate::ChannelError>, tokio::task::JoinError>,
    //     ) -> Result<(), crate::ChannelError> {
    //         match res {
    //             Ok(Ok(())) => Ok(()),
    //             Ok(Err(e)) => Err(e),
    //             Err(join_err) => std::panic::resume_unwind(join_err.into_panic()),
    //         }
    //     }

    //     // handler future should never leave thread (~!Send)
    //     // if handler wants parallelism, this should be explicit (internal)
    //     // this also alleviates Arc, spawn_local and other nonsense
    //     let task = |request| {
    //         let handler = Arc::clone(&handler);
    //         let mut tx = tx.clone();
    //         async move {
    //             let response = handler.call(request).await?; // TODO: handle call failure
    //             tx.send(response).await
    //         }
    //     };

    //     loop {
    //         // If over capacity, only poll existing
    //         if tasks.len() >= max_concurrent {
    //             if let Some(res) = tasks.join_next().await {
    //                 handle_task_result(res)?;
    //             }
    //             continue;
    //         }

    //         tokio::select! {
    //             // Accept new
    //             req = rx.recv() => match req {
    //                 Ok(request) => { tasks.spawn_local(task(request)); },
    //                 Err(crate::ChannelError::Closed) => break,
    //                 Err(e) => return Err(e),
    //             },
    //             // Poll existing
    //             task_res = tasks.join_next(), if !tasks.is_empty() => {
    //                 // the unwrap is guarded by is_empty check above
    //                 handle_task_result(task_res.unwrap())?;
    //             }
    //         }
    //     }

    //     while let Some(res) = tasks.join_next().await {
    //         handle_task_result(res)?;
    //     }

    //     Ok(())
    // }
}

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

#[cfg(feature = "unstable-split-any")]
pub mod split_any {
    use futures_util::lock::BiLock;

    use crate::{Channel, Rx, Tx, ops::split::Split};

    pub struct BiLockedTx<T>(BiLock<T>);

    impl<T: Tx + Unpin> Tx for BiLockedTx<T> {
        type In = T::In;
        async fn send(&mut self, data: Self::In) -> Result<(), crate::ChannelError> {
            self.0.lock().await.as_pin_mut().get_mut().send(data).await
        }
    }

    pub struct BiLockedRx<T>(BiLock<T>);

    impl<T: Rx + Unpin> Rx for BiLockedRx<T> {
        type Out = T::Out;

        async fn recv(&mut self) -> Result<Self::Out, crate::ChannelError> {
            self.0.lock().await.as_pin_mut().get_mut().recv().await
        }
    }

    pub struct BiLocked<T>(pub T);

    impl<T: Tx> Tx for BiLocked<T> {
        type In = T::In;

        fn send(
            &mut self,
            data: Self::In,
        ) -> impl Future<Output = Result<(), crate::ChannelError>> {
            self.0.send(data)
        }
    }

    impl<T: Rx> Rx for BiLocked<T> {
        type Out = T::Out;

        fn recv(&mut self) -> impl Future<Output = Result<Self::Out, crate::ChannelError>> {
            self.0.recv()
        }
    }

    impl<T: Channel + Unpin> Split for BiLocked<T> {
        type Tx = BiLockedTx<T>;
        type Rx = BiLockedRx<T>;

        fn split(self) -> (Self::Tx, Self::Rx) {
            let (tx, rx) = BiLock::new(self.0);
            (BiLockedTx(tx), BiLockedRx(rx))
        }
    }
}
