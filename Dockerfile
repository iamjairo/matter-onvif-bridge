# ── Stage 1: Build ─────────────────────────────────────────────────────────────
FROM rust:1.87-bookworm AS builder

WORKDIR /build

# Install build dependencies (DBus and Avahi client libraries are required by
# rs-matter's zeroconf mDNS backend on Linux).
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libdbus-1-dev \
    libavahi-client-dev \
    && rm -rf /var/lib/apt/lists/*

# ── Dependency caching layer ──────────────────────────────────────────────────
# Copy only the manifests first so that dependency builds are cached as long as
# Cargo.toml / Cargo.lock don't change.
COPY Cargo.toml Cargo.lock ./
COPY crates/bridge/Cargo.toml crates/bridge/Cargo.toml
COPY crates/matter-camera/Cargo.toml crates/matter-camera/Cargo.toml
COPY crates/media/Cargo.toml crates/media/Cargo.toml
COPY crates/onvif-client/Cargo.toml crates/onvif-client/Cargo.toml

# Create dummy source files so cargo can resolve the workspace and build deps.
RUN mkdir -p crates/bridge/src && echo "fn main() {}" > crates/bridge/src/main.rs \
    && mkdir -p crates/matter-camera/src && echo "" > crates/matter-camera/src/lib.rs \
    && mkdir -p crates/media/src && echo "" > crates/media/src/lib.rs \
    && mkdir -p crates/onvif-client/src && echo "" > crates/onvif-client/src/lib.rs

RUN cargo build --release -p matter-onvif-bridge

# Remove dummy sources, copy real source tree, and rebuild (only project code).
RUN rm -rf crates/*/src
COPY . .
RUN cargo build --release -p matter-onvif-bridge

# ── Stage 2: Runtime ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

# Install runtime libraries needed by the bridge binary.
RUN apt-get update && apt-get install -y --no-install-recommends \
    libdbus-1-3 \
    libavahi-client3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user for the runtime.
RUN groupadd --system appgroup && useradd --system --no-create-home --gid appgroup appuser

WORKDIR /app

COPY --from=builder /build/target/release/matter-onvif-bridge /app/matter-onvif-bridge

RUN chown appuser:appgroup /app

# Default environment (override in docker-compose or with -e flags)
ENV RUST_LOG=info

# Matter protocol ports:
#   5540/udp  — Matter commissioning and operation
#   5353/udp  — mDNS (requires --network=host on Linux; not available on macOS Docker)
EXPOSE 5540/udp

USER appuser

ENTRYPOINT ["/app/matter-onvif-bridge"]
