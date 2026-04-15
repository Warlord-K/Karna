# Build Rust binary
FROM rust:1.88-slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests for dependency caching
COPY Cargo.toml Cargo.lock* ./
COPY shared/Cargo.toml shared/Cargo.toml
COPY agent/Cargo.toml agent/Cargo.toml
COPY api/Cargo.toml api/Cargo.toml

# Stubs for dependency caching
RUN mkdir -p shared/src agent/src api/src \
    && echo "pub mod db; pub mod models;" > shared/src/lib.rs \
    && echo "pub fn stub() {}" > shared/src/db.rs \
    && echo "" > shared/src/models.rs \
    && echo "fn main() {}" > agent/src/main.rs \
    && echo "fn main() {}" > api/src/main.rs
RUN cargo build --release -p karna-agent 2>/dev/null || true

# Copy real source and build
COPY shared/src shared/src
COPY agent/src agent/src
COPY agent/templates agent/templates
RUN touch agent/src/main.rs && cargo build --release -p karna-agent

# Runtime image
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates git curl jq openssh-client \
    dbus gnome-keyring libsecret-1-0 \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js (for Claude Code CLI + MCP servers)
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Install GitHub CLI
RUN ARCH=$(dpkg --print-architecture) \
    && GH_VERSION=$(curl -fsSL https://api.github.com/repos/cli/cli/releases/latest | jq -r .tag_name | tr -d v) \
    && curl -fsSL "https://github.com/cli/cli/releases/download/v${GH_VERSION}/gh_${GH_VERSION}_linux_${ARCH}.tar.gz" \
    | tar -xz --strip-components=1 -C /usr/local

# Install Claude Code CLI + Codex CLI
RUN npm install -g @anthropic-ai/claude-code @openai/codex

# safe.directory is static, set at build time
RUN git config --global safe.directory "*"

# Create non-root user (Claude Code refuses --dangerously-skip-permissions as root)
RUN useradd -m -s /bin/bash agent

WORKDIR /app
COPY --from=builder /build/target/release/karna-agent ./
COPY agent/templates ./templates

# Create workspace dirs, keyring dir, codex config dir, and .ssh dir owned by agent user
RUN mkdir -p /workspace /workspace/.workspaces /home/agent/.local/share/keyrings /home/agent/.codex /home/agent/.ssh \
    && chmod 700 /home/agent/.ssh \
    && chown -R agent:agent /workspace /app /home/agent/.local /home/agent/.codex /home/agent/.ssh

USER agent

ENV RUST_LOG=info

# Git identity configured at runtime from env vars
CMD git config --global user.name "${GIT_AUTHOR_NAME:-Karna Agent}" \
    && git config --global user.email "${GIT_AUTHOR_EMAIL:-agent@karna.dev}" \
    && exec dbus-run-session -- sh -c 'echo "" | gnome-keyring-daemon --unlock --components=secrets && exec ./karna-agent'
