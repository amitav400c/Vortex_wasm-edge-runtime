# Architecture Guide

This document provides a deep dive into Vortex's system design and implementation.

## Overview

Vortex is built on three core principles:
1. **Thread-per-core** - Eliminate lock contention
2. **Zero-copy** - Minimize allocations and data movement
3. **Async I/O** - Maximize hardware utilization

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        User Space                           │
│  ┌────────────────────────────────────────────────────────┐│
│  │           Main Thread (Orchestrator)                   ││
│  │  ┌──────────────────────────────────────────────┐     ││
│  │  │  Config Loader (TOML + Env Vars)             │     ││
│  │  │  Module Registry (ArcSwap for hot-reload)    │     ││
│  │  │  eBPF Initializer (XDP program loader)       │     ││
│  │  └──────────────────────────────────────────────┘     ││
│  └────────────────────────────────────────────────────────┘│
│           │││                                                │
│           │││ Spawn per-core executors                      │
│           ▼▼▼                                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │  Core 0     │  │  Core 1     │  │  Core N     │        │
│  │ ┌─────────┐ │  │ ┌─────────┐ │  │ ┌─────────┐ │        │
│  │ │glommio  │ │  │ │glommio  │ │  │ │glommio  │ │        │
│  │ │Executor │ │  │ │Executor │ │  │ │Executor │ │        │
│  │ └────┬────┘ │  │ └────┬────┘ │  │ └────┬────┘ │        │
│  │      │      │  │      │      │  │      │      │        │
│  │ ┌────▼────┐ │  │ ┌────▼────┐ │  │ ┌────▼────┐ │        │
│  │ │ Server  │ │  │ │ Server  │ │  │ │ Server  │ │        │
│  │ │Instance │ │  │ │Instance │ │  │ │Instance │ │        │
│  │ └────┬────┘ │  │ └────┬────┘ │  │ └────┬────┘ │        │
│  └──────┼──────┘  └──────┼──────┘  └──────┼──────┘        │
└─────────┼─────────────────┼─────────────────┼──────────────┘
          │                 │                 │
          │ io_uring        │ io_uring        │ io_uring
          ▼                 ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│                      Kernel Space                           │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐           │
│  │  io_uring  │  │  io_uring  │  │  io_uring  │           │
│  │   SQ/CQ    │  │   SQ/CQ    │  │   SQ/CQ    │           │
│  └──────┬─────┘  └──────┬─────┘  └──────┬─────┘           │
│         │                │                │                 │
│  ┌──────▼────────────────▼────────────────▼──────┐         │
│  │           TCP/IP Stack                         │         │
│  └──────┬─────────────────────────────────────────┘         │
│         │                                                    │
│  ┌──────▼─────┐   eBPF XDP Program                         │
│  │  Network   │   (Packet counting)                        │
│  │  Driver    │                                             │
│  └────────────┘                                             │
└─────────────────────────────────────────────────────────────┘
```

## Component Deep Dive

### 1. Thread-Per-Core Architecture (glommio)

**Why?**
- Eliminates synchronization overhead
- Better CPU cache utilization
- Predictable performance

**Implementation:**
```rust
let handles: Vec<_> = (0..cpu_count).map(|i| {
    std::thread::spawn(move || {
        LocalExecutorBuilder::new(Placement::Fixed(i))
            .spawn(move || async move {
                let server = Server::new(port, registry, rate_limiter, i, max_requests);
                server.run().await
            }).unwrap()
    })
}).collect();
```

Each core runs:
- Dedicated `glommio` executor (pinned to that core)
- Dedicated `Server` instance
- Dedicated `io_uring` instance
- Shared-nothing design (except for ArcSwap registry and CRDT counter)

### 2. Zero-Copy HTTP Parsing

**Goal:** Parse HTTP without copying data.

**Technique:**
```rust
let mut buffer = [0u8; 1024];
let n = stream.read(&mut buffer).await?;

let mut headers = [httparse::EMPTY_HEADER; 16];
let mut req = httparse::Request::new(&mut headers);
req.parse(&buffer[..n])?;  // <-- No copy!
```

**Benefits:**
- ~30% latency reduction
- Lower memory pressure
- Cache-friendly

### 3. Module Registry & Hot-Reloading

**Challenge:** Update Wasm modules without stopping the server.

**Solution:** `ArcSwap<Pipeline>`

```rust
pub struct ModuleRegistry {
    pipeline: ArcSwap<Pipeline>,
}

impl ModuleRegistry {
    pub fn update(&self, wasm_bytes: &[u8]) -> Result<()> {
        let new_pipeline = Pipeline::from_bytes(wasm_bytes)?;
        self.pipeline.store(Arc::new(new_pipeline));  // Atomic swap!
        Ok(())
    }
    
    pub fn get(&self) -> Arc<Pipeline> {
        self.pipeline.load_full()  // Read current version
    }
}
```

**Key Properties:**
- Lock-free reads (no mutex)
- Atomic updates (one instruction)
- Old requests see old pipeline, new requests see new pipeline
- No downtime

### 4. CRDT Rate Limiter

**Challenge:** Shared rate limiting across cores without locks.

**Solution:** Grow-only Counter (G-Counter)

```rust
pub struct GCounter {
    counters: Vec<AtomicU64>,  // One per core
}

impl GCounter {
    pub fn inc(&self, node_id: usize) {
        self.counters[node_id].fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn read(&self) -> u64 {
        self.counters.iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum()
    }
}
```

**Properties:**
- Each core updates **its own** counter (no contention)
- Reads sum all counters (eventual consistency)
- No locks, no CAS loops

**Trade-off:** Not perfectly accurate under extreme load, but good enough for rate limiting.

### 5. Pipeline Execution

**Flow:**
```
Request
  │
  ▼
┌─────────────┐
│ Module 1    │  (auth.wat)
│ Returns 200 │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Module 2    │  (rate_limit.wat)
│ Returns 429 │ ──────────► STOP (return 429 to client)
└─────────────┘
```

**Implementation:**
```rust
for module in &self.modules {
    match module.run_request().await {
        Ok(body) if body == "403" || body == "429" => {
            return early_exit(body);  // Stop pipeline
        }
        Ok(body) => { /* Continue */ }
        Err(e) => return Err(e),
    }
}
```

### 6. eBPF Integration

**XDP Program (Kernel-level packet counting):**

```rust
// vortex-bpf/src/main.rs
#[xdp]
pub fn vortex_xdp(ctx: XdpContext) -> u32 {
    let key: u32 = 0;
    if let Some(count) = PACKET_COUNTS.get_ptr_mut(&key) {
        unsafe { *count += 1; }
    }
    xdp_action::XDP_PASS  // Let packet through
}
```

**Userspace Loader:**
```rust
let mut bpf = Ebpf::load_file("vortex-bpf/target/.../vortex-bpf")?;
let program: &mut Xdp = bpf.program_mut("vortex_xdp")?;
program.load()?;
program.attach("lo", XdpFlags::default())?;
```

**Benefits:**
- Minimal overhead (kernel-level)
- Bypasses userspace for metrics
- Accurate packet counting

### 7. Distributed Tracing

**Instrumentation Points:**
1. `Server::handle_connection` - HTTP layer
2. `Pipeline::run_request` - Pipeline orchestration
3. `WasmModule::run_request` - Individual module execution

**Trace Hierarchy:**
```
handle_connection
├── run_request (pipeline)
│   ├── run_request (module 1)
│   ├── run_request (module 2)
│   └── run_request (module 3)
```

**Implementation:**
```rust
#[tracing::instrument(skip(self))]
async fn handle_connection(self, stream: TcpStream) -> io::Result<()> {
    tracing::info!("Accepted connection");
    // ...
}
```

## Performance Optimizations

### 1. Fixed-Size Buffers
- Avoid dynamic allocations in hot path
- 1KB buffer for HTTP headers (sufficient for most requests)

### 2. Inline Small Functions
- `#[inline]` on critical path functions
- Help compiler optimize across boundaries

### 3. CRDT Relaxed Ordering
- `Ordering::Relaxed` for counters (not sequential consistency)
- Huge performance win (no memory barriers)

### 4. Pipeline Early Exit
- Stop on first error/403/429
- Don't waste CPU on doomed requests

## Scaling Characteristics

| Cores | RPS (Observed) | Scaling Efficiency |
|-------|----------------|-------------------|
| 4     | ~40k           | 100%              |
| 8     | ~75k           | 94%               |
| 12    | ~130k          | 108% (superlinear!)|

**Why superlinear?** Better cache locality as load is spread.

## Debugging Tips

Enable tracing:
```bash
RUST_LOG=info ./target/release/vortex-core module.wat
```

Run with eBPF (requires sudo):
```bash
sudo ./target/release/vortex-core module.wat
```

Profile with `perf`:
```bash
sudo perf record -F 99 -a -g -- sleep 10
sudo perf script | ../FlameGraph/stackcollapse-perf.pl | ../FlameGraph/flamegraph.pl > vortex.svg
```
