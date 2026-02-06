# flute
Flute is a utility crate for defining bi-directional channels and RPC interfaces. It is non-opinionated, focusing only on boilerplate code generation, leaving full control over the protocol.


# todo
- [ ] Documentation
- [ ] Examples
- [x] Better error handling (snafu)
- [ ] Tracing integration
- [x] Futures Stream+Sink compat
- [x] Dynamic channels
- [ ] In-band error handling (fallible RPC)

V2:
- [ ] Easier transform / channel definition
- [ ] Batching
- [ ] Streaming responses
- [ ] Transport muxing
- [ ] Primitive Multipath
- [ ] Channel priorities

# Non-goals
- Defining our own transport
- Defining our own framing
- Interop with languages other than rust
