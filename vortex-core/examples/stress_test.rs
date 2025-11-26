use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let chaos_mode = args.contains(&"--chaos".to_string());

    // Parse target count if provided, default to time-based or high number
    let target_count = if let Some(idx) = args.iter().position(|r| r == "--count") {
        args.get(idx + 1)
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(10_000_000)
    } else {
        0 // 0 means time-based (default 10s)
    };

    let duration = if target_count > 0 {
        Duration::from_secs(3600) // 1 hour max if counting
    } else {
        Duration::from_secs(10)
    };

    let concurrency = 512; // Increased for higher throughput
    let target_addr = "127.0.0.1:8080";

    if target_count > 0 {
        println!(
            "Starting HIGH-PERFORMANCE stress test against {} for {} requests with {} threads...",
            target_addr, target_count, concurrency
        );
    } else {
        println!(
            "Starting HIGH-PERFORMANCE stress test against {} for {:?} with {} threads...",
            target_addr, duration, concurrency
        );
    }

    if chaos_mode {
        println!("CHAOS MODE ENABLED: Will deploy modules randomly during test.");
    }

    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    let rate_limited_count = Arc::new(AtomicUsize::new(0));
    let start_time = Instant::now();

    // Chaos Thread
    if chaos_mode {
        let success_clone = success_count.clone();
        let error_clone = error_count.clone();
        thread::spawn(move || {
            let modules = vec!["tests/fixtures/simple.wat", "tests/fixtures/auth.wat"];
            let mut idx = 0;
            while start_time.elapsed() < duration {
                if target_count > 0 {
                    let current =
                        success_clone.load(Ordering::Relaxed) + error_clone.load(Ordering::Relaxed);
                    if current >= target_count {
                        break;
                    }
                }

                thread::sleep(Duration::from_millis(500)); // Deploy every 500ms

                let path = modules[idx % modules.len()];
                idx += 1;

                if let Ok(wasm_bytes) = fs::read(path) {
                    if let Ok(mut stream) = TcpStream::connect(target_addr) {
                        let req = format!(
                            "POST /_vortex/deploy HTTP/1.1\r\nHost: localhost:8080\r\nContent-Length: {}\r\n\r\n",
                            wasm_bytes.len()
                        );

                        let _ = stream.write_all(req.as_bytes());
                        let _ = stream.write_all(&wasm_bytes);

                        let mut buffer = [0u8; 1024];
                        if let Ok(n) = stream.read(&mut buffer) {
                            let response = String::from_utf8_lossy(&buffer[..n]);
                            if response.contains("200 OK") {
                                println!("[CHAOS] Deployed {}", path);
                            } else {
                                println!(
                                    "[CHAOS] Failed to deploy {}: {}",
                                    path,
                                    response.lines().next().unwrap_or("")
                                );
                            }
                        }
                    }
                } else {
                    println!("[CHAOS] Failed to read {}", path);
                }
            }
        });
    }

    let mut handles = vec![];

    for _ in 0..concurrency {
        let success = success_count.clone();
        let error = error_count.clone();
        let rate_limited = rate_limited_count.clone();

        handles.push(thread::spawn(move || {
            let start = Instant::now();
            let request = b"GET / HTTP/1.1\r\nHost: localhost:8080\r\n\r\n";
            let mut buffer = [0u8; 1024];

            loop {
                // Check termination conditions
                if target_count > 0 {
                    let current = success.load(Ordering::Relaxed) + error.load(Ordering::Relaxed);
                    if current >= target_count {
                        break;
                    }
                } else if start.elapsed() >= duration {
                    break;
                }

                // Open raw socket
                if let Ok(mut stream) = TcpStream::connect(target_addr) {
                    if stream.write_all(request).is_ok() {
                        if let Ok(n) = stream.read(&mut buffer) {
                            if n > 0 {
                                let response = String::from_utf8_lossy(&buffer[..n]);
                                if response.contains("200 OK") {
                                    success.fetch_add(1, Ordering::Relaxed);
                                } else if response.contains("429 Too Many Requests") {
                                    rate_limited.fetch_add(1, Ordering::Relaxed);
                                    error.fetch_add(1, Ordering::Relaxed);
                                } else {
                                    error.fetch_add(1, Ordering::Relaxed);
                                }
                            } else {
                                error.fetch_add(1, Ordering::Relaxed);
                            }
                        } else {
                            error.fetch_add(1, Ordering::Relaxed);
                        }
                    } else {
                        error.fetch_add(1, Ordering::Relaxed);
                    }
                } else {
                    error.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start_time.elapsed();
    let successes = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    let rate_limits = rate_limited_count.load(Ordering::Relaxed);
    let total = successes + errors;
    let rps = total as f64 / elapsed.as_secs_f64();

    println!("Test completed in {:.2?}", elapsed);
    println!("Total Requests: {}", total);
    println!("Successes: {}", successes);
    println!("Errors: {}", errors);
    println!("  - Rate Limited (429): {}", rate_limits);
    println!("  - Other Errors: {}", errors - rate_limits);
    println!("RPS: {:.2}", rps);
}
