mod config;
mod crdt;
mod opa;
mod parsing_test;
mod pipeline;
mod server;
#[cfg(test)]
mod server_test;
pub mod simd_parser; // Public for benchmarks
mod tls;
mod wasm;
use clap::Parser;
use glommio::{LocalExecutorBuilder, Placement};
use server::{ModuleRegistry, Server};
use wasm::WasmEngine;

#[derive(Parser, Debug)]
#[command(name = "vortex")]
#[command(about = "High-performance WebAssembly runtime", long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "vortex.toml")]
    config: String,

    /// Module paths to load (overrides config)
    #[arg(trailing_var_arg = true)]
    modules: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    // Initialize Tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Parse CLI arguments
    let args = Args::parse();

    // Load configuration
    let config = config::Config::load_or_default(&args.config);

    let cpu_count = config.server.workers;
    println!("Starting Vortex on {} cores", cpu_count);
    println!(
        "Config: port={}, rate_limit={}",
        config.server.port, config.rate_limit.max_requests
    );

    let engine = WasmEngine::new().expect("Failed to initialize Wasm engine");
    let registry = ModuleRegistry::new(engine);

    // Initialize CRDT Rate Limiter
    let rate_limiter = std::sync::Arc::new(crdt::GCounter::new(cpu_count));

    // Load TLS Config if available
    let tls_config =
        if let (Some(cert), Some(key)) = (&config.server.cert_path, &config.server.key_path) {
            println!("Loading TLS config from {} and {}", cert, key);
            Some(tls::load_server_config(cert, key).expect("Failed to load TLS config"))
        } else {
            None
        };

    // Determine which modules to load (CLI overrides config)
    let module_paths = if !args.modules.is_empty() {
        args.modules
    } else {
        config.modules.pipeline.clone()
    };

    if !module_paths.is_empty() {
        println!("Loading pipeline with modules: {:?}", module_paths);
        registry
            .load_pipeline(&module_paths)
            .expect("Failed to load pipeline");
    }

    // Initialize eBPF (Optional, requires root)
    #[cfg(feature = "bpf")]
    {
        println!("Initializing eBPF...");
        if let Err(e) = init_bpf() {
            eprintln!("eBPF Warning: Failed to initialize: {}", e);
            eprintln!("Continuing without eBPF stats...");
        }
    }

    let port = config.server.port;
    let max_requests = config.rate_limit.max_requests;

    let handles: Vec<_> = (0..cpu_count)
        .map(|i| {
            let registry = registry.clone();
            let rate_limiter = rate_limiter.clone();
            let tls_config = tls_config.clone();
            let opa_config = config.opa.clone();

            std::thread::spawn(move || {
                let builder =
                    LocalExecutorBuilder::new(Placement::Fixed(i)).name(&format!("executor-{}", i));

                builder
                    .spawn(move || async move {
                        let server = Server::new(
                            port,
                            registry,
                            rate_limiter,
                            i,
                            max_requests,
                            tls_config,
                            opa_config,
                        );
                        server.run().await
                    })
                    .expect("failed to spawn executor")
                    .join()
                    .expect("executor failed")
                    .expect("server failed");
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    Ok(())
}

#[cfg(feature = "bpf")]
fn init_bpf() -> anyhow::Result<()> {
    use aya::programs::{Xdp, XdpFlags};
    use aya::Ebpf;
    use std::path::Path;

    let path = Path::new("vortex-bpf/target/bpfel-unknown-none/release/vortex-bpf");
    if !path.exists() {
        return Err(anyhow::anyhow!("eBPF object not found at {:?}", path));
    }

    let mut bpf = Ebpf::load_file(path)?;

    // Load XDP program
    let program: &mut Xdp = bpf.program_mut("vortex_xdp").unwrap().try_into()?;
    program.load()?;

    // Attach to loopback (lo) for testing
    // In production, this would be eth0 or similar
    program.attach("lo", XdpFlags::default())?;

    println!("eBPF XDP program attached to 'lo'");

    // Leak BPF to keep it running
    std::mem::forget(bpf);

    Ok(())
}
