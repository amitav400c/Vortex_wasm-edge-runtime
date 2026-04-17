use futures_lite::{AsyncReadExt, AsyncWriteExt};
use glommio::net::TcpStream;
use serde::{Deserialize, Serialize};
use futures_lite::future::FutureExt;
use std::time::Duration;

#[derive(Serialize)]
struct OpaInput<'a> {
    method: &'a str,
    path: &'a str,
}

#[derive(Serialize)]
struct OpaRequest<'a> {
    input: OpaInput<'a>,
}

#[derive(Deserialize, Debug)]
struct OpaResponse {
    result: Option<bool>,
}

// TODO: In the future, as the project matures, replace this handwritten
// HTTP/1.1 TCP client with the `hyper` crate adapted for the Glommio executor.
// This will provide better HTTP/2 support, robust Keep-Alive connection pooling,
// and hardened edge-case handling.
pub async fn check_opa_authorization(
    endpoint: &str,
    method: &str,
    path: &str,
) -> anyhow::Result<bool> {
    // Basic URL parsing (assuming HTTP only for now)
    let without_scheme = endpoint.strip_prefix("http://").unwrap_or(endpoint);
    let (host_port, api_path) = match without_scheme.find('/') {
        Some(idx) => (&without_scheme[..idx], &without_scheme[idx..]),
        None => (without_scheme, "/"),
    };

    // If port is missing, default to 80
    let host_port_owned = if host_port.contains(':') {
        host_port.to_string()
    } else {
        format!("{}:80", host_port)
    };

    // Build JSON payload
    let req_body = OpaRequest {
        input: OpaInput { method, path },
    };
    let body_str = serde_json::to_string(&req_body)?;

    // Build HTTP POST request string
    let http_req = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        api_path,
        host_port,
        body_str.len(),
        body_str
    );

    let network_operation = async {
        // Connect to OPA server (creates a new connection per request currently)
        let mut stream = TcpStream::connect(&host_port_owned)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to OPA server: {}", e))?;

        // Send request
        stream.write_all(http_req.as_bytes()).await?;
        stream.flush().await?;

        // Read response
        let mut buffer = [0u8; 4096];
        let mut response_data = Vec::new();

        loop {
            let n = stream.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            response_data.extend_from_slice(&buffer[..n]);
            // Basic check if we reached end of JSON (simplistic)
            if response_data.ends_with(b"}") || response_data.ends_with(b"\n") {
                break;
            }
        }
        Ok::<Vec<u8>, anyhow::Error>(response_data)
    };

    let timeout_operation = async {
        glommio::timer::sleep(Duration::from_millis(500)).await;
        Err(anyhow::anyhow!("OPA server timeout (500ms)"))
    };

    let response_data = match network_operation.or(timeout_operation).await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("OPA Error: {}", e);
            return Ok(false); // Fail closed on timeout or network error
        }
    };

    let response_str = String::from_utf8_lossy(&response_data);

    // Find the start of the body (after \r\n\r\n)
    if let Some(body_start_idx) = response_str.find("\r\n\r\n") {
        let body_content = &response_str[body_start_idx + 4..];
        
        match serde_json::from_str::<OpaResponse>(body_content) {
            Ok(opa_res) => {
                return Ok(opa_res.result.unwrap_or(false));
            }
            Err(e) => {
                tracing::error!("Failed to parse OPA JSON response: {} - body was: {}", e, body_content);
                return Ok(false); // Fail closed
            }
        }
    }

    tracing::error!("Malformed HTTP response from OPA server");
    Ok(false) // Fail closed
}
