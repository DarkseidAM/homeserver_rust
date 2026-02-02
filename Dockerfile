# Multi-stage build for Rust homeserver

FROM rust:1.93 AS builder

WORKDIR /build

# Copy Cargo files for dependency caching
COPY Cargo.toml Cargo.lock* ./
# Create dummy main to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release || true

# Copy source and rebuild
COPY src ./src
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m serveruser

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/target/release/homeserver /app/homeserver
# Copy default config
COPY config.toml /app/config.toml

RUN mkdir -p /app/data /app/config && chown -R serveruser:serveruser /app

USER serveruser

ENTRYPOINT ["/app/homeserver"]
