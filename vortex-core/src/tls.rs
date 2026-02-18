use std::fs::File;
use std::io::BufReader;
// use std::path::Path; - unused
use std::sync::Arc;

use anyhow::{Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;

pub fn load_server_config(cert_path: &str, key_path: &str) -> Result<Arc<ServerConfig>> {
    let certs = load_certs(cert_path)?;
    let key = load_private_key(key_path)?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("Failed to build TLS server config")?;

    Ok(Arc::new(config))
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path).with_context(|| format!("Failed to open cert file: {}", path))?;
    let mut reader = BufReader::new(file);
    
    // rustls-pemfile 2.0 returns an iterator of Results
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse certificates")?;
        
    Ok(certs)
}

fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>> {
    let file = File::open(path).with_context(|| format!("Failed to open key file: {}", path))?;
    let mut reader = BufReader::new(file);

    // Try to parse private key (PKCS8, RSA, or Sec1)
    // rustls-pemfile 2.0 provides centralized parsing
    loop {
        match rustls_pemfile::read_one(&mut reader).context("Failed to parse private key")? {
            Some(rustls_pemfile::Item::Pkcs8Key(key)) => return Ok(PrivateKeyDer::Pkcs8(key)),
            Some(rustls_pemfile::Item::Pkcs1Key(key)) => return Ok(PrivateKeyDer::Pkcs1(key)),
            Some(rustls_pemfile::Item::Sec1Key(key)) => return Ok(PrivateKeyDer::Sec1(key)),
            Some(_) => continue, // Skip non-key items (like certificates in same file)
            None => break,
        }
    }

    Err(anyhow::anyhow!("No valid private key found in {}", path))
}

#[cfg(test)]
mod tests {
    use super::*;
    // use std::io::Write; - unused
    // use std::env; - unused

    #[test]
    fn test_load_server_config_missing_file() {
        let result = load_server_config("non_existent_cert.pem", "non_existent_key.pem");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to open cert file"));
    }
}
