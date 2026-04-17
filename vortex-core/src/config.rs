use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub rate_limit: RateLimitConfig,
    #[serde(default)]
    pub modules: ModulesConfig,
    pub opa: Option<OpaConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpaConfig {
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_workers")]
    pub workers: usize,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_max_requests")]
    pub max_requests: usize,
    #[serde(default = "default_window_seconds")]
    pub window_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulesConfig {
    #[serde(default)]
    pub pipeline: Vec<String>,
}

// Default values
fn default_port() -> u16 {
    8080
}
fn default_workers() -> usize {
    num_cpus::get()
}
fn default_max_requests() -> usize {
    10000
}
fn default_window_seconds() -> u64 {
    60
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                port: default_port(),
                workers: default_workers(),
                cert_path: None,
                key_path: None,
            },
            rate_limit: RateLimitConfig {
                max_requests: default_max_requests(),
                window_seconds: default_window_seconds(),
            },
            modules: ModulesConfig::default(),
            opa: None,
        }
    }
}

impl Config {
    /// Load config from file, with environment variable overrides
    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;

        Ok(config)
    }

    /// Load config from file if it exists, otherwise use defaults
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        let mut config = Self::load(path).unwrap_or_else(|e| {
            eprintln!("Warning: Could not load config ({}), using defaults", e);
            Self::default()
        });

        // Apply environment variable overrides
        if let Ok(port) = std::env::var("VORTEX_PORT") {
            if let Ok(p) = port.parse() {
                config.server.port = p;
            }
        }
        if let Ok(limit) = std::env::var("VORTEX_RATE_LIMIT") {
            if let Ok(l) = limit.parse() {
                config.rate_limit.max_requests = l;
            }
        }
        if let Ok(workers) = std::env::var("VORTEX_WORKERS") {
            if let Ok(w) = workers.parse() {
                config.server.workers = w;
            }
        }
        if let Ok(endpoint) = std::env::var("VORTEX_OPA_ENDPOINT") {
            config.opa = Some(OpaConfig { endpoint });
        }

        config
    }
}
