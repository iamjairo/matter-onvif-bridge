# HTTP Control Surface — Design Spec

**Status:** Draft (design only — no Rust code yet)
**Author:** iamjairo · Claude
**Date:** 2026-05-20
**Tracks:** ROADMAP.md "Later / Optional" → promotes to Phase 2 follow-up once controller validation lands

---

## Why

The bridge is a headless Rust service. It has no HTTP API today, so external observers (the IoT Dashboard's MatterBridgeTab, monitoring tools, status pages) can't see:

- Whether the bridge process is running
- What cameras have been discovered or commissioned
- Whether commissioning is currently open (QR code window)
- Whether motion events are firing

The dashboard tab used to fake this with a "Runtime Status" card showing perpetually-"Unknown" rows. That card was removed in the dashboard's `bde07ab` commit and replaced with a forward-looking "Live status — coming soon" card pointing at this doc.

A small Axum HTTP server inside the bridge process closes the gap. Local-only by default; no internet exposure required.

---

## Non-goals

- **Camera management UI.** Adding/removing cameras stays in `.env` for now. The HTTP surface is read-mostly; the only writes are commissioning-window control.
- **Authentication.** v1 is localhost-bound. If the bridge needs to be reachable from another host on the LAN later, layer auth in v2.
- **Replacement for the Matter protocol.** This is observability + light control, not a parallel command path.
- **Long-running RPC.** Matter commissioning, ONVIF discovery loops, and motion event pumps stay where they are. The HTTP layer just exposes their current state.

---

## Endpoints

All under `/api/v1/` so future breaking changes can ship as `/api/v2/`.

### `GET /api/v1/status`

Bridge process health + summary state. Polled by dashboards.

**Response 200**
```json
{
  "version": "0.1.0",
  "uptime_seconds": 1234,
  "matter": {
    "commissioned": true,
    "commissioning_window_open": false,
    "commissioning_window_expires_at": null,
    "vendor_id": "0xFFF1",
    "product_id": "0x8000",
    "discriminator": 3840
  },
  "onvif": {
    "discovery_mode": "auto",
    "last_scan_at": "2026-05-20T11:04:32Z",
    "next_scan_at": "2026-05-20T11:05:32Z",
    "static_cameras_configured": 0,
    "manual_rtsp_configured": 2
  },
  "media": {
    "server": "mediamtx",
    "host": "localhost",
    "api_port": 9997,
    "reachable": true,
    "last_check_at": "2026-05-20T11:04:30Z"
  },
  "cameras": {
    "discovered": 3,
    "commissioned": 3,
    "endpoints_in_use": 3,
    "endpoints_total": 8
  }
}
```

**Implementation notes**
- `uptime_seconds` from a `std::time::Instant` captured at bridge startup
- `media.reachable` should NOT block the response — cache the most recent probe result (the bridge already does periodic media-server health checks per the registration tolerance work)
- `commissioning_window_expires_at` uses RFC3339; `null` when the window isn't open

---

### `GET /api/v1/cameras`

List of cameras the bridge knows about, with their Matter endpoint assignment.

**Response 200**
```json
{
  "cameras": [
    {
      "id": "ABC123456",                          // ONVIF serial if available, else IP
      "name": "Front Door",                       // from ONVIF_CAMERA_NAMES override, else discovered name
      "source": "onvif",                          // "onvif" | "static" | "manual_rtsp"
      "host": "192.168.1.10",
      "port": 2020,
      "rtsp_url_redacted": "rtsp://[REDACTED]@192.168.1.10:554/Streaming/Channels/101",
      "matter": {
        "endpoint_id": 1,
        "slot_index": 0,
        "commissioned": true,
        "has_motion": true
      },
      "media": {
        "stream_name": "front_door",
        "registered": true,
        "last_motion_at": "2026-05-20T11:03:12Z"
      },
      "first_seen_at": "2026-05-18T22:14:01Z",
      "last_seen_at":  "2026-05-20T11:04:32Z"
    }
  ]
}
```

**Implementation notes**
- `rtsp_url_redacted` uses the existing `crates/media/src/lib.rs::redact_rtsp_url()` helper — never expose credentials over HTTP
- `slot_index` is the persistent slot from `slot_persistence.rs` so dashboards can show stable identifiers
- `media.last_motion_at` is `null` if no motion has fired since process start; consumers should treat it as advisory, not authoritative

---

### `GET /api/v1/pairing/qr`

Current Matter commissioning QR code. Only returns image bytes when the commissioning window is open.

**Query params**
- `format` — `png` (default) or `text` (returns the underlying setup payload string)

**Response 200 (window open, `format=png`)**
```
Content-Type: image/png
Content-Length: <n>
<binary PNG bytes>
```

**Response 200 (window open, `format=text`)**
```json
{
  "setup_payload": "MT:Y.K9042C00KA0648G00",
  "manual_pairing_code": "34970112332",
  "discriminator": 3840,
  "vendor_id": "0xFFF1",
  "product_id": "0x8000",
  "expires_at": "2026-05-20T11:09:32Z"
}
```

**Response 404 (window closed)**
```json
{ "error": "commissioning_window_closed" }
```

**Implementation notes**
- rs-matter exposes the setup payload via `MatterMdnsService::commissioning_info()` or similar — exact API TBD at implementation time
- QR rendering uses an existing crate (`qrcode` or `qr2term` with image backend); ~100 KB binary cost
- The endpoint must respect rs-matter's commissioning state — never serve a stale QR

---

### `POST /api/v1/pairing/restart`

Re-open the commissioning window for `duration_seconds` (default 900 = 15 min).

**Request body**
```json
{ "duration_seconds": 900 }
```

**Response 200**
```json
{
  "commissioning_window_open": true,
  "expires_at": "2026-05-20T11:19:32Z"
}
```

**Response 409 (already commissioned + admin not granting re-pair)**
```json
{ "error": "controller_not_authorized" }
```

**Implementation notes**
- Re-opens commissioning via rs-matter's `open_commissioning_window()` if available — confirm at implementation time
- v1 has no auth, so this endpoint is the most security-sensitive on the surface. **Bind to 127.0.0.1 only by default**; require an explicit env var to bind elsewhere.
- A subsequent commissioning replaces the previous fabric only if the controller is authorized to do so — let rs-matter govern the policy

---

### `GET /api/v1/events` (SSE stream)

Server-Sent Events stream for live updates. Keeps consumers in sync without polling.

**Response**
```
Content-Type: text/event-stream
Cache-Control: no-store
Connection: keep-alive

event: motion
data: {"camera_id":"ABC123456","at":"2026-05-20T11:03:12Z"}

event: discovery
data: {"camera_id":"DEF789012","host":"192.168.1.11","action":"added"}

event: commissioning
data: {"action":"window_opened","expires_at":"2026-05-20T11:19:32Z"}

event: media
data: {"server":"mediamtx","reachable":false,"at":"2026-05-20T11:04:30Z"}
```

**Event types**
- `motion` — ONVIF MotionAlarm received, emitted alongside the OccupancySensing cluster update
- `discovery` — camera added or removed from the registry (`action: "added" | "removed"`)
- `commissioning` — window opened / closed / pairing succeeded / pairing failed
- `media` — media-server reachability changed
- `keepalive` — empty data, sent every 30s so proxies don't close the connection

**Implementation notes**
- Hook into the existing motion event pump in `crates/onvif-client/src/`
- Use `tokio::sync::broadcast` so multiple SSE consumers can attach without slowing the source loops
- Cap broadcast buffer at ~256 events; if a slow consumer overflows, send a `lagged` event and close that connection

---

## Crate placement

Two options, listed by preference:

### Option A — Add a small `crates/control-api/` crate

A new workspace member that:
- Depends on `bridge`, `matter-camera`, `onvif-client`, `media` (read-only access to their state)
- Owns the Axum router, the SSE broadcast channels, the QR rendering
- Exposes a single `pub async fn serve(state: AppState, addr: SocketAddr) -> Result<()>`

`crates/bridge/src/main.rs` calls `control_api::serve(...)` if `BRIDGE_HTTP_ENABLED=true`, otherwise no-op.

**Pros**
- Clean separation; CI can run the API crate's tests independently
- Doesn't bloat `bridge`'s already-large `main.rs`
- Easy to swap implementations or share with future tooling

**Cons**
- One more crate to maintain
- Cross-crate state sharing needs an `Arc<RwLock<...>>` shape worked out

### Option B — In-tree under `crates/bridge/src/http/`

A new module inside the existing bridge crate. Axum + handlers + SSE plumbing all live in one place.

**Pros**
- No workspace structural change
- State access is direct — no `Arc<RwLock<...>>` gymnastics

**Cons**
- `bridge` already pulls in heavy deps (rs-matter, tokio, embassy); Axum adds more
- Mixing protocol code with HTTP code in one crate makes the binary harder to reason about
- If someone wants to reuse the API surface in a different binary, the modules aren't extractable

**Recommendation:** Option A. The bridge crate is already complex; an HTTP surface deserves its own boundary.

---

## Configuration

New `.env` keys:

```
# HTTP control surface — disabled by default. Enable to expose
# /api/v1/* to localhost for dashboards / monitoring.
BRIDGE_HTTP_ENABLED=false

# Bind address. Default 127.0.0.1 (loopback only). Override only if
# the bridge needs to be reachable from another host on the LAN; in
# that case add auth (out of scope for v1) before binding 0.0.0.0.
BRIDGE_HTTP_BIND=127.0.0.1:8530

# Optional: shared-secret token required on every request. If unset,
# the API is open (only intended when bound to loopback).
# BRIDGE_HTTP_TOKEN=
```

Port `8530` is chosen because:
- Not a Matter / ONVIF / RTSP / WebRTC well-known port
- Not used by go2rtc (1984) or MediaMTX (9997)
- Easy to remember (`MAT` on a phone keypad → `628`; `8530` is unallocated and close)

---

## Security posture for v1

| Concern | Mitigation |
|---|---|
| Credentials leaked over HTTP | `rtsp_url_redacted` field, never expose raw URLs; reuse `redact_rtsp_url()` |
| Unauthorized commissioning trigger | Bind to `127.0.0.1` by default; explicit `BRIDGE_HTTP_TOKEN` for non-loopback |
| Denial-of-service via SSE | Cap broadcast buffer, hard-limit concurrent SSE connections to ~16 |
| Information disclosure via timing | Don't return whether a token is valid via timing — constant-time compare for `BRIDGE_HTTP_TOKEN` |
| Cross-origin abuse | Set `Access-Control-Allow-Origin` to the dashboard origin only if it's same-machine; otherwise return without CORS headers (browsers will block) |

---

## Testing strategy

Mirrors the existing crate test conventions:

- **Unit** — handlers as `axum::Router` instances driven by `tower::ServiceExt::oneshot` with mock state. Cover happy paths + the documented error responses.
- **Integration** — spin up the real server against a stub bridge state, hit it with `reqwest`. Verify the JSON schemas above match the responses exactly.
- **SSE** — open a connection with `reqwest::Client::get(...).header("Accept", "text/event-stream")`, feed three events through the broadcast channel, assert receipt.
- **Snapshot** — keep a `tests/snapshots/` directory of canonical responses (`status.json`, `cameras.json`, etc.) so schema drift is loud.

---

## Sequencing

This spec ships first; the Rust work follows.

1. **Now** — open this doc as a PR. Get sign-off on endpoint shapes + crate placement.
2. **After controller validation lands** (top of ROADMAP) — start `crates/control-api/` scaffold. `/api/v1/status` first, then `/api/v1/cameras`. Both read-only, both safe to ship without changing bridge behavior.
3. **Next** — `/api/v1/pairing/qr` + `/api/v1/pairing/restart`. Requires plumbing into rs-matter's commissioning lifecycle.
4. **Last** — `/api/v1/events` SSE. Largest change — needs the broadcast channel wired into the motion pump and discovery loop.
5. **Dashboard side** — once `/api/v1/status` + `/api/v1/cameras` are live, replace the "Live status — coming soon" card in MatterBridgeTab with the real values.

Each step is independently shippable. None of this is on the critical path for controller validation; it's a follow-on observability layer.

---

## Open questions

- **rs-matter API for commissioning window control** — exact function names and the lifecycle hooks for "window opened / closed" events need confirmation. The endpoints above assume they exist; the implementation PR will surface whatever rs-matter actually exposes.
- **QR rendering crate choice** — `qrcode` is widely used and has no binary dependency. ~30 KB cost. Confirm at implementation time vs `qr2term` (terminal-oriented, not what we want).
- **Whether the dashboard should poll `/status` or just consume the SSE stream** — both work. SSE is cheaper at scale, polling is simpler to debug. Recommend dashboard polls `/status` every 5s for stable widgets and subscribes to `/events` for motion-grade updates.
- **Should `/api/v1/cameras` include thumbnails?** Useful for the dashboard's IoT-style card layout. Defers to the `CaptureSnapshot` Matter cluster that already exists in `matter-camera`; the HTTP surface can just proxy the snapshot bytes when the cluster supports it.

---

## Out of scope (intentional)

- WebSocket as an alternative to SSE — SSE is enough for the read-only event stream
- gRPC — overkill for the surface we need
- OpenAPI / Swagger generation — small enough surface that hand-maintained docs (this file + the dashboard tab) stay in sync without tooling
- A management UI hosted by the bridge itself — the IoT Dashboard's MatterBridgeTab is the UI; the bridge stays headless
