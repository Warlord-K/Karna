# Build stage
FROM rust:1.88-slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests for dependency caching
COPY Cargo.toml Cargo.lock* ./
COPY shared/Cargo.toml shared/Cargo.toml
COPY api/Cargo.toml api/Cargo.toml
COPY agent/Cargo.toml agent/Cargo.toml

# Stubs for dependency caching
RUN mkdir -p shared/src api/src agent/src \
    && echo "pub mod db; pub mod models;" > shared/src/lib.rs \
    && echo "pub fn stub() {}" > shared/src/db.rs \
    && echo "" > shared/src/models.rs \
    && echo "fn main() {}" > api/src/main.rs \
    && echo "fn main() {}" > agent/src/main.rs
RUN cargo build --release -p karna-api 2>/dev/null || true

# Copy real source
COPY shared/src shared/src
COPY api/src api/src
RUN touch api/src/main.rs && cargo build --release -p karna-api

# Runtime — minimal image, no heavy deps needed
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/karna-api ./

ENV RUST_LOG=info
EXPOSE 8081

CMD ["./karna-api"]
