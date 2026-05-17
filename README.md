# Matter-ONVIF Camera Bridge

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024_edition-orange.svg)](https://www.rust-lang.org/)
[![Matter](https://img.shields.io/badge/Matter-1.5-brightgreen.svg)](https://csa-iot.org/developer-resource/specifications-download-request/)

A Rust bridge that discovers ONVIF IP cameras on your LAN and exposes them as native **Matter 1.5** camera devices. Tested with Google Home and Apple Home; other Matter 1.5 controllers that support the camera device type may work, but interoperability has not been validated.

Uses [rs-matter](https://github.com/project-chip/rs-matter) for the Matter protocol stack, [oxvif](https://github.com/smiti1642/oxvif) for ONVIF discovery and control, and either [go2rtc](https://github.com/AlexxIT/go2rtc) or [MediaMTX](https://github.com/bluenviron/mediamtx) for RTSP-to-WebRTC media bridging.

---

## Table of Contents

- [Features](#features)
- [Quick Start](#quick-start)
- [Requirements](#requirements)
- [Configuration](#configuration)
- [Support Matrix](#support-matrix)
- [Deployment](#deployment)
  - [macOS — Native](#macos--native)
  - [Linux — Native](#linux--native)
  - [Linux — Docker (Full Stack)](#linux--docker-full-stack)
  - [Raspberry Pi](#raspberry-pi)
  - [systemd Service](#run-as-a-systemd-service)
- [Media Servers](#media-servers)
  - [go2rtc (default)](#go2rtc-default)
  - [MediaMTX](#mediamtx)
- [Cross-Compilation](#cross-compilation)
- [Commissioning](#commissioning)
- [Architecture](#architecture)
- [Workspace Structure](#workspace-structure)
- [Scripts Reference](#scripts-reference)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [License](#license)

---

## Features

- **Matter 1.5 camera device type** — `CameraAvStreamManagement` (0x0551) and `WebRTCTransportProvider` (0x0553)
- **OccupancySensing cluster** — cameras advertising ONVIF `MotionAlarm` events expose a motion-sensor endpoint
- **Dual media-server support** — choose between [go2rtc](https://github.com/AlexxIT/go2rtc) (default) or [MediaMTX](https://github.com/bluenviron/mediamtx) via a single env var (`MEDIA_SERVER`)
- **Snapshot capture (`CaptureSnapshot`)** — bridge captures real JPEG snapshots through backend APIs when supported
- **ONVIF WS-Discovery + static list** — automatically finds cameras on the LAN; static fallback for fixed IPs
- **Manual RTSP fallback** — explicit per-camera RTSP configuration for cameras or networks that do not behave reliably with ONVIF discovery/integration
- **Friendly name overrides** — map camera serial numbers or IPs to human-readable names
- **Persistent slot mapping** — camera ↔ Matter endpoint assignment survives restarts (Google Home room assignments stay stable)
- **Motion event pump** — long-polls ONVIF PullPoint subscriptions and propagates `MotionAlarm` events to the OccupancySensing cluster
- **Up to 8 camera endpoints** — 7 with OccupancySensing, 1 camera-only
- **Small footprint** — ≈10 MB binary, ≈20 MB runtime

---

## Quick Start

### Prerequisites

- Docker (for the media server)
- Rust (for the bridge) — or a pre-compiled binary

```bash
# 1. Copy example config and fill in your ONVIF credentials
cp .env.example .env
nano .env          # set ONVIF_USERNAME, ONVIF_PASSWORD

# 2. Start go2rtc (RTSP → WebRTC media server)
docker compose up -d

# 3. Build and run the bridge
cargo run --release -p matter-onvif-bridge

# 4. On first run the bridge prints a QR code — scan it with your Matter controller
```

> **macOS note:** Docker Desktop on macOS cannot forward UDP multicast. Run the bridge
> natively (steps above). Only the media server (go2rtc / MediaMTX) runs in Docker.

---

## Requirements

### Runtime

| Component | Required on | Notes |
|-----------|-------------|-------|
| Rust 1.85+ | Build host | `rustup update stable` |
| `libdbus-1` | Linux runtime | mDNS via avahi-daemon |
| `libavahi-client` | Linux runtime | mDNS via zeroconf |
| Docker | All platforms | For go2rtc or MediaMTX container |
| IPv6 enabled | Linux host | Matter uses IPv6 for commissioning |

### Ports

| Port | Protocol | Direction | Purpose |
|------|----------|-----------|---------|
| 5540 | UDP | inbound | Matter protocol |
| 5353 | UDP | LAN multicast | mDNS (must be on host network) |
| 3702 | UDP | LAN multicast | ONVIF WS-Discovery |
| 1984 | TCP | loopback | go2rtc REST API |
| 8555 | UDP/TCP | inbound | go2rtc WebRTC |
| 9997 | TCP | loopback | MediaMTX REST API *(if using MediaMTX)* |
| 8889 | TCP | inbound | MediaMTX WHEP *(if using MediaMTX)* |

---

## Configuration

All settings come from environment variables (use a `.env` file — see `.env.example`).

### ONVIF Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `ONVIF_USERNAME` | `admin` | ONVIF credentials shared by all cameras |
| `ONVIF_PASSWORD` | `admin` | ONVIF credentials shared by all cameras |
| `ONVIF_DISCOVERY_MODE` | `auto` | `auto` — WS-Discovery + static list, periodic rescan; `static` — connect to static list once |
| `ONVIF_DISCOVERY_INTERVAL` | `60000` | Rescan interval in ms *(auto mode only)* |
| `ONVIF_STATIC_CAMERAS` | *(empty)* | Comma-separated `host:port` list, e.g. `192.168.1.10:2020,192.168.1.11:80` |
| `ONVIF_CAMERA_NAMES` | *(empty)* | Friendly-name overrides: `serial=Front Door,192.168.1.12=Backyard` — serial number is preferred (survives IP changes) |

### Media Server

| Variable | Default | Description |
|----------|---------|-------------|
| `MEDIA_SERVER` | `go2rtc` | `go2rtc` or `mediamtx` |

### Manual RTSP fallback

| Variable | Default | Description |
|----------|---------|-------------|
| `MANUAL_RTSP_CAMERAS` | *(empty)* | Explicit fallback cameras: `name\|rtsp_url[\|stable_id];name\|rtsp_url[\|stable_id]` |

`MANUAL_RTSP_CAMERAS` is intended for homelab reliability, not automatic abstraction:

- `name` and `rtsp_url` are required; `stable_id` is optional but recommended.
- Camera entries are separated by `;`; fields are separated by `|`.
- For simplicity, `name` must not contain `|` or `;`.
- Manual RTSP cameras bypass ONVIF discovery and do **not** change existing `ONVIF_DISCOVERY_MODE` / `ONVIF_STATIC_CAMERAS` behavior.
- When ONVIF metadata/events are unavailable, manual cameras are exposed as **reduced-capability, video-only** cameras:
  - no implied motion/event support
  - no ONVIF metadata enrichment beyond conservative fallback capability values
  - conservative fallback video metadata is advertised when profile data is unavailable (currently 1280x720 @ 15 fps, single encoder)
  - no ONVIF event pump
- Prefer embedding RTSP credentials directly in the URL for explicit, predictable behavior.

Example:

```env
MANUAL_RTSP_CAMERAS=Front Door|rtsp://user:pass@192.168.1.10:554/stream1|front-door;Garage|rtsp://user:pass@192.168.1.11:554/live
```

#### Manual vs ONVIF duplication / precedence

The current implementation does **not** try to deduplicate a manual RTSP entry against an ONVIF-discovered camera automatically.

- A manual RTSP entry is always added as its own explicit fallback camera identity.
- ONVIF discovery and static-host cameras continue to work unchanged.
- If you define the same physical camera both manually and via ONVIF, the bridge will expose **two** bridged cameras.
- Manual fallback entries therefore do **not** override, suppress, or take precedence over ONVIF-discovered cameras.

Use the manual RTSP path only for cameras that need it, or disable/remove the overlapping ONVIF definition to avoid duplicates.

#### go2rtc (default)

| Variable | Default | Description |
|----------|---------|-------------|
| `GO2RTC_MODE` | `external` | `external` (Docker/remote) or `local` (spawn subprocess) |
| `GO2RTC_PATH` | `./bin/go2rtc` | Binary path — local mode only |
| `GO2RTC_HOST` | `localhost` | Host where the bridge reaches go2rtc |
| `GO2RTC_API_PORT` | `1984` | go2rtc REST API port |
| `GO2RTC_WEBRTC_PORT` | `8555` | go2rtc WebRTC ICE/media port |

#### MediaMTX

| Variable | Default | Description |
|----------|---------|-------------|
| `MEDIAMTX_MODE` | `external` | `external` (Docker/remote) or `local` (spawn subprocess) |
| `MEDIAMTX_PATH` | `./bin/mediamtx` | Binary path — local mode only |
| `MEDIAMTX_HOST` | `localhost` | Host where the bridge reaches MediaMTX |
| `MEDIAMTX_API_PORT` | `9997` | MediaMTX HTTP API port |
| `MEDIAMTX_WHEP_PORT` | `8889` | MediaMTX WHEP (WebRTC) port |

> `MEDIAMTX_MODE=local` is currently **experimental** and less battle-tested than
> Docker/external mode.

### Matter Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `MATTER_PORT` | `5540` | Matter protocol UDP port |
| `MATTER_PASSCODE` | `20202021` | Commissioning passcode *(change before production use)* |
| `MATTER_DISCRIMINATOR` | `3840` | Commissioning discriminator |
| `MATTER_STORAGE_PATH` | `.matter-storage` | Directory for persistent commissioning state |
| `RUST_LOG` | `info` | Log verbosity filter (see [env_logger docs](https://docs.rs/env_logger)) |

---

## Support Matrix

| Area | go2rtc | MediaMTX |
|------|--------|----------|
| Native bridge (`cargo run`) | ✅ Supported | ✅ Supported |
| Linux Docker bridge (`linux-bridge` profile) | ✅ Supported | ⚠️ Not currently wired as full-stack profile |
| Local subprocess mode (`*_MODE=local`) | ✅ Supported | ⚠️ Experimental |
| External mode (`*_MODE=external`) | ✅ Supported | ✅ Supported |
| `CaptureSnapshot` JPEG path | ✅ via go2rtc `/api/frame.jpeg` | ⚠️ Not available via current MediaMTX bridge flow |

Current Docker full-stack profile (`linux-bridge`) starts `bridge` with `depends_on: go2rtc`,
so full containerized backend symmetry is not yet implemented.

---

## Deployment

### macOS — Native

Docker on macOS cannot forward UDP multicast, so the bridge **must** run natively. The media server still runs in Docker.

```bash
# Terminal 1 — start go2rtc
docker compose up -d

# Terminal 2 — build and run the bridge
cargo run --release -p matter-onvif-bridge
```

To use MediaMTX instead:

```bash
docker compose --profile mediamtx up -d
MEDIA_SERVER=mediamtx cargo run --release -p matter-onvif-bridge
```

### Linux — Native

Same as macOS, but you also need the DBus/Avahi libraries:

```bash
sudo apt install -y build-essential pkg-config libdbus-1-dev libavahi-client-dev

# Enable IPv6 if not already active
sudo sysctl -w net.ipv6.conf.all.disable_ipv6=0

docker compose up -d
cargo run --release -p matter-onvif-bridge
```

### Linux — Docker (Full Stack)

On Linux, `--network=host` lets the bridge container reach LAN multicast directly.
The current `linux-bridge` profile is **go2rtc-backed full stack**:

```bash
# Build the bridge image (first time or after code changes)
docker compose --profile linux-bridge build

# Start go2rtc + bridge together (current full-stack Docker path)
docker compose --profile linux-bridge up -d

# View bridge logs
docker compose logs -f bridge
```

For MediaMTX on Linux today, run the bridge natively and run MediaMTX in Docker:

```bash
docker compose --profile mediamtx up -d
MEDIA_SERVER=mediamtx cargo run --release -p matter-onvif-bridge
```

> Docker host-networking is **not available on macOS**. Docker Desktop runs in a VM and
> cannot expose the host's network interfaces to containers.

Make sure IPv6 is enabled on the host:

```bash
sudo sysctl -w net.ipv6.conf.all.disable_ipv6=0
# To make permanent, add to /etc/sysctl.conf:
echo "net.ipv6.conf.all.disable_ipv6=0" | sudo tee -a /etc/sysctl.conf
```

### Raspberry Pi

> **Note:** The recommended deployment is Docker on x86 Linux (see above).
> The Pi instructions below are maintained for users running directly on ARM hardware.

**Option A — Cross-compile from your Mac/PC**

```bash
# Install cross (Docker-based cross-compilation tool)
cargo install cross --git https://github.com/cross-rs/cross

# Build for 64-bit Pi (Pi 3B+, Pi 4, Pi 5)
cross build --release --target aarch64-unknown-linux-gnu -p matter-onvif-bridge

# Copy to the Pi
scp target/aarch64-unknown-linux-gnu/release/matter-onvif-bridge pi@<host>:~/
scp .env.example scripts/install-go2rtc.sh scripts/install-mediamtx.sh pi@<host>:~/
```

On the Pi:

```bash
# Install go2rtc
bash install-go2rtc.sh               # downloads to ./bin/go2rtc

# Configure and run
cp .env.example .env
nano .env                             # ONVIF_USERNAME, ONVIF_PASSWORD, GO2RTC_MODE=local
RUST_LOG=info ./matter-onvif-bridge
```

**Option B — Build natively on the Pi**

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install build dependencies
sudo apt install -y build-essential pkg-config libdbus-1-dev libavahi-client-dev

# Clone and build (this will take several minutes on a Pi)
git clone https://github.com/iamjairo/matter-onvif-bridge
cd matter-onvif-bridge
cargo build --release -p matter-onvif-bridge

# Install go2rtc or MediaMTX
bash scripts/install-go2rtc.sh
# — OR —
bash scripts/install-mediamtx.sh

cp .env.example .env && nano .env
RUST_LOG=info ./target/release/matter-onvif-bridge
```

### Run as a systemd Service

The quickest way on Pi or any Linux machine with a compiled binary:

```bash
# run from a clone of this repo (or from a folder that also contains install-go2rtc.sh)
sudo bash scripts/setup-pi.sh
```

This installs go2rtc into `/opt/matter-onvif-bridge/bin`, creates `/opt/matter-onvif-bridge/.env` from `.env.example`, and registers a systemd service. Then:

```bash
# Edit credentials before starting
sudo nano /opt/matter-onvif-bridge/.env

sudo systemctl start matter-onvif-bridge
journalctl -u matter-onvif-bridge -f
```

Or manually:

```bash
sudo tee /etc/systemd/system/matter-onvif-bridge.service <<EOF
[Unit]
Description=Matter-ONVIF Camera Bridge
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$HOME/matter-onvif-bridge
ExecStart=$HOME/matter-onvif-bridge/matter-onvif-bridge
Restart=always
RestartSec=5
EnvironmentFile=$HOME/matter-onvif-bridge/.env
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now matter-onvif-bridge
journalctl -u matter-onvif-bridge -f
```

---

## Media Servers

### go2rtc (default)

[go2rtc](https://github.com/AlexxIT/go2rtc) is the default and easiest option. It pulls RTSP streams from cameras on demand and serves WebRTC to Matter controllers.

```bash
# Start via Docker (recommended)
docker compose up -d

# Or install the binary locally
bash scripts/install-go2rtc.sh
GO2RTC_MODE=local cargo run --release -p matter-onvif-bridge
```

The go2rtc container/image wiring lives under `docker/go2rtc/` (see the Dockerfile). Streams are registered dynamically by the bridge via the go2rtc REST API, so no manual stream configuration is needed.

### MediaMTX

[MediaMTX](https://github.com/bluenviron/mediamtx) is a full-featured RTSP/RTMP/WebRTC/WHEP proxy. Use it if you need H.265 WHEP streaming, re-streaming to other targets, or prefer MediaMTX's feature set.

```bash
# Start via Docker
docker compose --profile mediamtx up -d

# Run the bridge pointing at MediaMTX
MEDIA_SERVER=mediamtx cargo run --release -p matter-onvif-bridge

# Or install MediaMTX locally
bash scripts/install-mediamtx.sh
MEDIA_SERVER=mediamtx MEDIAMTX_MODE=local cargo run --release -p matter-onvif-bridge
```

The bridge registers each camera's RTSP stream under a path in MediaMTX via its HTTP API (`/v3/config/paths/add/{name}`), then exchanges WebRTC SDP via WHEP (`POST /whep/{name}`).

---

## Cross-Compilation

`cross` handles `aarch64-unknown-linux-gnu` (64-bit ARM — Raspberry Pi 3B+/4/5) out of the box. For targets that need `libdbus-1-dev` and `libavahi-client-dev`, `cross` installs them automatically in its default container images.

```bash
cargo install cross --git https://github.com/cross-rs/cross

# 64-bit ARM (Pi 3B+, Pi 4, Pi 5)
cross build --release --target aarch64-unknown-linux-gnu -p matter-onvif-bridge

# 32-bit ARM (Pi 2 / older Pi 3)
cross build --release --target armv7-unknown-linux-gnueabihf -p matter-onvif-bridge

# x86_64 Linux (from macOS)
cross build --release --target x86_64-unknown-linux-gnu -p matter-onvif-bridge
```

---

## Commissioning

On first launch the bridge prints a QR code and manual pairing code:

```
SetupQRCode: [MT:-24J0AFN00KA064IJ3P0...]
PairingCode:  3497-0112-332
```

Commissioning has been validated with Google Home and Apple Home. Amazon Alexa and other Matter 1.5-capable controllers that support the camera device type should also work:

| Controller | Steps |
|------------|-------|
| **Google Home** | Add device → Matter → Enter setup code or scan QR |
| **Apple Home** | Open Home app → Add accessory → More options → scan QR |
| **Amazon Alexa** | Alexa app → Devices → Add Device → Matter → scan QR or enter code |
| **chip-tool** | `chip-tool pairing onnetwork 2 20202021` |
| **CHIP Device Controller** | Use `pairing code-thread` or `pairing onnetwork` |

After commissioning, each ONVIF camera appears as a separate bridged device. Cameras added later (via WS-Discovery) appear automatically — no re-commissioning required.

To reset commissioning state (e.g. to re-pair with a new controller):

```bash
bash scripts/reset-device.sh
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│  ONVIF IP Cameras (LAN)                                             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐                            │
│  │ Camera A │ │ Camera B │ │ Camera C │  ...  (up to 8)            │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘                            │
│       │  SOAP/HTTP │            │ RTSP                              │
└───────┼────────────┼────────────┼─────────────────────────────────-┘
        │            │            │
        ▼            ▼            ▼
┌───────────────────────────────────────────────────────────────────┐
│  matter-onvif-bridge (Rust binary)                                │
│                                                                   │
│  ┌──────────────────┐    ┌────────────────────────────────────┐   │
│  │  oxvif           │    │  CameraRegistry (broadcast events) │   │
│  │  WS-Discovery    │───▶│  Added / Removed / Updated         │   │
│  │  ONVIF Client    │    └────────┬──────────────┬────────────┘   │
│  └──────────────────┘            │              │                 │
│                                  │              │                 │
│  ┌───────────────────────────────▼─┐  ┌─────────▼──────────────┐ │
│  │  StreamManager                  │  │  MatterBridge          │ │
│  │  Registers RTSP streams         │  │  Populates endpoint    │ │
│  │  in go2rtc or MediaMTX          │  │  states from ONVIF     │ │
│  └───────────────────────────────┬─┘  └─────────┬──────────────┘ │
│                                  │              │                 │
│  ┌───────────────────────────────▼─┐  ┌─────────▼──────────────┐ │
│  │  AnyMediaApi                    │  │  rs-matter endpoints   │ │
│  │  go2rtc / MediaMTX REST + WHEP  │  │  CameraAvStreamMgmt    │ │
│  └─────────────────────────────────┘  │  WebRtcTransportProv   │ │
│                                       │  OccupancySensing      │ │
│  ┌──────────────────────────────────┐ │  BridgedDeviceBasicInfo│ │
│  │  MotionPump (per camera)         │ └────────────────────────┘ │
│  │  PullPoint subscription          │                            │
│  │  MotionAlarm → OccupancySensing  │                            │
│  └──────────────────────────────────┘                            │
└───────────────────────────────────────────────────────────────────┘
        │
        ▼ mDNS (UDP 5353) · Matter (UDP 5540) · IPv6
┌───────────────────────────────────────────────────────────────────┐
│  Matter Controllers                                               │
│  Google Home · Apple Home · chip-tool · CHIP Device Controller   │
└───────────────────────────────────────────────────────────────────┘
```

**Data flow for a WebRTC stream:**

1. Controller invokes `ProvideOffer` on `WebRtcTransportProvider` (0x0553) with an SDP offer
2. Bridge looks up the camera's RTSP stream name
3. Bridge calls go2rtc's SDP API (or MediaMTX's WHEP endpoint) and gets an SDP answer
4. Bridge returns the SDP answer to the controller via `ProvideOfferResponse`
5. ICE/DTLS negotiation completes directly between controller and media server

`WebRtcTransportProvider` bridge behavior in this project:

- `ProvideOffer` is the primary interoperable path and remains the bridge-default flow.
- `SolicitOffer` is accepted but returned as `deferredOffer=true` because this bridge cannot generate server-originated offers from the current backend APIs.
- `ProvideAnswer` and `ProvideICECandidates` are validated/accepted for active sessions; they currently do not change backend negotiation state because go2rtc/MediaMTX negotiations in this bridge are still offer-driven.

---

## Workspace Structure

```
matter-onvif-bridge/
├── Cargo.toml                  # Workspace manifest
├── Dockerfile                  # Multi-stage bridge Docker image
├── docker-compose.yml          # go2rtc + MediaMTX + linux-bridge profiles
├── .env.example                # All environment variables with documentation
├── .github/
│   ├── labels.yml              # GitHub issue/PR label definitions
│   └── dependabot.yml          # Automated dependency update config
├── crates/
│   ├── bridge/                 # Main binary — wires everything together
│   │   └── src/
│   │       ├── main.rs         # Matter endpoint setup, data model, main loop
│   │       ├── config.rs       # Config struct, env var parsing
│   │       ├── onvif_bridge.rs # ONVIF + media-server bridge thread
│   │       ├── slot_persistence.rs  # Stable camera → endpoint slot mapping
│   │       └── mdns.rs         # mDNS responder (avahi/zeroconf backend)
│   ├── matter-camera/          # Custom Matter cluster implementations
│   │   └── src/
│   │       ├── lib.rs                # Shared helpers (TLV parsing, stream usage)
│   │       ├── cluster_av_stream.rs  # CameraAvStreamManagement (0x0551)
│   │       ├── cluster_webrtc.rs     # WebRtcTransportProvider (0x0553)
│   │       ├── cluster_occupancy.rs  # OccupancySensing wrapper
│   │       └── types.rs              # Matter 1.5 camera type definitions
│   ├── onvif-client/           # ONVIF discovery, client, and registry
│   │   ├── examples/
│   │   │   ├── probe_all.rs          # Discover + dump all camera info
│   │   │   ├── probe_capabilities.rs # Probe camera capabilities
│   │   │   ├── probe_motion.rs       # Test motion event subscriptions
│   │   │   ├── probe_names.rs        # List camera friendly names
│   │   │   ├── probe_scopes.rs       # Dump ONVIF scopes
│   │   │   └── test_connect.rs       # Basic ONVIF connectivity test
│   │   └── src/
│   │       ├── lib.rs          # Crate root / re-exports
│   │       ├── client.rs       # Per-camera ONVIF connect + info fetch
│   │       ├── discovery.rs    # WS-Discovery loop + static camera list
│   │       ├── motion.rs       # PullPoint motion-alarm event pump
│   │       ├── registry.rs     # CameraRegistry broadcast store
│   │       └── types.rs        # Camera device/profile types
│   └── media/                  # Media-server integration
│       └── src/
│           ├── lib.rs              # Crate root / re-exports
│           ├── media_api.rs        # AnyMediaApi — unified go2rtc/MediaMTX API
│           ├── go2rtc_api.rs       # go2rtc REST API client
│           ├── go2rtc_manager.rs   # go2rtc process lifecycle
│           ├── mediamtx_api.rs     # MediaMTX REST + WHEP client
│           ├── mediamtx_manager.rs # MediaMTX process lifecycle
│           ├── stream_manager.rs   # Syncs CameraRegistry → media server
│           └── webrtc_session.rs   # SDP offer/answer + ICE negotiation
├── docker/
│   ├── go2rtc/
│   │   └── go2rtc.yaml         # go2rtc configuration
│   └── mediamtx/
│       └── mediamtx.yml        # MediaMTX configuration
└── scripts/
    ├── install-go2rtc.sh       # Download go2rtc binary for current platform
    ├── install-mediamtx.sh     # Download MediaMTX binary for current platform
    ├── setup-pi.sh             # Full Pi setup (go2rtc + systemd service)
    └── reset-device.sh         # Wipe Matter commissioning state for re-pairing
```

---

## Scripts Reference

| Script | Usage | Description |
|--------|-------|-------------|
| `scripts/install-go2rtc.sh` | `bash scripts/install-go2rtc.sh [version] [dir]` | Downloads a go2rtc binary for the current OS/arch into `./bin/go2rtc` (default) |
| `scripts/install-mediamtx.sh` | `bash scripts/install-mediamtx.sh [version] [dir]` | Downloads a MediaMTX binary for the current OS/arch into `./bin/mediamtx` (default) |
| `scripts/setup-pi.sh` | `sudo bash scripts/setup-pi.sh [install-dir] [user]` | Full Raspberry Pi setup: installs go2rtc in `<install-dir>/bin`, creates `<install-dir>/.env`, registers systemd service |
| `scripts/reset-device.sh` | `bash scripts/reset-device.sh` | Stops the bridge, removes Matter commissioning state, optionally restarts |

---

## Troubleshooting

### Bridge doesn't appear in Google Home / Apple Home

- **Check IPv6**: `ip addr show | grep inet6` — Matter requires IPv6 link-local addresses
- **Check mDNS**: `avahi-browse -r _matter._tcp` — should list the bridge service
- **Check port**: `ss -unlp | grep 5540` — bridge must be listening on UDP 5540
- **Enable IPv6**: `sudo sysctl -w net.ipv6.conf.all.disable_ipv6=0`

### Cameras not discovered

- Check ONVIF credentials in `.env`
- Try static mode: `ONVIF_DISCOVERY_MODE=static ONVIF_STATIC_CAMERAS=192.168.1.10:2020`
- Test connectivity: `cargo run -p onvif-client --example test_connect`
- Check firewall: ONVIF WS-Discovery needs UDP 3702 multicast

### go2rtc / MediaMTX not reachable

- Verify Docker containers are running: `docker compose ps`
- Check the API port: `curl http://localhost:1984/api` (go2rtc) or `curl http://localhost:9997/v3/paths/list` (MediaMTX)
- If using Docker on Linux with the bridge inside Docker too, make sure both containers share a network

### WebRTC stream doesn't open

- Confirm the media server is reachable from where the controller is (STUN/ICE traversal)
- Check WebRTC port is not blocked: go2rtc uses UDP/TCP 8555, MediaMTX uses 8889
- Enable `RUST_LOG=debug` for detailed SDP exchange logs

### Re-pair after factory reset

```bash
bash scripts/reset-device.sh
# Then restart the bridge — it will print a new QR code
```

---

## Contributing

Contributions, bug reports, and feature requests are welcome!

1. **Bug reports / feature requests** — open an issue and apply the appropriate label
2. **Pull requests** — fork, create a branch, make your changes, open a PR
3. **Code style** — run `cargo fmt` and `cargo clippy` before submitting

### Development tips

```bash
# Check the full workspace (no build artefacts needed)
cargo check --workspace

# Run all tests
cargo test --workspace

# Check for common issues
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all
```

Dependabot is configured to open automated PRs for Cargo dependency updates, Docker base-image bumps, and GitHub Actions version pins.

---

## License

MIT — see [LICENSE](LICENSE) for the full text.
