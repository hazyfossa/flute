use flute::{define_rpc, rpc::RpcResult, tools::in_memory};

// The syntax for defining services is similar to traits
//
// Flute derives for you automatically three core primitives:
// - Client interface
// - Server definition boilerplate
// - Request/Response as complete enum types
define_rpc!(Simple {
    fn echo(something: String) -> String,
});

// To define a server, implement the Handler trait.
struct Server;
impl Simple::Handler for Server {
    // All RPC functions are fallible by default.
    // RpcResult will convert any error to a string via Display, then send it on the wire.
    // Note that this may drop context of the error.
    //
    // This is expected to change once we finalize our error handling scheme.
    fn echo(&self, something: String) -> RpcResult<String> {
        Ok(something)
    }
}

fn main() {
    // This creates an in-memory channel pair, suitable for in-process communcation
    let (client_channel, server_channel) =
        in_memory::unbounded_pair::<Simple::Request, Simple::Response>();

    // To create a server, simply call ::server with a handler and an appropriate channel
    let server = Simple::server(Server, server_channel);

    // Server is just a future that enters a loop.
    // You can make it a background task, join with other futures, etc.
    // Flute is in no way dependent on smol, you can use any other runtime!
    smol::spawn(server).detach();

    // Here we execute both sides simultaneously
    // In a real-world application, these two parts can be in two different binaries
    // both of which depend on a "service-definition" crate containing the define_rpc!
    smol::block_on(async {
        let mut client = Simple::client(client_channel);
        let a = client.echo("Hello".into()).await.unwrap();
        println!("{a}, world!")
    });
}
