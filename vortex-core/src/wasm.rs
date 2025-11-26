use anyhow::Result;
use wasmtime::{
    Caller, Config, Engine, InstanceAllocationStrategy, Linker, Module, PoolingAllocationConfig,
    Store,
};
use wasmtime_wasi::preview1::WasiP1Ctx;
use wasmtime_wasi::WasiCtxBuilder;

#[derive(Clone)]
pub struct WasmEngine {
    engine: Engine,
}

struct WasmContext {
    wasi: WasiP1Ctx,
}

impl WasmEngine {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.async_support(true);
        config.consume_fuel(true);

        // Enable Pooling Allocator
        let mut pooling_config = PoolingAllocationConfig::default();
        pooling_config.max_unused_warm_slots(100); // Keep warm instances ready
        config.allocation_strategy(InstanceAllocationStrategy::Pooling(pooling_config));

        let engine = Engine::new(&config)?;
        Ok(Self { engine })
    }

    fn create_linker(engine: &Engine) -> Result<Linker<WasmContext>> {
        let mut linker = Linker::new(engine);
        wasmtime_wasi::preview1::add_to_linker_async(&mut linker, |ctx: &mut WasmContext| {
            &mut ctx.wasi
        })?;

        // Define env.log host function
        linker.func_wrap(
            "env",
            "log",
            |mut caller: Caller<'_, WasmContext>, ptr: i32, len: i32| {
                let mem = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(mem)) => mem,
                    _ => return,
                };

                let data = mem.data(&caller);
                let offset = ptr as usize;
                let end = offset + len as usize;

                if end <= data.len() {
                    let msg = String::from_utf8_lossy(&data[offset..end]);
                    println!("[Wasm Log]: {}", msg);
                }
            },
        )?;

        // Define env.fetch host function (mock)
        linker.func_wrap(
            "env",
            "fetch",
            |mut caller: Caller<'_, WasmContext>, ptr: i32, len: i32| -> i32 {
                let mem = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(mem)) => mem,
                    _ => return 500,
                };

                let data = mem.data(&caller);
                let offset = ptr as usize;
                let end = offset + len as usize;

                if end <= data.len() {
                    let url = String::from_utf8_lossy(&data[offset..end]);
                    println!("[Host Fetch]: {}", url);
                    return 200; // Mock 200 OK
                }
                400 // Bad Request
            },
        )?;
        Ok(linker)
    }

    pub fn load_module(&self, path: &str) -> Result<WasmModule> {
        let module = Module::from_file(&self.engine, path)?;
        let linker = Self::create_linker(&self.engine)?;
        let instance_pre = linker.instantiate_pre(&module)?;
        Ok(WasmModule {
            engine: self.engine.clone(),
            instance_pre,
        })
    }

    pub fn load_module_from_bytes(&self, bytes: &[u8]) -> Result<WasmModule> {
        let module = Module::new(&self.engine, bytes)?;
        let linker = Self::create_linker(&self.engine)?;
        let instance_pre = linker.instantiate_pre(&module)?;
        Ok(WasmModule {
            engine: self.engine.clone(),
            instance_pre,
        })
    }

    // For testing with WAT
    #[allow(dead_code)]
    pub fn load_module_from_wat(&self, wat: &str) -> Result<WasmModule> {
        let module = Module::new(&self.engine, wat)?;
        let linker = Self::create_linker(&self.engine)?;
        let instance_pre = linker.instantiate_pre(&module)?;
        Ok(WasmModule {
            engine: self.engine.clone(),
            instance_pre,
        })
    }
}

#[derive(Clone)]
pub struct WasmModule {
    engine: Engine,
    instance_pre: wasmtime::InstancePre<WasmContext>,
}

impl WasmModule {
    #[tracing::instrument(skip(self))]
    pub async fn run_request(&self) -> anyhow::Result<String> {
        tracing::debug!("Running Wasm module");

        let wasi = WasiCtxBuilder::new().inherit_stdio().build_p1();

        let mut store = Store::new(&self.engine, WasmContext { wasi });

        // Add fuel (e.g. 10,000,000 units)
        store.set_fuel(10_000_000)?;

        let instance = self.instance_pre.instantiate_async(&mut store).await?;

        // Check if handle_request exists, otherwise just return empty (for WASI tests that might just run start)
        let handle_request = match instance.get_typed_func::<(), i32>(&mut store, "handle_request")
        {
            Ok(func) => func,
            Err(_) => return Ok("No handle_request exported".to_string()),
        };

        // Wrap execution in timeout (e.g. 100ms)
        // We use glommio::timer::sleep for timeout
        use futures_lite::future::FutureExt;

        let execution = async {
            let ptr = handle_request.call_async(&mut store, ()).await?;
            Ok(ptr)
        };

        let timeout = async {
            glommio::timer::sleep(std::time::Duration::from_millis(100)).await;
            Err(anyhow::anyhow!("Execution timed out"))
        };

        // Race execution and timeout
        let ptr = execution.or(timeout).await?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow::anyhow!("memory not exported"))?;

        // Read string from memory at ptr
        let mut buffer = Vec::new();
        let mut offset = ptr as usize;
        let data = memory.data(&store);

        loop {
            if offset >= data.len() {
                break;
            }
            let byte = data[offset];
            if byte == 0 {
                break;
            }
            buffer.push(byte);
            offset += 1;
        }

        let s = String::from_utf8(buffer).unwrap_or_else(|_| "Invalid UTF-8".to_string());
        Ok(s)
    }
}
