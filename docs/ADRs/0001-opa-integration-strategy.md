# ADR 0001: OPA Integration Strategy

## Status
Proposed

## Context
We need to integrate Open Policy Agent (OPA) into the Vortex WebAssembly runtime for authorization. The runtime is built on `glommio` (a thread-per-core io_uring executor), which restricts the use of standard Tokio-based HTTP clients (like `reqwest`).

We have two primary options for integration:
1. **REST API over Raw TCP:** Build a custom, lightweight HTTP client using `glommio::net::TcpStream` to query an external OPA server.
2. **Native WebAssembly:** Compile OPA policies to `.wasm` and execute them directly inside the Vortex pipeline natively using Wasmtime.

## Options Analyzed

### 1. REST API (Raw TCP Client)
*   **Pros:** Fast to implement, completely decouples the authorization engine (OPA) from the runtime, easier to update policies without restarting the Wasm host.
*   **Cons:** Introduces network latency (even over localhost) for every request. Requires manual HTTP parsing. Without connection pooling (Keep-Alive), TCP handshakes will bottleneck the edge server.

### 2. Native WebAssembly (Wasm)
*   **Pros:** The absolute fastest execution speed with zero network latency. Perfect alignment with Vortex's identity as a high-performance Wasm edge runtime. High security via the Wasm sandbox.
*   **Cons:** Massive upfront engineering cost. Vortex's current `wasm.rs` does not support passing complex JSON memory contexts to guests. Requires fully implementing OPA's specific Wasm ABI (`opa_eval_ctx_new`, `opa_malloc`, etc.) and host built-ins.

## Future Work (Hyper Integration)
**Note for future maintainers:** While the raw TCP client was chosen for speed of initial implementation, a handwritten HTTP parser is prone to missing edge cases and lacks built-in connection pooling/HTTP2 support. 

As the project matures towards a production-ready state, we should transition this raw TCP client to use the `hyper` crate. This will require writing custom `hyper::rt::Executor` and `hyper::rt::Read/Write` traits to adapt Hyper's internals to the Glommio `io_uring` thread-per-core model.

## Decision
We will implement the **REST API (Raw TCP Client)** approach for the initial iteration. 

To mitigate the performance drawbacks, we must ensure the client is optimized for the Glommio thread-per-core model. While Native WebAssembly is the ultimate "end-game" for an edge runtime like Vortex, the current Wasm engine is too immature to support the complex OPA ABI without a multi-week rewrite. 

## Consequences
*   We accept a slight latency penalty (network hop to OPA server) in exchange for faster time-to-market.
*   We take on the maintenance burden of a custom, lightweight HTTP parser for the `glommio` runtime.
*   **Future Work:** The Native WebAssembly approach is deferred to a future milestone (v0.2 or v0.3) once the base Wasm context engine is stabilized.
