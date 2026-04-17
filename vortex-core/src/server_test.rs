#[cfg(test)]
mod tests {
    use crate::opa::check_opa_authorization;
    use crate::server::{ModuleRegistry, Server};
    use crate::wasm::WasmEngine;
    use futures_lite::{AsyncReadExt, AsyncWriteExt, StreamExt};
    use glommio::net::TcpListener;
    use glommio::{LocalExecutorBuilder, Placement};

    #[test]
    fn test_opa_authorization_allow() {
        let builder = LocalExecutorBuilder::new(Placement::Unbound);
        let handle = builder
            .spawn(|| async move {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let port = listener.local_addr().unwrap().port();
                let endpoint = format!("http://127.0.0.1:{}/v1/data/authz/allow", port);

                // Spawn mock OPA server
                glommio::spawn_local(async move {
                    let mut incoming = listener.incoming();
                    if let Some(Ok(mut stream)) = incoming.next().await {
                        let mut buf = [0u8; 1024];
                        let _ = stream.read(&mut buf).await;
                        let response =
                            "HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n{\"result\":true}";
                        let _ = stream.write_all(response.as_bytes()).await;
                    }
                })
                .detach();

                let result = check_opa_authorization(&endpoint, "GET", "/api/data")
                    .await
                    .unwrap();
                assert_eq!(result, true);
            })
            .unwrap();

        handle.join().unwrap();
    }

    #[test]
    fn test_opa_authorization_deny() {
        let builder = LocalExecutorBuilder::new(Placement::Unbound);
        let handle = builder
            .spawn(|| async move {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let port = listener.local_addr().unwrap().port();
                let endpoint = format!("http://127.0.0.1:{}/v1/data/authz/allow", port);

                // Spawn mock OPA server
                glommio::spawn_local(async move {
                    let mut incoming = listener.incoming();
                    if let Some(Ok(mut stream)) = incoming.next().await {
                        let mut buf = [0u8; 1024];
                        let _ = stream.read(&mut buf).await;
                        let response =
                            "HTTP/1.1 200 OK\r\nContent-Length: 16\r\n\r\n{\"result\":false}";
                        let _ = stream.write_all(response.as_bytes()).await;
                    }
                })
                .detach();

                let result = check_opa_authorization(&endpoint, "POST", "/api/data")
                    .await
                    .unwrap();
                assert_eq!(result, false);
            })
            .unwrap();

        handle.join().unwrap();
    }

    #[test]
    fn test_wasm_execution() {
        let builder = LocalExecutorBuilder::new(Placement::Unbound);
        let handle = builder
            .spawn(|| async move {
                let engine = WasmEngine::new().unwrap();
                let registry = ModuleRegistry::new(engine);
                let rate_limiter = std::sync::Arc::new(crate::crdt::GCounter::new(1));
                let _server = Server::new(8081, registry.clone(), rate_limiter, 0, 100, None, None);

                // Load the simple WAT module
                let wat = r#"
                (module
                  (func (export "handle_request") (result i32)
                    i32.const 0
                  )
                  (memory (export "memory") 1)
                )
            "#;
                registry
                    .update(wat.as_bytes())
                    .expect("Failed to load module");

                // Verify it loaded (by getting it)
                let pipeline = registry.get();
                assert_eq!(pipeline.modules().len(), 1);
            })
            .unwrap();

        handle.join().unwrap();
    }
}
