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
RUN cargo chef cook --release --recipe-path recipe.json -p karna-api
COPY . .
RUN cargo build --release -p karna-api \
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
