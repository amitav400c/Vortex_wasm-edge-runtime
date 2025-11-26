use std::time::Instant;
use vortex_core::simd_parser::{find_crlf, find_crlf_scalar};

fn bench_crlf_search() {
    println!("\n=== CRLF Search Benchmarks ===\n");

    // Test data
    let short_data = b"GET / HTTP/1.1\r\nHost: localhost\r\n";
    let medium_data: Vec<u8> = [b'X'; 120].iter().chain(b"\r\n").copied().collect();
    let long_data: Vec<u8> = [b'Y'; 500].iter().chain(b"\r\n").copied().collect();
    let very_long_data: Vec<u8> = [b'Z'; 2000].iter().chain(b"\r\n").copied().collect();

    let test_cases = vec![
        ("Short (32B)", short_data as &[u8]),
        ("Medium (128B)", &medium_data),
        ("Long (512B)", &long_data),
        ("Very Long (2KB)", &very_long_data),
    ];

    for (name, data) in test_cases {
        // Scalar benchmark
        let iterations = 1_000_000;
        let start = Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(find_crlf_scalar(std::hint::black_box(data)));
        }
        let scalar_duration = start.elapsed();
        let scalar_ns = scalar_duration.as_nanos() / iterations;

        // SIMD benchmark
        let start = Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(find_crlf(std::hint::black_box(data)));
        }
        let simd_duration = start.elapsed();
        let simd_ns = simd_duration.as_nanos() / iterations;

        let speedup = scalar_ns as f64 / simd_ns as f64;

        println!("{}", name);
        println!("  Scalar: {}ns", scalar_ns);
        println!("  SIMD:   {}ns", simd_ns);
        println!("  Speedup: {:.2}x\n", speedup);
    }
}

use std::sync::Arc;
use std::thread;
use vortex_core::crdt::GCounter;

fn bench_crdt_contention() {
    println!("\n=== CRDT Contention Benchmark ===\n");

    let num_threads = 8;
    let iterations = 10_000_000;

    let counter = Arc::new(GCounter::new(num_threads));
    let start = Instant::now();

    let mut handles = vec![];

    for i in 0..num_threads {
        let counter = counter.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..iterations {
                counter.inc(i);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    let total_ops = num_threads * iterations;
    let ops_per_sec = total_ops as f64 / duration.as_secs_f64();

    println!("Threads: {}", num_threads);
    println!("Total Ops: {}", total_ops);
    println!("Duration: {:.2?}", duration);
    println!("Throughput: {:.2} M ops/sec", ops_per_sec / 1_000_000.0);
}

fn main() {
    println!("Vortex Benchmarks");
    println!("=================");

    #[cfg(target_arch = "x86_64")]
    {
        println!("CPU Features:");
        println!("  AVX2:   {}", is_x86_feature_detected!("avx2"));
        println!("  SSE4.2: {}", is_x86_feature_detected!("sse4.2"));
    }

    bench_crlf_search();
    bench_crdt_contention();

    println!("\nBenchmark complete!");
}
