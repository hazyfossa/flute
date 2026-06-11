// This is an example showcasing a REST-like API with flute and axum.
// The same approach should be applicable to many other http frameworks.
//
// If client-side is written in wasm, use
/// ```
/// let fetch = flute::compat::wasm::FetchJson::new(url);
/// let client = YourService::client(fetch)
/// ```
// to achieve similar ergonomics to leptos/dioxus server functions.
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

// The following is a generic example of a flute service

flute::define_rpc!(KV {
    fn get(key: String) -> Option<String>;
    fn set(key: String, value: String) -> ();
});

#[derive(Clone)]
struct WebHashMap {
    // In a real-world application you should use scc or dashmap instead
    map: Arc<Mutex<HashMap<String, String>>>,
}

impl KV::Handler for WebHashMap {
    async fn get(&self, key: String) -> Option<String> {
        let map = self.map.lock().unwrap();
        map.get(&key).cloned()
    }

    async fn set(&self, key: String, value: String) {
        let mut map = self.map.lock().unwrap();
        map.insert(key, value);
    }
}

// This is how you would typically define an api handler
// Note that flute does nothing axum-specific, so the same
// approach could be used with other frameworks
async fn api(
    service: extract::State<WebHashMap>,
    request: extract::Json<KV::Request>,
) -> Json<KV::Response> {
    // connect the handler functions as service routes
    let service = KV::Route(service.0);

    // use the handler manually
    use flute::rpc::Handler;

    service.handle(request.0).await.into()
}

#[tokio::main]
async fn main() {
    let state = WebHashMap {
        map: Arc::new(Mutex::new(HashMap::new())),
    };

    let router = Router::new().route("api", get(api)).with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
