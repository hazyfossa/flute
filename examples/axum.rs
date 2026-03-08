// This is an example showcasing a REST-like API with flute and axum.
// The same approach should be applicable to many other http frameworks.
//
// Note that this is not truly REST, since it uses flute's in-band errors
// and schema instead of http status codes and OpenAPI.
//
// Here, flute channels are not used, instead, all networking is delegated
// to the framework for seamless integration. For maximum performance,
// you can also serve flute natively, over channels backed by hyper.

// TODO: example of flute channels over hyper

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{Json, Router, extract, routing::get};
use flute::rpc::RpcResult;

use crate::Service::Handler;

flute::define_rpc!(Service {
    fn get(key: String) -> Option<String>,
    fn set(key: String, value: String) -> (),
});

#[derive(Clone)]
struct KVHandler {
    // In a real-world application you should use scc or dashmap instead
    map: Arc<Mutex<HashMap<String, String>>>,
}

impl KVHandler {
    fn new() -> Self {
        Self {
            map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Service::Handler for KVHandler {
    fn get(&self, key: String) -> RpcResult<Option<String>> {
        let map = self.map.lock().unwrap();
        Ok(map.get(&key).cloned())
    }

    fn set(&self, key: String, value: String) -> RpcResult<()> {
        let mut map = self.map.lock().unwrap();
        map.insert(key, value);
        Ok(())
    }
}

// This is how you would typically define an api handler
// Note that flute does nothing axum-specific, so the same
// approach could be used with other frameworks
async fn api(
    handler: extract::State<KVHandler>,
    request: extract::Json<Service::Request>,
) -> Json<Service::Response> {
    handler.handle(request.0).into()
}

#[tokio::main]
async fn main() {
    let router = Router::new()
        .route("api", get(api))
        .with_state(KVHandler::new());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
