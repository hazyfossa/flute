use flute::{define_rpc, rpc::RpcResult, tools::in_memory};

// The syntax for defining services is similar to traits
//
// Flute derives for you automatically three core primitives:
// - Client interface
// - Server definition boilerplate
// - Request/Response as complete enum types
define_rpc!(Simple {
    fn echo(something: String) -> String;

    // Note that you must always specify an explicit return type.
    // If your function does not return anything, specify "-> ()"
    //
    // This reflects on the fact that even for empty return bodies,
    // an acknowledgement and (optionally) an in-band error will be transmitted.
    fn empty() -> ();
});

// To define a server, implement the Handler trait.
struct Server;
impl Simple::Handler for Server {
    // All RPC functions are fallible by default.
    // This means that a handler always has an option to return an error.
    // You should always return an error instead of panicking or unwrapping (where possible).
    //
    // RpcResult will convert any error to a string via Debug, then send it on the wire.
    //
    // This is expected to change once we finalize our error handling scheme.
    async fn echo(&self, something: String) -> RpcResult<String> {
        Ok(something)
    }

    async fn empty(&self) -> RpcResult<()> {
        // The error will still be transmitted
        Err("this function is empty")?
    }
}

#[tokio::main]
async fn main() {
    // This creates an in-memory channel pair, suitable for in-process communcation
    let (client_channel, server_channel) =
        in_memory::unbounded_pair::<Simple::Request, Simple::Response>();

    // This wrapper automatically connects the handler functions to the service definition
    let service = Simple::Route(Server);

    // To create a server, simply call serve() with an instance and an appropriate channel
    let server = flute::tools::server::serve(service, server_channel);

    // Server is just a future that enters a loop.
    // You can make it a background task, join with other futures, etc.
    // And flute is in no way dependent on tokio, you can use any other runtime!
    let _ = tokio::spawn(server).await;

    // Here we execute both sides simultaneously
    // In a real-world application, these two parts can be in two different binaries
    // both of which depend on a "service-definition" crate containing the define_rpc!
    let mut client = Simple::Client::from(client_channel);
    let a = client.echo("Hello".into()).await.unwrap();
    println!("{a}, world!")
}
