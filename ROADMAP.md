# Roadmap

This roadmap is for my personal-use fork of `matter-onvif-bridge`.

The goal is to keep development simple and practical:
- use `main` as the main source of truth
- avoid long-lived extra branches where possible
- focus on real homelab usefulness over full spec parity
- track progress in one visual file

---

## Current Project Goal

Bridge ONVIF / RTSP cameras into Matter in a way that is useful for my homelab, especially for:

- Google Home
- Apple Home
- Amazon Alexa

Primary priorities:

- stable commissioning
- live view
- motion / occupancy behavior
- snapshot support
- honest docs about what works and what is still limited

---

## Phase 1 — Complete

### PR #14: Core bridge implementation
- [x] Raspberry Pi setup/install path fix
- [x] manual RTSP fallback support
- [x] improved AV capability mapping from ONVIF-derived data
- [x] MediaMTX support path added
- [x] snapshot support added for supported backend path(s)
- [x] broader WebRTC command handling added
- [x] docs improved for deployment/configuration/architecture
- [x] wording cleanup to avoid overclaiming compatibility

### PR #32: Audit fixes
- [x] registry entry/update handling
- [x] boxed discovery event payload
- [x] startup stream snapshot behavior
- [x] tolerant media backend registration logic (400-tolerance)
- [x] `.gitignore` / `.env.example` hygiene improvements
- [x] Amazon Alexa added as validation target

### PR #35: Clusters, security hardening, README remaster
- [x] **WebRTCTransportRequestor** (0x0554) — functional SDP exchange callback
- [x] **ZoneManagement** (0x0550) — in-memory zone CRUD with create/update/delete
- [x] **PushAvStreamTransport** (0x0555) — stub, reports zero transport connections
- [x] **Chime** (0x0556) — stub, reports zero chime sounds
- [x] RTSP credential redaction in all media backend logging (`redact_rtsp_url()`)
- [x] CI workflow permissions (`contents: read`) — resolved code-scanning alerts
- [x] rs-matter pinned to specific commit (`rev = 6574649f...`)
- [x] MediaMTX Docker image pinned to `1.12.2` (was `:latest`)
- [x] go2rtc Docker image already pinned to `1.9.7`
- [x] Regression tests for 400-tolerance in both go2rtc and MediaMTX
- [x] `.gitignore` duplicate `.env` entry removed
- [x] `scripts/setup-pi.sh` deleted (no longer relevant)
- [x] README.md fully remastered — CI badge, cluster status table, tighter layout

### Dependabot PRs (merged)
- [x] `actions/checkout` bumped v4 → v6 (PR #34)
- [x] Rust Docker image bumped 1.87 → 1.95 (PR #33)

### Dependabot alerts
- [x] Alert #3 (`rand` 0.8 unsoundness) — dismissed as tolerable risk; transitive dep from oxvif/rs-matter, not exploitable in this project

### GitHub issues #17–#23 — all closed
- [x] #17 — meta-issue: post-PR #14 hardening
- [x] #18 — missing docker/go2rtc/go2rtc.yaml
- [x] #19 — RTSP credential redaction
- [x] #20 — CI workflows + Dependabot mismatch
- [x] #21 — dependency pinning
- [x] #22 — regression tests for 400-tolerance
- [x] #23 — cleanup (gitignore, logging, poison handling)

### Notes
- Snapshot support is currently backend-dependent (go2rtc only)
- WebRTC support is still bridge-oriented, not full native-camera parity
- This project is still a bridge MVP+, not a full Matter camera reference implementation

---

## Next Priorities

### 1. Real controller validation
- [ ] Validate commissioning in Google Home
- [ ] Validate live view in Google Home
- [ ] Validate motion / occupancy behavior in Google Home
- [ ] Validate snapshot behavior in Google Home if surfaced

- [ ] Validate commissioning in Apple Home
- [ ] Validate live view in Apple Home
- [ ] Validate snapshot behavior in Apple Home if surfaced

- [ ] Validate commissioning in Amazon Alexa
- [ ] Validate live view in Amazon Alexa
- [ ] Validate motion / occupancy behavior in Amazon Alexa
- [ ] Validate snapshot behavior in Amazon Alexa if surfaced

### 2. Document real-world support
- [ ] Add a tested-controller matrix to README
- [ ] Mark what is:
  - [ ] tested working
  - [ ] partially working
  - [ ] unverified
- [ ] Document any Google Home quirks
- [ ] Document any Apple Home quirks
- [ ] Document any Amazon Alexa quirks

### 3. Snapshot hardening
- [ ] Test `CaptureSnapshot` end-to-end with real cameras
- [ ] Verify clear failure behavior for unsupported snapshot backends
- [ ] Add more tests for malformed / invalid snapshot requests
- [ ] Confirm snapshot docs match actual controller behavior

---

## Phase 2

### Capability hardening
- [ ] reduce remaining placeholder AV attributes
- [ ] improve audio capability reporting where possible
- [ ] improve snapshot capability reporting
- [ ] keep manual RTSP fallback conservative when metadata is missing

### Validation tooling
- [ ] keep using targeted test command:
  - [ ] `cargo test -p matter-camera -p media -p matter-onvif-bridge`
- [ ] add a simple developer validation checklist to README
- [ ] add smoke-test flow for:
  - [ ] video stream allocate
  - [ ] snapshot stream allocate
  - [ ] capture snapshot
  - [ ] `ProvideOffer`
  - [ ] `SolicitOffer`
  - [ ] `ProvideAnswer`
  - [ ] `ProvideICECandidates`

### PTZ / camera settings
- [ ] audit ONVIF PTZ support in existing client code
- [ ] determine minimum viable PTZ support
- [ ] decide whether to implement PTZ control or capability surfacing first

---

## Later / Optional

These are lower priority unless I actually need them:

- [ ] recording / Push AV upload flows (PushAvStreamTransport stub is in place)
- [ ] zone management enhancements (ZoneManagement CRUD is in place)
- [ ] chime behavior (Chime stub is in place)
- [ ] broader controller compatibility work
- [ ] more polished Docker/full-stack deployment symmetry (MediaMTX full-stack profile)

---

## Known Limitations

- WebRTC behavior is bridge-limited and controller-oriented
- `SolicitOffer` is handled conservatively (`deferredOffer=true`)
- `ProvideAnswer` / `ProvideICECandidates` are accepted/validated but do not currently drive backend renegotiation
- Snapshot support is not identical across media backends (go2rtc only)
- PushAvStreamTransport, Chime are stubs (report zero capabilities)
- ZoneManagement zones are in-memory only (not persisted across restarts)
- This repo is not intended to fully clone the native CHIP Linux camera app

---

## Simple Workflow

For this personal repo, keep things simple:

1. work from `main`
2. use a temporary PR/branch only when needed
3. merge back quickly
4. close extra PRs that are no longer needed
5. update this roadmap as the single source of planning truth

---

## Immediate Next Step

Phase 1 is complete. All issues closed, all PRs merged. Next:

- [ ] begin Google Home validation
- [ ] begin Apple Home validation
- [ ] begin Amazon Alexa validation
- [ ] update README with actual tested-controller results

---

## Personal Notes

Use this file instead of trying to mentally track:
- multiple PRs
- multiple branches
- half-finished audit comments

If something is important, add it here.
