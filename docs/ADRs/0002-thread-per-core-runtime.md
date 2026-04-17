# ADR 0002: Thread-Per-Core Async Runtime (Glommio)

## Status
Accepted

## Context
When building a high-performance WebAssembly edge runtime, the choice of the underlying async I/O runtime determines the entire architecture of the project. The Rust ecosystem is heavily dominated by Tokio, a work-stealing executor. However, edge runtimes and API gateways (like Envoy or NGINX) typically use an event-driven, thread-per-core architecture to maximize cache locality and eliminate lock contention.

## Options Analyzed

### 1. Tokio (Work-Stealing)
*   **Pros:** The industry standard. Massive ecosystem. Every crate (like `reqwest`, `hyper`, `sqlx`) just works out of the box.
*   **Cons:** Threads "steal" work from each other to balance load. This causes cache thrashing, context switching overhead, and requires cross-thread synchronization (locks/mutexes) which hurts tail latency under extreme load.

### 2. Glommio (Thread-Per-Core / io_uring)
*   **Pros:** Pure thread-per-core (TPC) architecture. Zero-copy networking using Linux's `io_uring`. Data never crosses thread boundaries, meaning no mutexes, perfect CPU cache locality, and deterministic, ultra-low tail latency.
*   **Cons:** Fragmented ecosystem. Tokio-dependent crates panic. We have to manually implement or adapt lower-level protocols (HTTP, TLS).

## Decision
We chose **Glommio** as the core async executor.

The primary goal of Vortex is to provide bare-metal performance for WebAssembly edge compute. By pinning threads to cores and using `io_uring`, we eliminate lock contention entirely. 

## Consequences
*   **Architectural constraint:** We cannot use off-the-shelf HTTP clients or servers that rely on Tokio (e.g., `hyper` out-of-the-box, `reqwest`, `axum`).
*   **Development overhead:** We must build custom parsers and network clients (like the raw TCP OPA client in ADR 0001).
*   **Performance:** We achieve significantly higher throughput and lower latency on modern Linux kernels compared to a standard work-stealing setup.
