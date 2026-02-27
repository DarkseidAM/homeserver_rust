# Multi-stage build for Rust homeserver

FROM rust:1.93-slim AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    make \
    && rm -rf /var/lib/apt/lists/*

# Copy source and build (single stage; dummy-build cache was causing bin to link stale lib)
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY examples ./examples
RUN cargo build --release --locked && \
    strip target/release/homeserver

# Runtime stage - minimal Debian image
FROM debian:13.3-slim

# Install runtime deps + tini (init for PID 1 so server doesn't get spurious signals)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tini \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/target/release/homeserver /app/homeserver
# Copy default config
COPY config.toml /app/config.toml
# Copy entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

# Create data directory (ownership will be fixed by entrypoint)
RUN mkdir -p /app/data

# Make entrypoint executable
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Run as root initially - entrypoint will switch to correct user
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["/app/homeserver"]
