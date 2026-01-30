use flute::{define_rpc, tools::in_memory};
use futures_util::future::join;

define_rpc!(Simple {
    Echo {something: String} -> String,
});

struct Server;
impl SimpleHandler for Server {
    fn echo(&self, something: String) -> String {
        something
    }
}

fn main() {
    let (client_channel, server_channel) =
        in_memory::unbounded_pair::<SimpleRequest, SimpleResponse>();

    smol::block_on(join(
        (async || Server.serve(server_channel).await.unwrap())(),
        (async || {
            let mut client = Simple::bind(client_channel);
            let a = client.echo("Hello".into()).await.unwrap();
            println!("{a}, world!")
        })(),
    ));
}
