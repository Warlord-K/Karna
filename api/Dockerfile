# syntax=docker/dockerfile:1
FROM rust:1.88-slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy full workspace source
COPY Cargo.toml Cargo.lock ./
COPY shared/ shared/
COPY api/ api/

# Agent stub (workspace member must exist but we don't need its real source)
COPY agent/Cargo.toml agent/Cargo.toml
RUN mkdir -p agent/src && echo "fn main() {}" > agent/src/main.rs

# Build with cache mounts — persists cargo registry + target dir across builds.
# Even if Docker layer cache misses, compiled deps survive.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    cargo build --release -p karna-api \
    && cp target/release/karna-api /karna-api

# Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /karna-api ./karna-api

ENV RUST_LOG=info
EXPOSE 8081

CMD ["./karna-api"]
