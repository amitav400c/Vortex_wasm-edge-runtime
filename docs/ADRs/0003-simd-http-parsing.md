# ADR 0003: Custom SIMD HTTP Parsing

## Status
Accepted

## Context
Because we selected Glommio (ADR 0002) as our async runtime, we cannot use standard Tokio-bound HTTP frameworks like `axum` or `actix-web`. While we could write an adapter for `hyper`, we needed an incredibly fast way to parse incoming HTTP/1.x headers before handing the payload off to the WebAssembly pipeline.

## Options Analyzed

### 1. Adapt `hyper` to Glommio
*   **Pros:** Spec-compliant, handles all edge cases (chunked encoding, HTTP/2).
*   **Cons:** Requires maintaining complex glue code between Hyper's Executor traits and Glommio's spawn mechanisms. Significant overhead for simple API Gateway routing.

### 2. Standard string parsing (Regex or Split)
*   **Pros:** Easy to write.
*   **Cons:** Too slow. Parsing string slices byte-by-byte for `\r\n` is a major bottleneck in high-throughput edge servers.

### 3. Custom SIMD-accelerated parser (`httparse` + AVX2)
*   **Pros:** Blistering speed. Uses CPU vector instructions (AVX2/SSE4.2) to scan 32 bytes at a time for CRLF tokens and common HTTP methods.
*   **Cons:** Requires `unsafe` Rust. Platform-specific (x86_64). Requires fallback logic for non-compatible CPUs.

## Decision
We implemented a **Custom SIMD-accelerated parser** (`simd_parser.rs`) using `httparse` for the state machine and raw AVX2 intrinsics for buffer scanning.

For an edge runtime, routing decisions (Method + Path) need to happen in nanoseconds before the WebAssembly sandbox is invoked.

## Consequences
*   **Performance:** Unmatched HTTP/1.1 parsing speed on x86_64 architecture.
*   **Safety:** We introduced `unsafe` blocks for SIMD intrinsics, which requires careful auditing and testing (see `test_simd_scalar_equivalence`).
*   **Fallback:** We maintain a scalar fallback for ARM or older CPUs without AVX2/SSE4.2 support.
