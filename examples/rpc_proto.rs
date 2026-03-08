// This example is incomplete and showcases a workflow I really want to change

use flute::{
    Channel, Wire,
    compat::{json::json, kanal},
    define_rpc,
    transform::TransformExt,
};

define_rpc!(Simple {
    fn echo(something: String) -> String;
});

pub async fn setup_server(
    channel: impl Wire<Vec<u8>>,
) -> impl Channel<In = Simple::Request, Out = Simple::Response> {
    channel.transform_tx(json()).transform_rx(json())
}

pub async fn setup_client(
    channel: impl Wire<Vec<u8>>,
) -> impl Channel<In = Simple::Response, Out = Simple::Request> {
    channel.transform_rx(json()).transform_tx(json())
}

fn main() {
    // This is purely for demonstation purposes
    // In practice, `wire` will most likely wrap a channel over a network
    // If your RPC can use in-process communication, use `flute::tools::in_memory` instead
    // See examples/rpc for reference
    let _opaque_wire = kanal::unbounded::<Vec<u8>>();
    todo!()
}
