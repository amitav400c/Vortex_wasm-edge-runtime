use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
fn test_edge_cases() {
    // Build the server first
    let status = Command::new("cargo")
        .args(&["build", "-p", "vortex-core"])
        .status()
        .expect("Failed to build server");
    assert!(status.success());

    // Start server with trap module
    let mut server = Command::new("cargo")
        .args(&["run", "-p", "vortex-core", "--", "tests/fixtures/trap.wat"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start server");

    // Give it time to start
    thread::sleep(Duration::from_secs(5));

    // Test 1: Trap (Unreachable)
    {
        let output = Command::new("curl")
            .arg("-i")
            .arg("http://localhost:8080")
            .output()
            .expect("Failed to run curl");

        let response = String::from_utf8_lossy(&output.stdout);
        assert!(
            response.contains("500 Internal Server Error"),
            "Trap should return 500"
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
        let mut stream = TcpStream::connect("127.0.0.1:8080").expect("Failed to connect");
        stream
            .write_all(b"GARBAGE DATA\r\n\r\n")
            .expect("Failed to write");

        let mut buffer = [0; 1024];
        let n = stream.read(&mut buffer).unwrap_or(0);
        let response = String::from_utf8_lossy(&buffer[..n]);

        // Should be 400 Bad Request
        assert!(
            response.contains("400 Bad Request"),
            "Malformed request should return 400"
        );
    }

    server.kill().expect("Failed to kill server");
}
