# Matter-ONVIF Bridge

A Matter bridge that discovers ONVIF IP cameras on your local network and exposes them as Matter camera devices. Uses [matter.js](https://github.com/matter-js/matter.js) for the Matter protocol and [go2rtc](https://github.com/AlexxIT/go2rtc) for RTSP-to-WebRTC media bridging.

## Architecture

```
ONVIF Cameras <--RTSP/SOAP--> [ONVIF Client] --> [Camera Registry]
                                                        |
Matter Controllers <--Matter--> [Matter Bridge (AggregatorEndpoint)]
       |                                |
       +------WebRTC (P2P)-----> [go2rtc] <--RTSP--> ONVIF Cameras
```

- **go2rtc** runs in Docker — handles RTSP ingestion and WebRTC streaming with H.264 passthrough (no transcoding)
- **Node.js bridge** runs natively — needs LAN access for ONVIF WS-Discovery (UDP multicast) and Matter mDNS commissioning

## Prerequisites

- Node.js 20+
- pnpm (`npm install -g pnpm`)
- Docker and Docker Compose
- ONVIF-compatible IP cameras on your network

## Quick Start

```bash
# Install dependencies
pnpm install

# Copy and edit environment config
cp .env.example .env
# Edit .env with your ONVIF camera credentials

# Start go2rtc in Docker
docker compose up -d

# Build and run the bridge
pnpm build
pnpm start
```

On startup, the bridge will display a commissioning banner:

```
╔══════════════════════════════════════════════╗
║  Matter ONVIF Camera Bridge                  ║
║                                              ║
║  Manual pairing code: 34970112332      ║
║  Passcode:            20202021          ║
║  Discriminator:       3840              ║
║  Port:                5540              ║
╚══════════════════════════════════════════════╝
```

## Commissioning

The bridge commissions over IP (no Bluetooth required). It advertises via mDNS on your local network.

**Apple Home** (iPhone/iPad):
1. Open Home app → **+** → **Add Accessory** → **More options...**
2. The bridge should appear as "ONVIF Camera Bridge"
3. Enter the setup code displayed in the banner

**Google Home** (Android/iPhone):
1. Open Google Home → **+** → **Set up device** → **New device** → **Matter**
2. Enter the pairing code from the banner

**Home Assistant**:
1. Go to **Settings** → **Devices & Services** → **Add Integration** → **Matter**
2. Select **Commission device** and enter the pairing code
3. HA and the bridge must be on the same network/VLAN

**chip-tool** (command line):
```bash
chip-tool pairing onnetwork 1 20202021
```

> **Note:** Matter 1.5 camera support is new (November 2024). Controllers may not yet fully support the camera clusters (CameraAvStreamManagement, WebRtcTransportProvider). The bridge will commission and appear as a device, but live video streaming depends on controller support.

## Configuration

All configuration is via environment variables in `.env`:

| Variable | Default | Description |
|----------|---------|-------------|
| `ONVIF_USERNAME` | `admin` | ONVIF camera username |
| `ONVIF_PASSWORD` | `admin` | ONVIF camera password |
| `ONVIF_DISCOVERY_INTERVAL` | `60000` | Discovery scan interval (ms) |
| `ONVIF_STATIC_CAMERAS` | | Comma-separated list of `host:port` for cameras that may not respond to WS-Discovery |
| `GO2RTC_MODE` | `external` | `external` (Docker) or `local` (subprocess) |
| `GO2RTC_HOST` | `localhost` | Hostname where go2rtc API is reachable |
| `GO2RTC_API_PORT` | `1984` | go2rtc REST API port |
| `GO2RTC_WEBRTC_PORT` | `8555` | go2rtc WebRTC port |
| `MATTER_PORT` | `5540` | Matter protocol port |
| `MATTER_PASSCODE` | `20202021` | Commissioning passcode |
| `MATTER_DISCRIMINATOR` | `3840` | Commissioning discriminator |
| `LOG_LEVEL` | `info` | Log level: trace, debug, info, warn, error, fatal |

### Static cameras

If your cameras use non-standard ONVIF ports or multicast discovery is unreliable, list them explicitly:

```env
ONVIF_STATIC_CAMERAS=192.168.1.10:2020,192.168.1.11:80,192.168.1.12:2020
```

## Development

```bash
# Run in development mode (auto-recompile)
pnpm dev

# Type check
pnpm exec tsc --noEmit

# Run tests
pnpm test

# View go2rtc logs
pnpm go2rtc:logs

# Start both go2rtc and bridge
pnpm up
```

### Project Structure

```
src/
  index.ts                          # Entry point
  config.ts                         # Environment config
  logger.ts                         # Pino logger
  onvif/
    discovery.ts                    # WS-Discovery + static camera list
    client.ts                       # Per-camera ONVIF client
    types.ts                        # Interfaces
  registry/
    camera-registry.ts              # Discovered camera store
    camera.ts                       # CameraDevice model
  media/
    go2rtc-manager.ts               # go2rtc lifecycle (Docker or subprocess)
    go2rtc-api.ts                   # REST/WebSocket client for go2rtc
    stream-manager.ts               # RTSP stream registration
    webrtc-session.ts               # WebRTC SDP/ICE session
  matter/
    bridge.ts                       # ServerNode + AggregatorEndpoint
    clusters/
      camera-av-stream-management/  # Cluster 0x0551 (Matter 1.5)
      webrtc-transport-provider/    # Cluster 0x0553 (Matter 1.5)
```

## Raspberry Pi Deployment

### Quick setup

```bash
# Copy the project to your Pi
scp -r . pi@<pi-ip>:/opt/matter-onvif-bridge/

# Run the setup script (installs Node.js, pnpm, go2rtc, creates systemd service)
sudo bash /opt/matter-onvif-bridge/scripts/setup-pi.sh

# Edit your credentials
nano /opt/matter-onvif-bridge/.env

# Start the service
sudo systemctl start matter-onvif-bridge

# View logs
journalctl -u matter-onvif-bridge -f
```

The service auto-starts on boot and restarts on failure.

### Requirements

- Raspberry Pi 4 or 5, 2GB+ RAM
- Raspberry Pi OS (64-bit recommended)
- Wired Ethernet recommended (for reliable multicast)
- ~80-110MB RAM usage (Node.js + go2rtc)

### go2rtc on Pi

On the Pi, you can either:
- Use Docker (keep `GO2RTC_MODE=external` and run `docker compose up -d`)
- Run go2rtc natively (set `GO2RTC_MODE=local` in `.env` and run `bash scripts/install-go2rtc.sh`)

## Known Issues

- **TP-Link VIGI cameras**: Some VIGI firmware versions have intermittent ONVIF connection failures. The bridge includes `preserveAddress: true` as a workaround, but connections may still be flaky. Cameras will connect on subsequent scan cycles.
- **Matter 1.5 controller support**: Camera clusters are new. Apple Home, Google Home, and other controllers may not yet support viewing Matter camera streams.
- **Snapshot capture**: Not yet implemented.

## License

Apache-2.0
