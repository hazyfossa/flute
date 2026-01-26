use flute::{define_rpc, flow::UseFlow, tools::in_memory};

define_rpc!(SimpleService {
    Echo {something: String} -> String,
    TryEcho { maybe_string: Vec<u8> } -> RpcResult<String>
});

struct Server;
impl SimpleServiceHandler for Server {
    fn echo(&self, something: String) -> String {
        something
    }

    fn try_echo(&self, maybe_string: Vec<u8>) -> RpcResult<String> {
        let string = String::try_from(maybe_string)?;
        Ok(string)
    }
}

fn main() {
    let (client, server_channel) = in_memory::unbounded_pair::<Vec<u8>>();
    Server.serve(server_channel.with_data_format());
}
