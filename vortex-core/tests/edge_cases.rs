use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
fn test_edge_cases() {
    // Generate certs before starting
    let cert_status = Command::new("bash")
        .current_dir("..")
        .arg("./scripts/generate_certs.sh")
        .status()
        .expect("Failed to generate certs");
    assert!(cert_status.success());

    // Build the server first
    let status = Command::new("cargo")
        .args(&["build", "-p", "vortex-core"])
        .status()
        .expect("Failed to build server");
    assert!(status.success());

    // Start server with trap module
    let mut server = Command::new(env!("CARGO_BIN_EXE_vortex-core"))
        .current_dir("..")
        .arg("--config")
        .arg("vortex.toml")
        .arg("vortex-core/tests/fixtures/trap.wat")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start server");

    // Give it time to start
    thread::sleep(Duration::from_secs(2));

    // Test 1: Trap (Unreachable)
    {
        let output = Command::new("curl")
            .arg("-k")
            .arg("-i")
            .arg("https://localhost:8080")
            .output()
            .expect("Failed to run curl");

        let response = String::from_utf8_lossy(&output.stdout);
        assert!(
            response.contains("500 Internal Server Error"),
            "Trap should return 500. Got: {}", response
        );
    }

    // Test 2: Empty Connection
    {
        let stream = TcpStream::connect("127.0.0.1:8080").expect("Failed to connect");
        // Close immediately (drop)
        drop(stream);
        // Server should handle this gracefully (logs might show error, but shouldn't crash)
    }

    // Test 3: Malformed Request
    {
        // Malformed TLS request doesn't return an HTTP response, it just gets dropped by TLS Acceptor
        // Let's connect and send garbage, ensuring it doesn't crash the server.
        let mut stream = TcpStream::connect("127.0.0.1:8080").expect("Failed to connect");
        stream
            .write_all(b"GARBAGE DATA\r\n\r\n")
            .expect("Failed to write");
        // Read will just return 0 bytes as connection closes
        let mut buffer = [0; 1024];
        let _ = stream.read(&mut buffer);
    }

    server.kill().expect("Failed to kill server");
}

#[test]
fn test_opa_attacks() {
    // Test 1: OPA Server Offline
    let mut server1 = Command::new(env!("CARGO_BIN_EXE_vortex-core"))
        .current_dir("..")
        .arg("--config")
        .arg("vortex.toml")
        .env("VORTEX_PORT", "8082")
        .env("VORTEX_OPA_ENDPOINT", "http://127.0.0.1:9999/offline")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start server");

    thread::sleep(Duration::from_secs(2));

    {
        let output = Command::new("curl")
            .arg("-k")
            .arg("-i")
            .arg("https://localhost:8082")
            .output()
            .expect("Failed to run curl");
        let response = String::from_utf8_lossy(&output.stdout);
        assert!(response.contains("403 Forbidden"), "Offline OPA should fail closed with 403. Got: {}", response);
    }

    server1.kill().unwrap();

    // Setup mock OPA listener
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let opa_endpoint = format!("http://127.0.0.1:{}/authz", port);

    let mut server2 = Command::new(env!("CARGO_BIN_EXE_vortex-core"))
        .current_dir("..")
        .arg("--config")
        .arg("vortex.toml")
        .env("VORTEX_PORT", "8083")
        .env("VORTEX_OPA_ENDPOINT", &opa_endpoint)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start server");

    thread::sleep(Duration::from_secs(2));

    // Test 2: OPA Malformed Response
    {
        let listener_clone = listener.try_clone().unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener_clone.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n{INVALID_JSON");
            }
        });

        let output = Command::new("curl")
            .arg("-k")
            .arg("-i")
            .arg("https://localhost:8083")
            .output()
            .unwrap();
        let response = String::from_utf8_lossy(&output.stdout);
        assert!(response.contains("403 Forbidden"), "Malformed JSON should fail closed with 403. Got: {}", response);
    }

    // Test 3: OPA Timeout Attack
    {
        let listener_clone = listener.try_clone().unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener_clone.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                // Sleep for 2 seconds to trigger the 500ms timeout
                thread::sleep(Duration::from_secs(2));
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n{\"result\":true}");
            }
        });

        let output = Command::new("curl")
            .arg("-k")
            .arg("-i")
            .arg("https://localhost:8083")
            .output()
            .unwrap();
        let response = String::from_utf8_lossy(&output.stdout);
        assert!(response.contains("403 Forbidden"), "Timeout should fail closed with 403. Got: {}", response);
    }

    server2.kill().unwrap();
}
