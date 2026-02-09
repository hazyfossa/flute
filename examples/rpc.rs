use flute::{define_rpc, rpc::RpcResult, tools::in_memory};
use futures_util::future::join;

// The syntax for defining services is a mix between enum fields and function defintions
//
// Flute derives for you automatically three core primitives:
// - Client interface
// - Server definition boilerplate
// - Request/Response as complete enum types
define_rpc!(Simple {
    Echo {something: String} -> String,
});

// To define a server, implement {Service}Handler trait.
struct Server;
impl SimpleHandler for Server {
    // All RPC functions are fallible by default.
    // RpcResult will convert any error to a string via Display,
    // then send it on the wire. Note that this may drop context of the error.
    //
    // This is expected to change once we finalize our error handling scheme.
    fn echo(&self, something: String) -> RpcResult<String> {
        Ok(something)
    }
}

fn main() {
    // This creates an in-memory channel pair, suitable for in-process communcation
    let (client_channel, server_channel) =
        in_memory::unbounded_pair::<SimpleRequest, SimpleResponse>();

    // Here we execute both sides simultaneously
    // In a real-world application, these two parts can be in two different binaries
    // both of which depend on a "service-definition" crate containing the code above main
    smol::block_on(join(
        // Simply call .serve with an appropriate channel
        // flute will route the requests to your functions based on the Handler trait
        (async || Server.serve(server_channel).await.unwrap())(),
        //
        // Call Service::bind on a channel to connect
        // you then can call remote functions as if they are local!
        (async || {
            let mut client = Simple::bind(client_channel);
            let a = client.echo("Hello".into()).await.unwrap();
            println!("{a}, world!")
        })(),
    ));
}
