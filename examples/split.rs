use flute::define_rpc;

define_rpc!(
    #[split_handler]
    pub Service {
        fn echo(something: String) -> String;
        fn complex(input: (u64, u64)) -> u128;
    }
);

struct CommonState {}

impl Service::split_handler::complex for CommonState {
    async fn handle(&self, input: (u64, u64)) -> u128 {
        // Pretend this is a very long function definition
        (input.0 + input.1) as _
    }
}

impl Service::split_handler::echo for CommonState {
    async fn handle(&self, something: String) -> String {
        something
    }
}

fn typecheck(_handler: impl Service::Handler) {}

fn main() {
    // Hints will be provided for what functions are left to implement
    typecheck(CommonState {});
}
