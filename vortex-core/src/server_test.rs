#[cfg(test)]
mod tests {
    use crate::server::{ModuleRegistry, Server};
    use crate::wasm::WasmEngine;
    use glommio::{LocalExecutorBuilder, Placement};

    #[test]
    fn test_wasm_execution() {
        let builder = LocalExecutorBuilder::new(Placement::Unbound);
        let handle = builder
            .spawn(|| async move {
                let engine = WasmEngine::new().unwrap();
                let registry = ModuleRegistry::new(engine);
                let rate_limiter = std::sync::Arc::new(crate::crdt::GCounter::new(1));
                let _server = Server::new(8081, registry.clone(), rate_limiter, 0, 100, None);

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
