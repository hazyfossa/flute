# flute
Flute is a utility crate for defining bi-directional channels and RPC interfaces. It is non-opinionated, focusing only on boilerplate code generation, leaving full control over the protocol.


# todo
- [ ] Documentation
- [x] Examples
- [ ] Better error handling
- [ ] Tracing integration
- [x] Futures Stream+Sink compat
- [x] Dynamic channels
- [x] In-band error handling (fallible RPC)
- [ ] Defmt support for in-band errors

V2:
- [ ] Batching
- [ ] Streaming responses
- [ ] Transport muxing
- [ ] Primitive Multipath
- [ ] Channel priorities

V3:
- [ ] completion-based io primitives (proper work on performance delayed until this)

# Non-goals
- Defining our own transport
- Defining our own framing
- Interop with languages other than rust
