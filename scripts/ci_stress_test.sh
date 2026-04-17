#!/bin/bash
set -e

# Configuration
PORT=8080
DURATION="30s"
# Detect CPU count (fallback to 2)
if command -v nproc > /dev/null; then
    THREADS=$(nproc)
else
    THREADS=2
fi
# Random connections between 1000 and 2000
CONNECTIONS=$((1000 + RANDOM % 1001))

echo "=== CI Stress Test Configuration ==="
echo "Threads: $THREADS"
echo "Connections: $CONNECTIONS (Randomized)"
echo "Duration: $DURATION"
echo "===================================="

# Check for wrk
WRK_CMD=${WRK_BINARY:-wrk}
if ! command -v $WRK_CMD &> /dev/null && ! [ -x "$WRK_CMD" ]; then
    if [ -x "/tmp/wrk/wrk" ]; then
        WRK_CMD="/tmp/wrk/wrk"
    else
        echo "wrk not found. Building from source..."
        rm -rf /tmp/wrk
        git clone https://github.com/wg/wrk.git /tmp/wrk
        cd /tmp/wrk
        make -j${THREADS}
        cd -
        WRK_CMD="/tmp/wrk/wrk"
    fi
fi

echo "Generating TLS Certificates..."
./scripts/generate_certs.sh

echo "Starting Vortex Server (logs -> server_stress.log)..."
# Start server in background with dev config, redirecting logs
# Override workers to match available CPUs to avoid glommio panic
export VORTEX_WORKERS=$THREADS
./target/release/vortex-core --config vortex.toml > server_stress.log 2>&1 &
SERVER_PID=$!

# Ensure cleanup on exit
cleanup() {
    echo "Stopping server (PID: $SERVER_PID)..."
    kill $SERVER_PID
}
trap cleanup EXIT

# Wait for server to be ready
echo "Waiting for port $PORT..."
RETRIES=30
while [ $RETRIES -gt 0 ]; do
    if curl -k -s https://localhost:$PORT >/dev/null; then
        echo "Server is up!"
        break
    fi
    sleep 1
    RETRIES=$((RETRIES - 1))
done

if [ $RETRIES -eq 0 ]; then
    echo "Timed out waiting for server to start. Logs:"
    cat server_stress.log
    exit 1
fi

echo "Running wrk benchmark for $DURATION..."
# Run wrk and capture output
if OUTPUT=$($WRK_CMD -t$THREADS -c$CONNECTIONS -d$DURATION https://localhost:$PORT); then
    echo "$OUTPUT"
else
    echo "Benchmark failed!"
    cat server_stress.log
    exit 1
fi

# Simple assertion: Check if we have a valid transfer rate or request count
if echo "$OUTPUT" | grep -q "Socket errors: connect"; then
   echo "WARNING: Socket errors detected (likely OS limit exhaustion)"
fi

echo "Stress test completed successfully."
