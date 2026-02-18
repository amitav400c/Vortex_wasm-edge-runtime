#!/bin/bash
set -e

# Create output directory if it doesn't exist (relative to repo root)
mkdir -p vortex-core

echo "Generating self-signed TLS certificates..."
openssl req -x509 -newkey rsa:2048 \
  -keyout vortex-core/key.pem \
  -out vortex-core/cert.pem \
  -days 365 -nodes \
  -subj '/CN=localhost'

echo "Certificates generated at vortex-core/cert.pem and vortex-core/key.pem"
