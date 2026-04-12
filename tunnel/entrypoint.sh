#!/bin/sh
set -e

# Support two modes:
#   1. Token-based (remotely managed) — routes configured in CF dashboard
#   2. Credentials-based (locally managed) — routes defined here via env vars

if [ -n "$CLOUDFLARE_TUNNEL_TOKEN" ]; then
  echo "[tunnel] Running with remotely-managed token (routes configured in CF dashboard)"
  exec cloudflared tunnel run --token "$CLOUDFLARE_TUNNEL_TOKEN"
fi

# Locally-managed mode — need tunnel ID + credentials
if [ -z "$CLOUDFLARE_TUNNEL_ID" ] || [ -z "$CLOUDFLARE_TUNNEL_CREDENTIALS" ]; then
  echo "[tunnel] No Cloudflare credentials configured, skipping tunnel"
  exit 0
fi

# Write credentials file from base64 env var
echo "$CLOUDFLARE_TUNNEL_CREDENTIALS" | base64 -d > /tmp/tunnel-creds.json

# Build ingress rules from env vars
FRONTEND_HOST="${TUNNEL_FRONTEND_HOSTNAME:-}"
CODE_HOST="${TUNNEL_CODE_SERVER_HOSTNAME:-}"

cat > /tmp/cloudflared.yml <<YAML
tunnel: ${CLOUDFLARE_TUNNEL_ID}
credentials-file: /tmp/tunnel-creds.json

ingress:
YAML

if [ -n "$FRONTEND_HOST" ]; then
  cat >> /tmp/cloudflared.yml <<YAML
  - hostname: ${FRONTEND_HOST}
    service: http://frontend:3000
YAML
  echo "[tunnel] ${FRONTEND_HOST} -> frontend:3000"
fi

if [ -n "$CODE_HOST" ]; then
  cat >> /tmp/cloudflared.yml <<YAML
  - hostname: ${CODE_HOST}
    service: http://code-server:8080
YAML
  echo "[tunnel] ${CODE_HOST} -> code-server:8080"
fi

AGENT_HOST="${TUNNEL_AGENT_HOSTNAME:-}"
if [ -n "$AGENT_HOST" ]; then
  cat >> /tmp/cloudflared.yml <<YAML
  - hostname: ${AGENT_HOST}
    service: http://agent:8080
YAML
  echo "[tunnel] ${AGENT_HOST} -> agent:8080"
fi

# Catch-all (required by cloudflared)
cat >> /tmp/cloudflared.yml <<YAML
  - service: http_status:404
YAML

echo "[tunnel] Config:"
cat /tmp/cloudflared.yml

exec cloudflared tunnel --config /tmp/cloudflared.yml run
