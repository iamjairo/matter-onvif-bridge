# Matter-ONVIF Camera Bridge

[![CI](https://github.com/iamjairo/matter-onvif-bridge/actions/workflows/ci.yml/badge.svg)](https://github.com/iamjairo/matter-onvif-bridge/actions/workflows/ci.yml)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024_edition-orange.svg)](https://www.rust-lang.org/)
[![Matter](https://img.shields.io/badge/Matter-1.5-brightgreen.svg)](https://csa-iot.org/developer-resource/specifications-download-request/)

A Rust bridge that discovers ONVIF IP cameras on your LAN and exposes them as native **Matter 1.5** camera devices. Tested with Google Home and Apple Home; other Matter 1.5 controllers that support the camera device type should also work.

Built on [rs-matter](https://github.com/project-chip/rs-matter) for the Matter stack, [oxvif](https://github.com/smiti1642/oxvif) for ONVIF discovery and control, and [go2rtc](https://github.com/AlexxIT/go2rtc) or [MediaMTX](https://github.com/bluenviron/mediamtx) for RTSP-to-WebRTC media bridging.

---

## Features

| Cluster | ID | Status |
|---------|----|--------|
| CameraAvStreamManagement | 0x0551 | Functional — video codec/resolution reporting, snapshot capture |
| WebRTCTransportProvider | 0x0553 | Functional — SDP offer/answer, ICE candidate relay |
| WebRTCTransportRequestor | 0x0554 | Functional — SDP exchange callback |
| ZoneManagement | 0x0550 | Functional — in-memory zone CRUD |
| PushAvStreamTransport | 0x0555 | Stub — reports zero transport connections |
| Chime | 0x0556 | Stub — reports zero chime sounds |
| OccupancySensing | 0x0406 | Functional — ONVIF MotionAlarm → occupancy events |

- **Dual media-server support** — choose between go2rtc (default) or MediaMTX via `MEDIA_SERVER` env var
- **Snapshot capture** — real JPEG snapshots via media-server APIs when available
- **ONVIF WS-Discovery + static list** — automatic LAN discovery with static IP fallback
- **Manual RTSP fallback** — explicit per-camera RTSP for cameras without reliable ONVIF
- **Friendly name overrides** — map serial numbers or IPs to human-readable names
- **Persistent slot mapping** — camera ↔ Matter endpoint assignment survives restarts
- **Motion event pump** — ONVIF PullPoint subscriptions propagate MotionAlarm events
- **Up to 8 camera endpoints** — 7 with OccupancySensing + 1 camera-only
- **Credential-safe logging** — RTSP URLs are redacted in all log output
- **~10 MB binary, ~20 MB runtime**

---

## Quick Start

```bash
# 1. Copy example config and set your ONVIF credentials
cp .env.example .env
nano .env

# 2. Start the media server
docker compose up -d

# 3. Build and run the bridge
cargo run --release -p matter-onvif-bridge

# 4. Scan the QR code with your Matter controller
```

> **macOS:** Docker Desktop cannot forward UDP multicast. Run the bridge natively — only the media server runs in Docker.

---

## Requirements

| Component | Platform | Notes |
|-----------|----------|-------|
| Rust 1.85+ | Build host | `rustup update stable` |
| Docker | All | For go2rtc or MediaMTX |
| `libdbus-1` + `libavahi-client` | Linux | mDNS via avahi-daemon |
| IPv6 | Linux | Required for Matter commissioning |

### Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| 5540 | UDP | Matter protocol |
| 5353 | UDP multicast | mDNS |
| 3702 | UDP multicast | ONVIF WS-Discovery |
| 1984 | TCP loopback | go2rtc REST API |
| 8555 | UDP/TCP | go2rtc WebRTC |
| 9997 | TCP loopback | MediaMTX REST API |
| 8889 | TCP | MediaMTX WHEP |

---

## Configuration

All settings come from environment variables. See [`.env.example`](.env.example) for a fully documented template.

### ONVIF

| Variable | Default | Description |
|----------|---------|-------------|
| `ONVIF_USERNAME` | `admin` | Shared ONVIF credentials |
| `ONVIF_PASSWORD` | `admin` | Shared ONVIF credentials |
| `ONVIF_DISCOVERY_MODE` | `auto` | `auto` (WS-Discovery + static, periodic rescan) or `static` (static list only, once) |
| `ONVIF_DISCOVERY_INTERVAL` | `60000` | Rescan interval in ms (auto mode only) |
| `ONVIF_STATIC_CAMERAS` | | Comma-separated `host:port` list |
| `ONVIF_CAMERA_NAMES` | | Friendly-name map: `serial=Name,ip=Name` |

### Media Server

| Variable | Default | Description |
|----------|---------|-------------|
| `MEDIA_SERVER` | `go2rtc` | `go2rtc` or `mediamtx` |

#### go2rtc

| Variable | Default | Description |
|----------|---------|-------------|
| `GO2RTC_MODE` | `external` | `external` (Docker) or `local` (subprocess) |
| `GO2RTC_PATH` | `./bin/go2rtc` | Binary path (local mode only) |
| `GO2RTC_HOST` | `localhost` | API host |
| `GO2RTC_API_PORT` | `1984` | REST API port |
| `GO2RTC_WEBRTC_PORT` | `8555` | WebRTC port |

#### MediaMTX

| Variable | Default | Description |
|----------|---------|-------------|
| `MEDIAMTX_MODE` | `external` | `external` (Docker) or `local` (subprocess, experimental) |
| `MEDIAMTX_PATH` | `./bin/mediamtx` | Binary path (local mode only) |
| `MEDIAMTX_HOST` | `localhost` | API host |
| `MEDIAMTX_API_PORT` | `9997` | HTTP API port |
| `MEDIAMTX_WHEP_PORT` | `8889` | WHEP (WebRTC) port |

### Manual RTSP Fallback

For cameras that don't work reliably with ONVIF discovery:

```env
MANUAL_RTSP_CAMERAS=Front Door|rtsp://user:pass@192.168.1.10:554/stream1|front-door;Garage|rtsp://user:pass@192.168.1.11:554/live
```

Format: `name|rtsp_url[|stable_id]` entries separated by `;`. Fields separated by `|`.

Manual RTSP cameras bypass ONVIF and are exposed as reduced-capability, video-only endpoints (no motion events, conservative fallback metadata of 1280x720 @ 15 fps). If the same physical camera is also discovered via ONVIF, both will appear as separate endpoints — use one or the other.

### Matter

| Variable | Default | Description |
|----------|---------|-------------|
| `MATTER_PORT` | `5540` | Matter UDP port |
| `MATTER_PASSCODE` | `20202021` | Commissioning passcode |
| `MATTER_DISCRIMINATOR` | `3840` | Commissioning discriminator |
| `MATTER_STORAGE_PATH` | `.matter-storage` | Persistent commissioning state directory |
| `RUST_LOG` | `info` | Log verbosity filter |

---

## Deployment

### macOS — Native

```bash
docker compose up -d                           # start go2rtc
cargo run --release -p matter-onvif-bridge     # run the bridge
```

For MediaMTX: `docker compose --profile mediamtx up -d` then `MEDIA_SERVER=mediamtx cargo run --release -p matter-onvif-bridge`.

### Linux — Native

```bash
sudo apt install -y build-essential pkg-config libdbus-1-dev libavahi-client-dev
sudo sysctl -w net.ipv6.conf.all.disable_ipv6=0

docker compose up -d
cargo run --release -p matter-onvif-bridge
```

### Linux — Docker (Full Stack)

The `linux-bridge` profile runs go2rtc + bridge together with `--network=host`:

```bash
docker compose --profile linux-bridge build
docker compose --profile linux-bridge up -d
docker compose logs -f bridge
```

> Host networking is required for LAN multicast and is **not available on macOS**.

### systemd Service

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

[go2rtc](https://github.com/AlexxIT/go2rtc) pulls RTSP streams on demand and serves WebRTC to Matter controllers. Streams are registered dynamically via the REST API — no manual stream configuration needed.

```bash
docker compose up -d                                              # Docker (recommended)
bash scripts/install-go2rtc.sh && GO2RTC_MODE=local cargo run ... # or local binary
```

Configuration: [`docker/go2rtc/go2rtc.yaml`](docker/go2rtc/go2rtc.yaml)

### MediaMTX

[MediaMTX](https://github.com/bluenviron/mediamtx) is a full-featured RTSP/WebRTC/WHEP proxy. The bridge registers streams via its HTTP API and exchanges WebRTC SDP via WHEP.

```bash
docker compose --profile mediamtx up -d
MEDIA_SERVER=mediamtx cargo run --release -p matter-onvif-bridge
```

Configuration: [`docker/mediamtx/mediamtx.yml`](docker/mediamtx/mediamtx.yml)

### Support Matrix

| Capability | go2rtc | MediaMTX |
|------------|--------|----------|
| Native bridge | Supported | Supported |
| Docker full-stack | Supported | Not wired yet |
| Local subprocess | Supported | Experimental |
| Snapshot JPEG | Supported | Not available |

---

## Commissioning

On first launch the bridge prints a QR code and pairing code. Scan with your Matter controller:

| Controller | Steps |
|------------|-------|
| **Google Home** | Add device → Matter → scan QR or enter code |
| **Apple Home** | Home app → Add accessory → More options → scan QR |
| **Amazon Alexa** | Devices → Add Device → Matter → scan QR or enter code |
| **chip-tool** | `chip-tool pairing onnetwork 2 20202021` |

Cameras discovered later appear automatically — no re-commissioning needed.

To reset commissioning state: `bash scripts/reset-device.sh`

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│  ONVIF IP Cameras (LAN)                                          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐                         │
│  │ Camera A │ │ Camera B │ │ Camera C │  ...  (up to 8)         │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘                         │
│       │ SOAP/HTTP  │            │ RTSP                           │
└───────┼────────────┼────────────┼────────────────────────────────┘
        ▼            ▼            ▼
┌──────────────────────────────────────────────────────────────────┐
│  matter-onvif-bridge                                              │
│                                                                   │
│  ┌────────────────┐    ┌──────────────────────────────────────┐   │
│  │ oxvif          │    │ CameraRegistry                       │   │
│  │ WS-Discovery   │───▶│ broadcast: Added / Removed / Updated │   │
│  │ ONVIF Client   │    └──────┬──────────────────┬────────────┘   │
│  └────────────────┘           │                  │                │
│                               ▼                  ▼                │
│  ┌────────────────────────────────┐  ┌────────────────────────┐   │
│  │ StreamManager                  │  │ rs-matter endpoints    │   │
│  │ Registers RTSP in media server │  │                        │   │
│  └──────────────┬─────────────────┘  │ CameraAvStreamMgmt    │   │
│                 ▼                    │ WebRtcTransportProv    │   │
│  ┌────────────────────────────────┐  │ WebRtcTransportReq     │   │
│  │ AnyMediaApi                    │  │ ZoneManagement         │   │
│  │ go2rtc / MediaMTX REST + WHEP  │  │ PushAvStreamTransport  │   │
│  └────────────────────────────────┘  │ Chime                  │   │
│                                      │ OccupancySensing       │   │
│  ┌────────────────────────────────┐  │ BridgedDeviceBasicInfo │   │
│  │ MotionPump (per camera)        │  └────────────────────────┘   │
│  │ PullPoint → OccupancySensing   │                               │
│  └────────────────────────────────┘                               │
└──────────────────────────────────────────────────────────────────┘
        │
        ▼  mDNS (5353) · Matter (5540) · IPv6
┌──────────────────────────────────────────────────────────────────┐
│  Matter Controllers                                               │
│  Google Home · Apple Home · chip-tool                             │
└──────────────────────────────────────────────────────────────────┘
```

**WebRTC stream flow:**

1. Controller sends SDP offer via `ProvideOffer` on WebRTCTransportProvider (0x0553)
2. Bridge forwards to go2rtc SDP API (or MediaMTX WHEP) and gets an SDP answer
3. Bridge returns the answer via `ProvideOfferResponse`
4. ICE/DTLS negotiation completes directly between controller and media server

---

## Workspace

```
matter-onvif-bridge/
├── Cargo.toml                    # workspace manifest
├── Dockerfile                    # multi-stage bridge image
├── docker-compose.yml            # go2rtc + MediaMTX + linux-bridge profiles
├── .env.example                  # documented env var template
├── .github/
│   ├── workflows/ci.yml          # CI: check, clippy, test, fmt
│   ├── labels.yml                # issue/PR label definitions
│   └── dependabot.yml            # automated dependency updates
├── crates/
│   ├── bridge/src/               # main binary — wires everything together
│   │   ├── main.rs               #   Matter endpoint setup, data model, main loop
│   │   ├── config.rs             #   env var parsing
│   │   ├── onvif_bridge.rs       #   ONVIF + media-server bridge thread
│   │   ├── slot_persistence.rs   #   stable camera → endpoint slot mapping
│   │   └── mdns.rs               #   mDNS responder
│   ├── matter-camera/src/        # Matter cluster implementations
│   │   ├── cluster_av_stream.rs  #   CameraAvStreamManagement (0x0551)
│   │   ├── cluster_webrtc.rs     #   WebRTCTransportProvider (0x0553)
│   │   ├── cluster_webrtc_requestor.rs  # WebRTCTransportRequestor (0x0554)
│   │   ├── cluster_zone_mgmt.rs  #   ZoneManagement (0x0550)
│   │   ├── cluster_push_av.rs    #   PushAvStreamTransport (0x0555)
│   │   ├── cluster_chime.rs      #   Chime (0x0556)
│   │   ├── cluster_occupancy.rs  #   OccupancySensing wrapper
│   │   └── types.rs              #   Matter 1.5 camera type definitions
│   ├── onvif-client/src/         # ONVIF discovery and client
│   │   ├── client.rs             #   per-camera ONVIF connect + info fetch
│   │   ├── discovery.rs          #   WS-Discovery + static camera list
│   │   ├── motion.rs             #   PullPoint motion-alarm event pump
│   │   ├── registry.rs           #   CameraRegistry broadcast store
│   │   └── types.rs              #   camera device/profile types
│   └── media/src/                # media-server integration
│       ├── media_api.rs          #   AnyMediaApi — unified go2rtc/MediaMTX facade
│       ├── go2rtc_api.rs         #   go2rtc REST API client
│       ├── mediamtx_api.rs       #   MediaMTX REST + WHEP client
│       ├── stream_manager.rs     #   syncs CameraRegistry → media server
│       └── webrtc_session.rs     #   SDP offer/answer + ICE negotiation
├── docker/
│   ├── go2rtc/go2rtc.yaml        # go2rtc configuration
│   └── mediamtx/mediamtx.yml     # MediaMTX configuration
└── scripts/
    ├── install-go2rtc.sh         # download go2rtc binary
    ├── install-mediamtx.sh       # download MediaMTX binary
    └── reset-device.sh           # wipe commissioning state
```

---

## Troubleshooting

**Bridge not visible to controllers:**
- Check IPv6: `ip addr show | grep inet6`
- Check mDNS: `avahi-browse -r _matter._tcp`
- Check port: `ss -unlp | grep 5540`
- Enable IPv6: `sudo sysctl -w net.ipv6.conf.all.disable_ipv6=0`

**Cameras not discovered:**
- Verify ONVIF credentials in `.env`
- Try static mode: `ONVIF_DISCOVERY_MODE=static ONVIF_STATIC_CAMERAS=192.168.1.10:2020`
- Test connectivity: `cargo run -p onvif-client --example test_connect`
- Check firewall allows UDP 3702 multicast

**Media server unreachable:**
- `docker compose ps` — containers running?
- `curl http://localhost:1984/api` (go2rtc) or `curl http://localhost:9997/v3/paths/list` (MediaMTX)

**WebRTC stream won't open:**
- Verify WebRTC port is reachable (go2rtc: 8555, MediaMTX: 8889)
- `RUST_LOG=debug` for SDP exchange details

**Re-pair after reset:** `bash scripts/reset-device.sh` then restart the bridge.

---

## Contributing

1. **Issues** — bug reports and feature requests welcome
2. **PRs** — fork, branch, change, PR
3. **Style** — `cargo fmt && cargo clippy --workspace -- -D warnings` before submitting

```bash
cargo check --workspace          # type-check
cargo test --workspace           # run tests
cargo clippy --workspace -- -D warnings  # lint
cargo fmt --all                  # format
```

Dependabot is configured for Cargo dependencies, Docker base images, and GitHub Actions versions.

---

## License

MIT — see [LICENSE](LICENSE).
