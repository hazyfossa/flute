#[cfg(feature = "futures")]
pub mod futures {
    use std::marker::PhantomData;

    use futures_util::{
        Sink, SinkExt, Stream, StreamExt,
        stream::{SplitSink, SplitStream},
    };

    use crate::{ChannelError, Rx, Tx, ops::split};

    pub struct Adapter<T, Wire> {
        inner: T,
        _wire: PhantomData<Wire>,
    }

    pub fn adapt<Fut, Wire>(future: Fut) -> Adapter<Fut, Wire> {
        Adapter {
            inner: future,
            _wire: PhantomData,
        }
    }

    impl<Wire, T: Sink<Wire> + Unpin> Tx for Adapter<T, Wire>
    where
        T::Error: Into<eyre::Error>,
    {
        type In = Wire;
        async fn send(&mut self, data: Wire) -> Result<(), ChannelError> {
            Ok(self.inner.send(data).await?)
        }
    }

    impl<T: Stream + Unpin, Wire> Rx for Adapter<T, Wire> {
        type Out = T::Item;

        async fn recv(&mut self) -> Result<T::Item, ChannelError> {
            match self.inner.next().await {
                Some(data) => Ok(data),
                None => Err(ChannelError::Closed),
            }
        }
    }

    impl<Wire, T> split::Split for Adapter<T, Wire>
    where
        T: Stream<Item = Wire> + Sink<Wire> + Unpin,
        T::Error: Into<eyre::Error>,
    {
        type Rx = Adapter<SplitStream<T>, Wire>;
        type Tx = Adapter<SplitSink<T, Wire>, Wire>;

        fn split(self) -> (Self::Tx, Self::Rx) {
            let (tx, rx) = self.inner.split();
            (adapt(tx), adapt(rx))
        }
    }
}

#[cfg(feature = "kanal")]
pub mod kanal {
    use kanal::{AsyncReceiver, AsyncSender};

    use crate::{ChannelError, Rx, Tx, ops::merge};

    impl<T> Tx for AsyncSender<T> {
        type In = T;

        async fn send(&mut self, data: T) -> Result<(), ChannelError> {
            AsyncSender::send(self, data)
                .await
                .map_err(|_| ChannelError::Closed)
        }
    }

    impl<T> Rx for AsyncReceiver<T> {
        type Out = T;

        async fn recv(&mut self) -> Result<T, ChannelError> {
            AsyncReceiver::recv(self)
                .await
                .map_err(|_| ChannelError::Closed)
        }
    }

    pub type KanalWire<T> = merge::Merged<AsyncSender<T>, AsyncReceiver<T>>;

    pub fn unbounded<T>() -> KanalWire<T> {
        let (tx, rx) = kanal::unbounded_async();
        merge::merge(tx, rx)
    }

    pub fn bounded<T>(size: usize) -> KanalWire<T> {
        let (tx, rx) = kanal::bounded_async(size);
        merge::merge(tx, rx)
    }
}

#[cfg(feature = "tower")]
pub mod tower {
    use std::{
        marker::PhantomData,
        pin::Pin,
        task::{Context, Poll},
    };

    use crate::error::AsResult;

    pub struct Adapt<T, S>(T, PhantomData<S>);
    pub fn adapt<T, S>(handler: T) -> Adapt<T, S>
    where
        S: crate::rpc::Service,
        T: crate::rpc::Handler<S>,
    {
        Adapt(handler, PhantomData)
    }

    impl<T, S> tower_service::Service<S::Request> for Adapt<T, S>
    where
        S: crate::rpc::Service,
        S::Request: 'static,
        S::Response: AsResult,
        T: crate::rpc::Handler<S> + Clone + 'static,
    {
        type Future = Pin<Box<dyn Future<Output = Result<S::Response, Self::Error>>>>;
        type Response = S::Response;
        type Error = eyre::Error;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: S::Request) -> Self::Future {
            let handler = self.0.clone();
            Box::pin(async move { handler.handle(req).await.into() })
        }
    }
}

#[cfg(feature = "wasm")]
pub mod wasm {
    use std::any::type_name_of_val;

    use eyre::{Context, ensure};
    use gloo_net::{
        http::{Method, RequestBuilder},
        websocket::{self, futures::WebSocket as WebSocketRaw},
    };

    use crate::{
        Wire,
        compat::futures::adapt,
        error::ErrorProvider,
        rpc::{Caller, Service},
        transform::Transform,
    };

    // Fetch

    pub struct FetchJson {
        url: String,
        method: Method,
    }

    impl FetchJson {
        pub fn new(url: String) -> Self {
            Self {
                url,
                method: Method::GET,
            }
        }

        pub fn new_with_method(url: String, method: Method) -> Self {
            Self { url, method }
        }
    }

    impl ErrorProvider for FetchJson {
        type Error = eyre::Error;
    }

    impl<S: Service> Caller<S> for FetchJson {
        async fn call(&mut self, request: S::Request) -> Result<S::Response, Self::Error> {
            let body = serde_json::to_string(&request)?;

            let ret = RequestBuilder::new(&self.url)
                .method(self.method.clone())
                .body(body)?
                .send()
                .await?;

            let body = ret
                .text()
                .await
                .context("cannot parse response body as string")?;

            ensure!(ret.ok(), "[{}] {:?}", ret.status(), ret.body());

            let response = serde_json::from_str(&body)?;
            Ok(response)
        }
    }

    // WebSocket

    fn websocket_wrap(raw: WebSocketRaw) -> impl Wire {
        let wire = adapt(raw);
        // TODO: better syntax (intentionally delayed)
        crate::transform::map::MapRx {
            inner: wire,
            f: |r: Result<websocket::Message, _>| r.into(),
        }
    }

    pub fn websocket_open(url: &str) -> Result<impl Wire, gloo_net::Error> {
        // NOTE: i really don't know why gloo doesn't do this error map internally
        let js_bind = WebSocketRaw::open(url).map_err(|e| gloo_net::Error::JsError(e))?;
        Ok(websocket_wrap(js_bind))
    }

    pub fn websocket_open_with_protocol(
        url: &str,
        protocol: &str,
    ) -> Result<impl Wire, gloo_net::Error> {
        let js_bind = WebSocketRaw::open_with_protocol(url, protocol)
            .map_err(|e| gloo_net::Error::JsError(e))?;

        Ok(websocket_wrap(js_bind))
    }

    macro_rules! ws_select_impl {
        ($vis:vis $name:ident ($ty:ty => $selector:ident)) => {
            $vis struct $name;

            impl ErrorProvider for $name {
                type Error = eyre::Error;
            }

            impl Transform for $name {
                type Before = websocket::Message;
                type After = $ty;

                fn decode(&mut self, data: Self::After) -> Result<Self::Before, Self::Error> {
                    Ok(websocket::Message::$selector(data))
                }

                fn encode(&mut self, data: Self::Before) -> Result<Self::After, Self::Error> {
                    match data {
                        websocket::Message::$selector(correct) => Ok(correct),
                        ref other => eyre::bail!("invalid type of websocket message: expected {}, got {}",
                            /*expected*/ stringify!($ty),
                            /*got*/ type_name_of_val(other),
                        ),
                    }
                }
            }
        };
    }

    ws_select_impl!(pub WebsocketSelectString (String => Text) );
    ws_select_impl!(pub WebsocketSelectBytes ( Vec<u8> => Bytes) );
}
