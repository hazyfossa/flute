use flute::{define_rpc, rpc::RpcResult, tools::in_memory};

define_rpc!(Simple {
    fn echo(something: String) -> String;

    // RpcResult is a convenient way to specify a function which is fallible.
    // It will format the error via it's Debug impl and send the full string over the wire.
    //
    // You can also use std Result for better efficiency.
    // Just remember to derive(Serialize, Deserialize) on your error.
    fn fallible() -> RpcResult<()>;

    // Note that you must always specify an explicit return type.
    // If your function does not return anything, specify "-> ()"
    //
    // This reflects on the fact that even for empty return bodies,
    // an acknowledgement of completion will be transmitted.
    fn empty() -> ();
});

// To define a server, implement the Handler trait.
struct Server;
impl Simple::Handler for Server {
    // All handler functions are allowed to be async
    async fn echo(&self, something: String) -> String {
        something
    }

    async fn fallible(&self) -> RpcResult<()> {
        Err("This function failed")?
    }

    async fn empty(&self) {
        print!("Doing nothing");
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
    let mut client = flute::rpc::open_channel::<Simple::Service>(client_channel);

    let a = client.echo("Hello".into()).await.unwrap();
    println!("{a}, world!")
}
