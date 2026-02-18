use futures_lite::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt, StreamExt};
use glommio::net::TcpListener;
use std::io;
use std::net::SocketAddr;
use futures_rustls::TlsAcceptor;

use crate::pipeline::Pipeline;
use crate::wasm::WasmEngine;
use arc_swap::ArcSwap;
use std::sync::Arc;

#[derive(Clone)]
pub struct ModuleRegistry {
    current: Arc<ArcSwap<Pipeline>>,
    engine: WasmEngine,
}

impl ModuleRegistry {
    pub fn new(engine: WasmEngine) -> Self {
        Self {
            current: Arc::new(ArcSwap::from_pointee(Pipeline::empty())),
            engine,
        }
    }

    pub fn get(&self) -> Pipeline {
        self.current.load().as_ref().clone()
    }

    pub fn update(&self, wasm_bytes: &[u8]) -> anyhow::Result<()> {
        // For now, update replaces the ENTIRE pipeline with a SINGLE module.
        // In the future, we might want to update specific stages or append.
        let module = self.engine.load_module_from_bytes(wasm_bytes)?;
        let pipeline = Pipeline::new(vec![module]);
        self.current.store(Arc::new(pipeline));
        Ok(())
    }

    #[allow(dead_code)]
    pub fn load_from_path(&self, path: &str) -> anyhow::Result<()> {
        let module = self.engine.load_module(path)?;
        let pipeline = Pipeline::new(vec![module]);
        self.current.store(Arc::new(pipeline));
        Ok(())
    }

    pub fn load_pipeline(&self, paths: &[String]) -> anyhow::Result<()> {
        let mut modules = Vec::new();
        for path in paths {
            modules.push(self.engine.load_module(path)?);
        }
        self.current.store(Arc::new(Pipeline::new(modules)));
        Ok(())
    }
}

use crate::crdt::GCounter;

#[derive(Clone)]
pub struct Server {
    port: u16,
    registry: ModuleRegistry,
    rate_limiter: Arc<GCounter>,
    node_id: usize,
    max_requests: usize, // Configurable rate limit
    tls_acceptor: Option<TlsAcceptor>,
}

impl Server {
    pub fn new(
        port: u16,
        registry: ModuleRegistry,
        rate_limiter: Arc<GCounter>,
        node_id: usize,
        max_requests: usize,
        tls_config: Option<Arc<rustls::ServerConfig>>,
    ) -> Self {
        let tls_acceptor = tls_config.map(|c| TlsAcceptor::from(c));
        Self {
            port,
            registry,
            rate_limiter,
            node_id,
            max_requests,
            tls_acceptor,
        }
    }

    pub async fn run(self) -> io::Result<()> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port))?;
        println!("Listening on 0.0.0.0:{}", self.port);

        let mut incoming = listener.incoming();

        while let Some(stream) = incoming.next().await {
            match stream {
                Ok(stream) => {
                    let server = self.clone();
                    glommio::spawn_local(async move {
                        if stream.set_nodelay(true).is_ok() {
                            // Nodelay set
                        }
                        let peer_addr = stream.peer_addr().ok();
                        
                        if let Some(acceptor) = server.tls_acceptor.clone() {
                            match acceptor.accept(stream).await {
                                Ok(tls_stream) => {
                                    if let Err(e) = server.handle_connection(tls_stream, peer_addr).await {
                                        tracing::debug!("Connection error: {}", e);
                                    }
                                }
                                Err(e) => {
                                    tracing::debug!("TLS Handshake error: {}", e);
                                }
                            }
                        } else {
                            if let Err(e) = server.handle_connection(stream, peer_addr).await {
                                tracing::debug!("Connection error: {}", e);
                            }
                        }
                    })
                    .detach();
                }
                Err(e) => {
                    eprintln!("Accept error: {}", e);
                }
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, stream), fields(peer_addr))]
    async fn handle_connection<S>(self, mut stream: S, peer_addr: Option<SocketAddr>) -> io::Result<()> 
    where S: AsyncRead + AsyncWrite + Unpin + 'static
    {
        tracing::Span::current().record("peer_addr", format!("{:?}", peer_addr));
        tracing::info!("Accepted connection");

        let mut buffer = [0u8; 4096]; // Larger buffer for reuse

        loop {
            // Read request
            let n = stream.read(&mut buffer).await?;

            if n == 0 {
                return Ok(()); // Connection closed
            }

            // Parse HTTP request with SIMD optimization
            #[cfg(feature = "simd")]
            let _crlf_check = crate::simd_parser::find_crlf(&buffer[..n]);

            let mut headers = [httparse::EMPTY_HEADER; 16];
            let mut req = httparse::Request::new(&mut headers);

            let response = match req.parse(&buffer[..n]) {
                Ok(httparse::Status::Complete(offset)) => {
                    // Check for deploy path
                    let is_deploy = matches!(req.path, Some("/_vortex/deploy"));

                    // Rate Limiting Check (Skip for deploy)
                    if !is_deploy {
                        // Increment local counter
                        self.rate_limiter.inc(self.node_id);

                        // Optimization: Only check global limit every 128 requests to reduce cache traffic
                        // if local_request_count % 128 == 0 {
                        let current_count = self.rate_limiter.read();
                        if current_count > self.max_requests as u64 {
                            let response =
                                "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n";
                            stream.write_all(response.as_bytes()).await?;
                            stream.flush().await?;
                            continue;
                        }
                        // }
                    }

                    match (req.method, req.path) {
                        (Some("POST"), Some("/_vortex/deploy")) => {
                            // Read body
                            let body_start = offset;
                            let body = &buffer[body_start..n];

                            match self.registry.update(body) {
                                Ok(_) => "HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nDeployed"
                                    .to_string(),
                                Err(e) => format!(
                                    "HTTP/1.1 500 Error\r\nContent-Length: {}\r\n\r\n{}",
                                    e.to_string().len(),
                                    e
                                ),
                            }
                        }
                        _ => {
                            let pipeline = self.registry.get();
                            let modules = pipeline.modules();

                            if modules.is_empty() {
                                // Fallback to hardcoded
                                match (req.method, req.path) {
                                    (Some("GET"), Some("/")) => "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, Vortex".to_string(),
                                    (Some("POST"), Some("/echo")) => "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nEcho".to_string(),
                                    _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found".to_string(),
                                }
                            } else {
                                // Execute pipeline
                                match pipeline.run_request().await {
                                    Ok(response) => response,
                                    Err(e) => {
                                        tracing::error!("Pipeline error: {}", e);
                                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n".to_string()
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(httparse::Status::Partial) => {
                    "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".to_string()
                }
                Err(_) => "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".to_string(),
            };

            stream.write_all(response.as_bytes()).await?;
            stream.flush().await?;
        }
    }
}
