# Session Handoff — matter-onvif-bridge

**Date:** 2026-05-20
**Repo:** https://github.com/iamjairo/matter-onvif-bridge
**Branch:** `main` (all work merged)
**Worktree:** `/Volumes/Anthropic - Claude AI/matter-onvif-bridge/.claude/worktrees/modest-greider-c7fdde`

---

## What Was Done (across 2 sessions)

### PR #32 — Audit & fix: buildability, runtime bugs, cleanup, docs ✅ MERGED
- Registry entry/update handling fixes
- Boxed discovery event payload
- Startup stream snapshot behavior
- Tolerant media backend registration (400-tolerance)
- `.gitignore` / `.env.example` hygiene
- Amazon Alexa added as validation target

### PR #35 — Matter 1.5 clusters, security hardening, README remaster ✅ MERGED
**4 new Matter 1.5 optional camera clusters:**
- `WebRTCTransportRequestor` (0x0554) — functional SDP exchange callback
- `ZoneManagement` (0x0550) — in-memory zone CRUD
- `PushAvStreamTransport` (0x0555) — stub
- `Chime` (0x0556) — stub

**Security & hardening:**
- RTSP credential redaction via `redact_rtsp_url()` in `crates/media/src/lib.rs`
- CI workflow `permissions: contents: read` (resolved 3 code-scanning alerts)
- rs-matter pinned to commit `6574649f...`
- MediaMTX Docker image pinned to `1.12.2`
- go2rtc Docker image already pinned to `1.9.7`
- Regression tests for 400-tolerance (go2rtc + MediaMTX)
- `.gitignore` duplicate removed, `scripts/setup-pi.sh` deleted
- README.md fully remastered

### PR #34 — Dependabot: actions/checkout v4 → v6 ✅ MERGED
### PR #33 — Dependabot: Rust Docker image 1.87 → 1.95 ✅ MERGED
### PR #37 — Dependabot: rust 1.94 → 1.95 ❌ CLOSED (image doesn't exist yet)

### PR #36 — Full repo audit: 17 consistency issues fixed ✅ MERGED
**Critical:**
- Added missing MIT `LICENSE` file
- Fixed Dockerfile `rust:1.95` → `rust:1.94` (1.95 doesn't exist)

**Medium:**
- Fixed placeholder repo URL (`user/` → `iamjairo/`)
- Fixed `.env.example` `LOG_LEVEL` → `RUST_LOG`
- Added missing `go2rtc_manager.rs` and `mediamtx_manager.rs` to README tree
- Synced `install-mediamtx.sh` default version to `1.12.2`
- Bridge crate now uses workspace `rs-matter` dep (was duplicated inline)

**Low:**
- Stale Dependabot comment removed
- `area: raspberry-pi` label → `area: deployment`
- `dotenvy` moved to dev-dependencies in onvif-client
- Clippy `--all-targets` flag matched in README

**Nit:**
- Stale TODO removed from `client.rs`

### GitHub Issues #17–#23 — ALL CLOSED
| Issue | Title | Resolution |
|-------|-------|------------|
| #17 | Post-PR #14 hardening (meta) | All sub-issues resolved |
| #18 | Missing docker/go2rtc/go2rtc.yaml | File exists |
| #19 | RTSP credential redaction | `redact_rtsp_url()` added |
| #20 | CI workflows + Dependabot | ci.yml exists with permissions |
| #21 | Dependency pinning | rs-matter rev + MediaMTX image pinned |
| #22 | Regression tests | 400-tolerance tests added |
| #23 | Cleanup | gitignore, logging, poison handling |

### Dependabot Alerts
- Alert #3 (`rand` 0.8 unsoundness) — dismissed as tolerable risk

### ROADMAP.md — Updated
- Full Phase 1 completion history documented
- All PRs, issues, and Dependabot work tracked
- Next priorities clearly listed

---

## Current Repo State

- **Open issues:** 0
- **Open PRs:** 0
- **Open alerts:** 0
- **Tests:** 47 passing (13 bridge + 11 matter-camera + 18 media + 5 onvif-client)
- **Clippy:** clean (zero warnings, `--all-targets`)
- **Fmt:** clean
- **CI:** `.github/workflows/ci.yml` with `permissions: contents: read`

### Matter 1.5 Clusters Implemented (10 total)
| Cluster | ID | Status |
|---------|----|--------|
| CameraAvStreamManagement | 0x0551 | Functional |
| WebRTCTransportProvider | 0x0553 | Functional |
| WebRTCTransportRequestor | 0x0554 | Functional |
| ZoneManagement | 0x0550 | Functional |
| PushAvStreamTransport | 0x0555 | Stub |
| Chime | 0x0556 | Stub |
| OccupancySensing | 0x0406 | Functional |
| BridgedDeviceBasicInformation | — | Functional |
| Descriptor | — | Functional (rs-matter) |
| Groups | — | Functional (rs-matter) |

---

## What's Next (from ROADMAP.md)

### Immediate — Controller Validation
1. Validate commissioning in Google Home
2. Validate live view in Google Home
3. Validate motion/occupancy in Google Home
4. Repeat for Apple Home and Amazon Alexa
5. Update README with tested-controller results matrix

### Phase 2 — Capability Hardening
- Reduce placeholder AV attributes
- Improve audio/snapshot capability reporting
- Add smoke-test flows for WebRTC commands
- Audit ONVIF PTZ support

### Later / Optional
- Recording / Push AV upload flows
- Zone management persistence
- Chime behavior
- MediaMTX full-stack Docker profile

---

## Key Files to Know

| File | Purpose |
|------|---------|
| `crates/bridge/src/main.rs` | Main binary — Matter endpoint setup, data model, macros |
| `crates/matter-camera/src/` | All 7 custom Matter cluster implementations |
| `crates/media/src/lib.rs` | `redact_rtsp_url()` helper + media crate root |
| `crates/media/src/go2rtc_api.rs` | go2rtc REST API client (with tests) |
| `crates/media/src/mediamtx_api.rs` | MediaMTX REST + WHEP client (with tests) |
| `crates/onvif-client/src/` | ONVIF discovery, client, registry, motion pump |
| `.github/workflows/ci.yml` | CI: check, clippy, test, fmt |
| `ROADMAP.md` | Single source of planning truth |
| `.env.example` | All env vars documented |

## Git Config Note

Commits must use: `102307984+iamjairo@users.noreply.github.com` for both author and committer email.

## Build Note

External drive doesn't support incremental compilation locks. Always use `CARGO_INCREMENTAL=0`.
