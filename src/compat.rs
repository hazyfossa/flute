#[cfg(feature = "futures")]
pub mod futures {
    use std::marker::PhantomData;

    use futures_util::{
        Sink, SinkExt, Stream, StreamExt,
        stream::{SplitSink, SplitStream},
    };
    use snafu::ResultExt;

    use crate::{ChannelError, Rx, Tx, error::BoxedErr, ops::split};

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
        T::Error: Into<BoxedErr>,
    {
        type In = Wire;
        async fn send(&mut self, data: Wire) -> Result<(), ChannelError> {
            Ok(self.inner.send(data).await.whatever_context("sink error")?)
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
        T::Error: Into<BoxedErr>,
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
        merge::merge(kanal::unbounded_async())
    }

    pub fn bounded<T>(size: usize) -> KanalWire<T> {
        merge::merge(kanal::bounded_async(size))
    }
}

#[cfg(feature = "wasm")]
pub mod wasm {
    use std::any::type_name_of_val;

    use gloo_net::{
        http::{Method, RequestBuilder},
        websocket::{self, futures::WebSocket as WebSocketRaw},
    };
    use snafu::{ResultExt, Snafu, ensure};

    use crate::{
        Wire,
        compat::futures::adapt,
        error::ErrorProvider,
        rpc::{Caller, Service},
        transform::Transform,
    };

    // Fetch

    #[derive(Debug, Snafu)]
    pub enum FetchError {
        #[snafu(context(false))]
        SendError { source: gloo_net::Error },
        #[snafu(context(false))]
        DataFormatError { source: serde_json::Error },
        #[snafu(display("cannot parse response body as string"))]
        BodyParseError { source: gloo_net::Error },
        #[snafu(display("[{status}]: {body}"))]
        HttpApiError { status: u16, body: String },
    }

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
        type Error = FetchError;
    }

    impl<S: Service> Caller<S> for FetchJson {
        async fn call(&mut self, request: S::Request) -> Result<S::Response, Self::Error> {
            let body = serde_json::to_string(&request)?;

            let ret = RequestBuilder::new(&self.url)
                .method(self.method.clone())
                .body(body)?
                .send()
                .await?;

            let body = ret.text().await.context(BodyParseSnafu)?;

            ensure!(
                ret.ok(),
                HttpApiSnafu {
                    status: ret.status(),
                    body,
                }
            );

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

    #[derive(Debug, Snafu)]
    #[snafu(display("invalid type of websocket message: expected {expected}, got {got}"))]
    pub struct WebsocketSelectError {
        expected: &'static str,
        got: &'static str,
    }

    macro_rules! ws_select_impl {
        ($vis:vis $name:ident ($ty:ty => $selector:ident)) => {
            $vis struct $name;

            impl ErrorProvider for $name {
                type Error = WebsocketSelectError;
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
                        ref other => Err(WebsocketSelectError {
                            expected: stringify!($ty),
                            got: type_name_of_val(other),
                        }),
                    }
                }
            }
        };
    }

    ws_select_impl!(pub WebsocketSelectString (String => Text) );
    ws_select_impl!(pub WebsocketSelectBytes ( Vec<u8> => Bytes) );
}
