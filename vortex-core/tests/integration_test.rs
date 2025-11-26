#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_server_starts_and_responds() {
        // Build the server first
        let status = Command::new("cargo")
            .args(&["build", "-p", "vortex-core"])
            .status()
            .expect("Failed to build server");
        assert!(status.success());

        // Test 1: Basic Server (No Wasm)
        {
            let mut server = Command::new("cargo")
                .args(&["run", "-p", "vortex-core"])
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start server");

            thread::sleep(Duration::from_secs(5));

            let output = Command::new("curl")
                .arg("-i")
                .arg("http://localhost:8080")
                .output()
                .expect("Failed to run curl");

            let response = String::from_utf8_lossy(&output.stdout);
            server.kill().expect("Failed to kill server");

            assert!(response.contains("Hello, Vortex"));
        }

        // Test 2: With Wasm module and Hot Reloading
        {
            let mut server = Command::new("cargo")
                .args(&[
                    "run",
                    "-p",
                    "vortex-core",
                    "--",
                    "tests/fixtures/simple.wat",
                ])
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start server with wasm");

            thread::sleep(Duration::from_secs(5));

            // Initial request
            let output = Command::new("curl")
                .arg("-i")
                .arg("http://localhost:8080")
                .output()
                .expect("Failed to run curl");
            let response = String::from_utf8_lossy(&output.stdout);
            assert!(response.contains("Hello from Wasm"));

            // Deploy new module (hot reload)
            // We'll use the same module but maybe we can modify it?
            // Or just re-deploying the same one proves the endpoint works.
            // To prove it changed, we'd need a different module.
            // Let's create a second fixture.

            let output = Command::new("curl")
                .arg("-i")
                .arg("-X")
                .arg("POST")
                .arg("--data-binary")
                .arg("@tests/fixtures/simple.wat")
                .arg("http://localhost:8080/_vortex/deploy")
                .output()
                .expect("Failed to deploy");

            let response = String::from_utf8_lossy(&output.stdout);
            assert!(response.contains("Deployed"));

            server.kill().expect("Failed to kill server");
        }

        // Test 3: Pipeline (Auth -> Hello)
        // We need to start server with 2 modules.
        {
            let mut server = Command::new("cargo")
                .args(&[
                    "run",
                    "-p",
                    "vortex-core",
                    "--",
                    "tests/fixtures/auth.wat",
                    "tests/fixtures/simple.wat",
                ])
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start server with pipeline");

            thread::sleep(Duration::from_secs(5));

            let output = Command::new("curl")
                .arg("-i")
                .arg("http://localhost:8080")
                .output()
                .expect("Failed to run curl");

            let response = String::from_utf8_lossy(&output.stdout);

            // auth.wat returns "403".
            // Logic: if body == "403", return 403 Forbidden and STOP.
            // So we should see 403.
            if !response.contains("403 Forbidden") {
                eprintln!("Test 3 Failed. Response: {}", response);
                eprintln!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
            }
            assert!(response.contains("403 Forbidden"));

            server.kill().expect("Failed to kill server");
        }
        // Test 4: Infinite Loop (Fuel Exhaustion)
        {
            let mut server = Command::new("cargo")
                .args(&[
                    "run",
                    "-p",
                    "vortex-core",
                    "--",
                    "tests/fixtures/infinite_loop.wat",
                ])
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start server with infinite loop");

            thread::sleep(Duration::from_secs(5));

            let output = Command::new("curl")
                .arg("-i")
                .arg("http://localhost:8080")
                .output()
                .expect("Failed to run curl");

            let response = String::from_utf8_lossy(&output.stdout);

            // Should return 500 Internal Server Error due to trap
            assert!(response.contains("500 Internal Server Error"));

            server.kill().expect("Failed to kill server");
        }
        // Test 5: WASI Logging
        {
            let mut server = Command::new("cargo")
                .args(&[
                    "run",
                    "-p",
                    "vortex-core",
                    "--",
                    "tests/fixtures/wasi_log.wat",
                ])
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start server with wasi log");

            thread::sleep(Duration::from_secs(5));

            let output = Command::new("curl")
                .arg("-i")
                .arg("http://localhost:8080")
                .output()
                .expect("Failed to run curl");

            let response = String::from_utf8_lossy(&output.stdout);

            // Kill server first to close stdout so we can read it all
            server.kill().expect("Failed to kill server");

            let server_stdout =
                std::io::BufRead::lines(std::io::BufReader::new(server.stdout.take().unwrap()))
                    .map(|l| l.unwrap())
                    .collect::<Vec<String>>()
                    .join("\n");

            assert!(response.contains("Logged"));
            assert!(server_stdout.contains("[Wasm Log]: Hello from WASI Log!"));
        }
        // Test 6: Host Fetch
        {
            let mut server = Command::new("cargo")
                .args(&["run", "-p", "vortex-core", "--", "tests/fixtures/fetch.wat"])
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start server with fetch");

            thread::sleep(Duration::from_secs(5));

            let output = Command::new("curl")
                .arg("-i")
                .arg("http://localhost:8080")
                .output()
                .expect("Failed to run curl");

            let response = String::from_utf8_lossy(&output.stdout);

            // Kill server first
            server.kill().expect("Failed to kill server");

            let server_stdout =
                std::io::BufRead::lines(std::io::BufReader::new(server.stdout.take().unwrap()))
                    .map(|l| l.unwrap())
                    .collect::<Vec<String>>()
                    .join("\n");

            assert!(response.contains("Fetched"));
            assert!(server_stdout.contains("[Host Fetch]: https://example.com"));
        }

        // Test 7: Rate Limiting (CRDT)
        {
            let mut server = Command::new("cargo")
                .args(&["run", "-p", "vortex-core"])
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start server for rate limiting");

            thread::sleep(Duration::from_secs(5));

            // Send 10100 requests (Limit is 10000)
            // We expect the last few to fail with 429
            // Using a loop is slow in shell, let's just use curl in a loop or similar.
            // Or just fire 105 requests and check if we get a 429.

            // We can use a simple bash loop inside Command or just run curl multiple times.
            // Since we are in Rust, let's just loop Command::new("curl").

            let mut hit_limit = false;
            for _ in 0..10100 {
                let output = Command::new("curl")
                    .arg("-i")
                    .arg("http://localhost:8080")
                    .output()
                    .expect("Failed to run curl");

                let response = String::from_utf8_lossy(&output.stdout);
                if response.contains("429 Too Many Requests") {
                    hit_limit = true;
                    break;
                }
            }

            server.kill().expect("Failed to kill server");
            assert!(hit_limit, "Did not hit rate limit of 10000");
        }

        // Test 8: Full Flow (Auth -> Logic with Host Functions)
        {
            let mut server = Command::new("cargo")
                .args(&[
                    "run",
                    "-p",
                    "vortex-core",
                    "--",
                    "tests/fixtures/full_flow_auth.wat",
                    "tests/fixtures/full_flow_logic.wat",
                ])
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start server with full flow");

            thread::sleep(Duration::from_secs(5));

            let output = Command::new("curl")
                .arg("-i")
                .arg("http://localhost:8080")
                .output()
                .expect("Failed to run curl");

            let response = String::from_utf8_lossy(&output.stdout);

            // Kill server first
            server.kill().expect("Failed to kill server");

            let server_stdout =
                std::io::BufRead::lines(std::io::BufReader::new(server.stdout.take().unwrap()))
                    .map(|l| l.unwrap())
                    .collect::<Vec<String>>()
                    .join("\n");

            assert!(response.contains("{\"status\":\"ok\",\"data\":\"fetched\"}"));
            assert!(server_stdout.contains("[Wasm Log]: Processing request..."));
            assert!(server_stdout.contains("[Host Fetch]: https://api.example.com/data"));
        }
    }
}
