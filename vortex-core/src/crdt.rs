use std::sync::atomic::{AtomicU64, Ordering};

/// Wrapper to pad a value to the size of a cache line (64 bytes on x86/ARM).
/// This prevents false sharing where multiple cores fight over the same cache line.
#[repr(align(64))]
struct CachePadded<T> {
    value: T,
}

impl<T> CachePadded<T> {
    fn new(value: T) -> Self {
        Self { value }
    }
}

/// A Grow-only Counter (G-Counter) CRDT.
/// Allows multiple threads to increment their own local counter without locking.
/// The global value is the sum of all local counters.
pub struct GCounter {
    counters: Vec<CachePadded<AtomicU64>>,
}

impl GCounter {
    /// Create a new G-Counter with `size` slots (usually one per core).
    pub fn new(size: usize) -> Self {
        let mut counters = Vec::with_capacity(size);
        for _ in 0..size {
            counters.push(CachePadded::new(AtomicU64::new(0)));
        }
        Self { counters }
    }

    /// Increment the counter for a specific node (core).
    pub fn inc(&self, node_id: usize) {
        if let Some(counter) = self.counters.get(node_id) {
            counter.value.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Read the global value by summing all local counters.
    /// This is eventually consistent.
    pub fn read(&self) -> u64 {
        self.counters
            .iter()
            .map(|c| c.value.load(Ordering::Relaxed))
            .sum()
    }
}
