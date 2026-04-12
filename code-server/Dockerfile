FROM codercom/code-server:latest

USER root

# jq for JSON, openssh for git operations
RUN apt-get update && apt-get install -y --no-install-recommends \
    jq openssh-client curl \
    && rm -rf /var/lib/apt/lists/*

# Node.js 22 (mirrors agent/Dockerfile — needed for Claude Code CLI, Codex CLI, MCP servers)
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# GitHub CLI
RUN ARCH=$(dpkg --print-architecture) \
    && GH_VERSION=$(curl -fsSL https://api.github.com/repos/cli/cli/releases/latest | jq -r .tag_name | tr -d v) \
    && curl -fsSL "https://github.com/cli/cli/releases/download/v${GH_VERSION}/gh_${GH_VERSION}_linux_${ARCH}.tar.gz" \
    | tar -xz --strip-components=1 -C /usr/local

# yq for YAML parsing in entrypoint
RUN ARCH=$(dpkg --print-architecture) \
    && curl -fsSL "https://github.com/mikefarah/yq/releases/latest/download/yq_linux_${ARCH}" \
    -o /usr/local/bin/yq \
    && chmod +x /usr/local/bin/yq

# Rust toolchain (system-wide so rust-analyzer can find cargo/rustc)
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable \
    && chmod -R a+rX /usr/local/rustup /usr/local/cargo

# Claude Code CLI + Codex CLI
RUN npm install -g @anthropic-ai/claude-code @openai/codex

# Use VS Code Marketplace instead of Open VSX (needed for anthropic.claude-code)
ENV EXTENSIONS_GALLERY='{"serviceUrl":"https://marketplace.visualstudio.com/_apis/public/gallery","itemUrl":"https://marketplace.visualstudio.com/items"}'

# Allow git operations in any workspace directory
RUN git config --system safe.directory "*"

# Pre-create code-server data dirs so volume mounts don't break permissions
RUN mkdir -p /home/coder/.local/share/code-server/extensions \
             /home/coder/.local/share/code-server/coder-logs \
             /home/coder/.local/share/code-server/User \
    && chown -R 1000:1000 /home/coder/.local

# Setup script runs before code-server via the stock entrypoint.d mechanism
COPY setup.sh /entrypoint.d/01-karna-setup.sh
RUN chmod +x /entrypoint.d/01-karna-setup.sh

USER 1000
