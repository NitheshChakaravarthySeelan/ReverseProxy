#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────
#  Test script for the Load Balancer & Proxy
#  Usage:  bash proxy/test.sh
# ──────────────────────────────────────────────

PROXY_DIR="$(cd "$(dirname "$0")" && pwd)"
PROXY_BIN="$PROXY_DIR/target/debug/proxy"

LPORT=18080
BPORT=18081
MPORT=9090

cleanup() {
  echo ""
  echo "=== Cleaning up ==="
  kill "$PROXY_PID" 2>/dev/null || true
  kill "$BACKEND_PID" 2>/dev/null || true
  wait 2>/dev/null || true
  rm -f /tmp/proxy_test.toml
  echo "Done."
}
trap cleanup EXIT

# ── Build ────────────────────────────────────
echo "=== Building proxy ==="
cargo build --manifest-path "$PROXY_DIR/Cargo.toml" 2>/dev/null

# ── Config ───────────────────────────────────
cat > /tmp/proxy_test.toml <<EOF
listen = "127.0.0.1:$LPORT"
backends = ["127.0.0.1:$BPORT"]
health_check_interval_secs = 30
health_check_path = "/health"
EOF

# ── Start backend ────────────────────────────
echo "=== Starting backend on :$BPORT ==="
python3 -m http.server "$BPORT" --bind 127.0.0.1 &>/dev/null &
BACKEND_PID=$!
sleep 1

# ── Start proxy ──────────────────────────────
echo "=== Starting proxy on :$LPORT ==="
PROXY_CONFIG=/tmp/proxy_test.toml RUST_LOG=error "$PROXY_BIN" &>/dev/null &
PROXY_PID=$!

# ── Wait for proxy to be ready ───────────────
echo -n "=== Waiting for proxy "
for i in $(seq 10); do
  if curl -sf --max-time 1 http://127.0.0.1:$LPORT -o /dev/null 2>/dev/null; then
    echo " (ready after ${i}s)"
    break
  fi
  sleep 1
  echo -n "."
done
echo ""

# ── Tests ────────────────────────────────────
echo ""
echo "──────────────────────────────────"
echo "  Test Results"
echo "──────────────────────────────────"

echo -n "  1. HTTP request via proxy    → "
STATUS=$(curl -s --max-time 3 -o /dev/null -w "%{http_code}" http://127.0.0.1:$LPORT || echo "000")
if [ "$STATUS" = "200" ]; then
  echo "PASS (HTTP $STATUS)"
else
  echo "FAIL (HTTP $STATUS)"
fi

echo -n "  2. Metrics endpoint          → "
if curl -sf --max-time 3 http://127.0.0.1:$MPORT/metrics -o /dev/null 2>/dev/null; then
  METRIC=$(curl -s --max-time 3 http://127.0.0.1:$MPORT/metrics | grep proxy_requests_total)
  echo "PASS ($METRIC)"
else
  echo "FAIL"
fi

echo -n "  3. Keep-alive (2nd request)  → "
STATUS2=$(curl -s --max-time 3 -o /dev/null -w "%{http_code}" http://127.0.0.1:$LPORT || echo "000")
if [ "$STATUS2" = "200" ]; then
  echo "PASS (HTTP $STATUS2)"
else
  echo "FAIL (HTTP $STATUS2)"
fi

echo -n "  4. Metrics counter           → "
COUNT=$(curl -s --max-time 3 http://127.0.0.1:$MPORT/metrics | grep proxy_requests_total | grep -oP '\d+$' || echo "0")
if [ "$COUNT" -ge 2 ]; then
  echo "PASS (count=$COUNT)"
else
  echo "FAIL (count=$COUNT)"
fi

echo "──────────────────────────────────"
echo ""
echo "All tests completed."
