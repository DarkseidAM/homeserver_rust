# Multi-stage build for Rust homeserver

FROM rust:1.93-slim AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source and build (single stage; dummy-build cache was causing bin to link stale lib)
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY examples ./examples
RUN cargo build --release --locked && \
    strip target/release/homeserver

# Runtime stage - minimal Debian image
FROM debian:bookworm-slim

# Install runtime deps + tini (init for PID 1 so server doesn't get spurious signals)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tini \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -u 1000 -m -s /sbin/nologin serveruser

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/target/release/homeserver /app/homeserver
# Copy default config
COPY config.toml /app/config.toml

# Create data directory with proper ownership
RUN mkdir -p /app/data && chown -R serveruser:serveruser /app

USER serveruser

ENTRYPOINT ["/usr/bin/tini", "--", "/app/homeserver"]
