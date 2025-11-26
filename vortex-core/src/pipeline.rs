use crate::wasm::WasmModule;

#[derive(Clone)]
pub struct Pipeline {
    modules: Vec<WasmModule>,
}

impl Pipeline {
    pub fn new(modules: Vec<WasmModule>) -> Self {
        Self { modules }
    }

    #[tracing::instrument(skip(self))]
    pub async fn run_request(&self) -> anyhow::Result<String> {
        tracing::info!("Starting pipeline execution");
        let mut final_response = String::new();

        for module in &self.modules {
            match module.run_request().await {
                Ok(body) => {
                    // Check for specific control signals
                    if body == "403" {
                        final_response =
                            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n".to_string();
                        break;
                    }

                    // Check if body indicates an error status (hacky parsing for now)
                    if body.starts_with("HTTP/1.1 4") || body.starts_with("HTTP/1.1 5") {
                        final_response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                            body.len(),
                            body
                        );
                    } else {
                        final_response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                            body.len(),
                            body
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("Wasm error: {}", e);
                    final_response =
                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                            .to_string();
                    break;
                }
            }
        }
        Ok(final_response)
    }

    pub fn empty() -> Self {
        Self {
            modules: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn add(&mut self, module: WasmModule) {
        self.modules.push(module);
    }

    pub fn modules(&self) -> &[WasmModule] {
        &self.modules
    }
}
