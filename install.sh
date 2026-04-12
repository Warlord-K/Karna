#!/usr/bin/env bash
# Karna installer — interactive setup from zero to running.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Warlord-K/karna/main/install.sh | bash
#
# Or clone first and run locally:
#   ./install.sh

set -euo pipefail

# ── Helpers ──────────────────────────────────────────────────

BOLD='\033[1m'
DIM='\033[2m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${BOLD}${CYAN}==>${NC} $*"; }
ok()    { echo -e "${GREEN}  ✓${NC} $*"; }
warn()  { echo -e "${YELLOW}  !${NC} $*"; }
err()   { echo -e "${RED}  ✗${NC} $*"; }
ask()   { echo -en "${BOLD}${CYAN}  ?${NC} $* "; }

die() { err "$@"; exit 1; }

# Read a line of input, optionally with a default value.
# Usage: value=$(prompt "Label" "default")
prompt() {
    local label="$1" default="${2:-}"
    if [ -n "$default" ]; then
        ask "$label ${DIM}[$default]${NC}:"
    else
        ask "$label:"
    fi
    local input
    read -r input
    echo "${input:-$default}"
}

# Read a secret (no echo).
prompt_secret() {
    local label="$1"
    ask "$label:"
    local input
    read -rs input
    echo ""
    echo "$input"
}

# ── Banner ───────────────────────────────────────────────────

echo ""
echo -e "${BOLD}  Karna — Self-hosted autonomous coding agent${NC}"
echo -e "${DIM}  https://github.com/Warlord-K/karna${NC}"
echo ""

# ── Prerequisites ────────────────────────────────────────────

info "Checking prerequisites..."

missing=()
command -v git >/dev/null 2>&1 || missing+=("git")
command -v docker >/dev/null 2>&1 || missing+=("docker")

if [ ${#missing[@]} -gt 0 ]; then
    die "Missing required tools: ${missing[*]}. Install them and re-run."
fi

# Check docker compose (v2 plugin or standalone)
if docker compose version >/dev/null 2>&1; then
    ok "docker compose $(docker compose version --short)"
elif docker-compose version >/dev/null 2>&1; then
    die "Found docker-compose (v1) but Karna requires Docker Compose v2. Update Docker."
else
    die "docker compose not available. Install Docker Desktop or the compose plugin."
fi

# Check Docker daemon is running
if ! docker info >/dev/null 2>&1; then
    die "Docker daemon is not running. Start Docker and re-run."
fi

ok "git $(git --version | awk '{print $3}')"
ok "Docker $(docker --version | awk '{print $3}' | tr -d ',')"

# ── Clone ────────────────────────────────────────────────────

INSTALL_DIR="${KARNA_DIR:-$HOME/karna}"

echo ""
info "Where should Karna be installed?"
INSTALL_DIR=$(prompt "Install directory" "$INSTALL_DIR")

if [ -d "$INSTALL_DIR/.git" ]; then
    warn "Existing installation found at $INSTALL_DIR"
    ask "Overwrite config? (y/N):"
    read -r overwrite
    if [[ ! "$overwrite" =~ ^[Yy] ]]; then
        info "Keeping existing config. Run 'cd $INSTALL_DIR && ./karna start' to start."
        exit 0
    fi
    cd "$INSTALL_DIR"
elif [ -d "$INSTALL_DIR" ] && [ "$(ls -A "$INSTALL_DIR" 2>/dev/null)" ]; then
    die "$INSTALL_DIR exists and is not empty. Remove it or choose a different path."
else
    info "Cloning Karna..."
    git clone --depth 1 https://github.com/Warlord-K/karna.git "$INSTALL_DIR"
    cd "$INSTALL_DIR"
    ok "Cloned to $INSTALL_DIR"
fi

echo ""

# ── Required Tokens ──────────────────────────────────────────

info "Required: GitHub Personal Access Token"
echo -e "  ${DIM}Create one at https://github.com/settings/tokens${NC}"
echo -e "  ${DIM}Scopes needed: repo, workflow${NC}"
echo ""
GITHUB_TOKEN=$(prompt_secret "GitHub token (ghp_... or github_pat_...)")

if [ -z "$GITHUB_TOKEN" ]; then
    die "GitHub token is required."
fi

# Validate token format
if [[ ! "$GITHUB_TOKEN" =~ ^(ghp_|github_pat_) ]]; then
    warn "Token doesn't look like a GitHub PAT. Continuing anyway."
fi

echo ""
info "Required: Claude Code OAuth Token"
echo -e "  ${DIM}Install Claude Code:  npm install -g @anthropic-ai/claude-code${NC}"
echo -e "  ${DIM}Generate token:       claude setup-token${NC}"
echo ""
CLAUDE_CODE_OAUTH_TOKEN=$(prompt_secret "Claude Code OAuth token")

if [ -z "$CLAUDE_CODE_OAUTH_TOKEN" ]; then
    die "Claude Code OAuth token is required."
fi

# Generate AUTH_SECRET
AUTH_SECRET=$(openssl rand -hex 32 2>/dev/null || head -c 32 /dev/urandom | xxd -p -c 64 2>/dev/null || date +%s%N | sha256sum | head -c 64)

echo ""

# ── Repos ────────────────────────────────────────────────────

info "Add repositories for the agent to work on"
echo -e "  ${DIM}Format: owner/repo (e.g. acme/backend)${NC}"
echo -e "  ${DIM}Press Enter with empty input when done.${NC}"
echo ""

repos=()
while true; do
    repo=$(prompt "Repository" "")
    [ -z "$repo" ] && break

    # Basic format check
    if [[ ! "$repo" =~ ^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$ ]]; then
        warn "Doesn't look like owner/repo format. Skipping."
        continue
    fi

    branch=$(prompt "  Branch" "main")
    repos+=("$repo:$branch")
    ok "Added $repo ($branch)"
    echo ""
done

if [ ${#repos[@]} -eq 0 ]; then
    warn "No repos added. You can add them later in config.yaml."
fi

echo ""

# ── Optional Features ────────────────────────────────────────

info "Optional features"
echo ""

# Auth
ask "Disable authentication? (single-user mode) (y/N):"
read -r auth_disabled
AUTH_DISABLED="false"
if [[ "$auth_disabled" =~ ^[Yy] ]]; then
    AUTH_DISABLED="true"
    ok "Auth disabled — no login required"
else
    ok "Auth enabled — create an account at first login"
fi

echo ""

# Git identity
GIT_AUTHOR_NAME=$(prompt "Git commit author name" "Karna Agent")
GIT_AUTHOR_EMAIL=$(prompt "Git commit author email" "agent@karna.dev")

echo ""

# Codex backend
ask "Enable OpenAI Codex backend? (y/N):"
read -r enable_codex
ENABLE_CODEX=false
if [[ "$enable_codex" =~ ^[Yy] ]]; then
    ENABLE_CODEX=true
    echo -e "  ${DIM}Run 'npm install -g @openai/codex && codex login' before starting.${NC}"
    echo -e "  ${DIM}Credentials in ~/.codex are mounted automatically.${NC}"
    ok "Codex backend enabled"
fi

echo ""

# Email notifications
ask "Set up email notifications via Resend? (y/N):"
read -r enable_email
RESEND_API_KEY=""
NOTIFICATION_EMAIL=""
if [[ "$enable_email" =~ ^[Yy] ]]; then
    RESEND_API_KEY=$(prompt_secret "Resend API key (re_...)")
    NOTIFICATION_EMAIL=$(prompt "Notification email address" "")
    ok "Email notifications configured"
fi

echo ""

# Code-server password
CODE_SERVER_PASSWORD=$(prompt "Code-server password (browser IDE)" "changeme")

echo ""

# ── Write .env ───────────────────────────────────────────────

info "Writing .env..."

cat > .env << ENVEOF
# Generated by Karna installer on $(date -u +%Y-%m-%dT%H:%M:%SZ)

# Required
GITHUB_TOKEN=$GITHUB_TOKEN
CLAUDE_CODE_OAUTH_TOKEN=$CLAUDE_CODE_OAUTH_TOKEN
AUTH_SECRET=$AUTH_SECRET

# Auth
AUTH_DISABLED=$AUTH_DISABLED

# Git identity
GIT_AUTHOR_NAME=$GIT_AUTHOR_NAME
GIT_AUTHOR_EMAIL=$GIT_AUTHOR_EMAIL

# Passwords
CODE_SERVER_PASSWORD=$CODE_SERVER_PASSWORD

# Ports
FRONTEND_PORT=3000
CODE_SERVER_PORT=8443
POSTGRES_PORT=5432
REDIS_PORT=6379
POSTGRES_PASSWORD=karna
ENVEOF

if [ -n "$RESEND_API_KEY" ]; then
    echo "" >> .env
    echo "# Email notifications" >> .env
    echo "RESEND_API_KEY=$RESEND_API_KEY" >> .env
fi

ok ".env written"

# ── Write config.yaml ────────────────────────────────────────

info "Writing config.yaml..."

cat > config.yaml << 'CFGEOF'
# Generated by Karna installer

repos:
CFGEOF

if [ ${#repos[@]} -eq 0 ]; then
    cat >> config.yaml << 'CFGEOF'
  # Add repos here:
  # - repo: owner/repo
  #   branch: main
CFGEOF
else
    for entry in "${repos[@]}"; do
        repo="${entry%%:*}"
        branch="${entry##*:}"
        echo "  - repo: $repo" >> config.yaml
        echo "    branch: $branch" >> config.yaml
    done
fi

cat >> config.yaml << CFGEOF

agent:
  max_turns: 100
  poll_interval_secs: 30
  max_concurrent_tasks: 1

  backends:
    claude:
      models: [opus, sonnet, haiku]
      default_model: sonnet
CFGEOF

if [ "$ENABLE_CODEX" = true ]; then
    cat >> config.yaml << 'CFGEOF'
    codex:
      models: [gpt-5.4, gpt-5.4-mini, gpt-5.3-codex]
      default_model: gpt-5.4
CFGEOF
fi

if [ -n "$NOTIFICATION_EMAIL" ]; then
    cat >> config.yaml << CFGEOF

notifications:
  email: $NOTIFICATION_EMAIL
CFGEOF
fi

cat >> config.yaml << 'CFGEOF'

mcp_servers:
  - name: fetch
    command: npx
    args: ["-y", "@modelcontextprotocol/server-fetch"]

  - name: context7
    command: npx
    args: ["-y", "@upstash/context7-mcp"]

  - name: memory
    command: npx
    args: ["-y", "@modelcontextprotocol/server-memory"]

  - name: sequential-thinking
    command: npx
    args: ["-y", "@modelcontextprotocol/server-sequential-thinking"]

  - name: github
    command: npx
    args: ["-y", "@modelcontextprotocol/server-github"]
    env:
      GITHUB_PERSONAL_ACCESS_TOKEN: "${GITHUB_TOKEN}"
CFGEOF

ok "config.yaml written"

echo ""

# ── Start ────────────────────────────────────────────────────

info "Starting Karna..."
echo ""

docker compose up -d

echo ""
ok "Karna is running!"
echo ""
echo -e "  ${BOLD}Task board:${NC}    http://localhost:3000"
echo -e "  ${BOLD}Agent API:${NC}     http://localhost:8080"
echo -e "  ${BOLD}Browser IDE:${NC}   http://localhost:8443  ${DIM}(password: $CODE_SERVER_PASSWORD)${NC}"
echo ""
echo -e "  ${DIM}Manage:        cd $INSTALL_DIR && ./karna status${NC}"
echo -e "  ${DIM}View logs:     ./karna logs${NC}"
echo -e "  ${DIM}Stop:          ./karna stop${NC}"
echo ""

if [ "$AUTH_DISABLED" = "true" ]; then
    echo -e "  Auth is disabled — you'll be logged in automatically."
else
    echo -e "  Open the task board and create your first account."
fi

echo ""
