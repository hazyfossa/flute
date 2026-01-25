# flute
Flute is a utility crate for defining RPC interfaces. It is non-opinionated, focusing only on boilerplate code generation, leaving full control over the protocol.


# todo
- [ ] Documentation
- [ ] Examples
- [x] Better error handling (snafu)
- [ ] Tracing integration
- [x] Futures Stream+Sink compat
- [x] Dynamic channels

V2:
- [ ] Batching
- [ ] Streaming responses
- [ ] Transport muxing
- [x] Primitive Multipath
- [ ] Channel priorities

# Non-goals
- Defining our own transport
- Defining our own framing
- Interop with languages other than rust
