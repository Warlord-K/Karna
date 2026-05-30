# syntax=docker/dockerfile:1
FROM rust:1.88-slim-bookworm AS chef
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install cargo-chef --locked

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
# Compile dependencies — cached layer (keyed on recipe.json = Cargo.toml/Cargo.lock).
RUN cargo chef cook --release --recipe-path recipe.json -p karna-agent
COPY . .
RUN cargo build --release -p karna-agent \
    && cp target/release/karna-agent /karna-agent

# Runtime image
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates git curl jq openssh-client \
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

# Install opencode (OpenRouter-friendly CLI — Kimi K2, DeepSeek, Qwen, etc.)
# Env var must be set on the bash invocation, not curl — the installer reads it in bash.
RUN curl -fsSL https://opencode.ai/install | OPENCODE_INSTALL_DIR=/usr/local/bin bash

# safe.directory is static, set at build time
RUN git config --global safe.directory "*"

# Create non-root user (Claude Code refuses --dangerously-skip-permissions as root).
# uid=1000 is pinned so the chart's fsGroup=1000 lines up reliably.
RUN useradd -u 1000 -m -s /bin/bash agent

WORKDIR /app
COPY --from=builder /karna-agent ./karna-agent
COPY agent/templates ./templates

# Create workspace dirs, codex config dir, and .ssh dir owned by agent user
RUN mkdir -p /workspace /workspace/.workspaces /home/agent/.codex /home/agent/.ssh \
    && chmod 700 /home/agent/.ssh \
    && chown -R agent:agent /workspace /app /home/agent/.codex /home/agent/.ssh

USER agent

ENV RUST_LOG=info

# Exec form — karna-agent is PID 1 so SIGTERM hits it directly (no shell wrapper).
# Git identity is configured by configure_git_auth() at startup.
CMD ["./karna-agent"]
