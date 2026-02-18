use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub rate_limit: RateLimitConfig,
    #[serde(default)]
    pub modules: ModulesConfig,
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
        }
    }
}

impl Config {
    /// Load config from file, with environment variable overrides
    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;

        // Apply environment variable overrides
        if let Ok(port) = std::env::var("VORTEX_PORT") {
            config.server.port = port.parse()?;
        }
        if let Ok(limit) = std::env::var("VORTEX_RATE_LIMIT") {
            config.rate_limit.max_requests = limit.parse()?;
        }
        if let Ok(workers) = std::env::var("VORTEX_WORKERS") {
            config.server.workers = workers.parse()?;
        }

        Ok(config)
    }

    /// Load config from file if it exists, otherwise use defaults
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        Self::load(path).unwrap_or_else(|e| {
            eprintln!("Warning: Could not load config ({}), using defaults", e);
            Self::default()
        })
    }
}
