# ── Stage 1: Build ─────────────────────────────────────────────────────────────
# Use the official Rust image for the build stage.
FROM rust:1.95 AS builder

WORKDIR /build

# Install build dependencies (DBus and Avahi client libraries are required by
# rs-matter's zeroconf mDNS backend on Linux).
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libdbus-1-dev \
    libavahi-client-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the workspace
COPY . .

# Build the release binary
RUN cargo build --release -p matter-onvif-bridge

# ── Stage 2: Runtime ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

# Install runtime libraries needed by the bridge binary.
RUN apt-get update && apt-get install -y --no-install-recommends \
    libdbus-1-3 \
    libavahi-client3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/matter-onvif-bridge /app/matter-onvif-bridge

# Default environment (override in docker-compose or with -e flags)
ENV RUST_LOG=info

# Matter protocol ports:
#   5540/udp  — Matter commissioning and operation
#   5353/udp  — mDNS (requires --network=host on Linux; not available on macOS Docker)
EXPOSE 5540/udp

ENTRYPOINT ["/app/matter-onvif-bridge"]
