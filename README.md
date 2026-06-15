# flute
Flute is a utility crate for defining bi-directional channels and RPC interfaces. It is non-opinionated, focusing only on boilerplate code generation, leaving full control over the protocol.

# getting started
Defining a service is very easy:

```rust
define_rpc!(Simple {
    fn echo(something: String) -> String;
})
```

Services are traits with some restrictions. These restrictions allow us to call them remotely.

The `define_rpc!` macro automatically derives for you three core primitives:
- Client interface
- Server definition boilerplate
- Request/Response as complete enum types

See `examples/rpc.rs` to learn more.

# todo
- [ ] Documentation
- [x] Examples
- [ ] Tracing integration
- [x] Futures Stream+Sink compat
- [x] Dynamic channels
- [x] In-band error handling (fallible RPC)
- [x] Better wire encoding for errors

V2:
- [x] Arbitrary callers
- [ ] Batching
- [ ] Streaming responses
- [ ] Transport muxing

V3:
- [ ] Uni-directional channels
- [ ] Requests without response

# Non-goals
- Defining our own transport
- Defining our own framing
- Interop with languages other than rust
