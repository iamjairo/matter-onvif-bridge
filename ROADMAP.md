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

Primary priorities:

- stable commissioning
- live view
- motion / occupancy behavior
- snapshot support
- honest docs about what works and what is still limited

---

## Phase 1 Status

### Completed
- [x] Raspberry Pi setup/install path fix
- [x] manual RTSP fallback support
- [x] improved AV capability mapping from ONVIF-derived data
- [x] MediaMTX support path added
- [x] snapshot support added for supported backend path(s)
- [x] broader WebRTC command handling added
- [x] docs improved for deployment/configuration/architecture
- [x] wording cleanup to avoid overclaiming compatibility

### Notes
- Snapshot support is currently backend-dependent
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

### 2. Document real-world support
- [ ] Add a tested-controller matrix to README
- [ ] Mark what is:
  - [ ] tested working
  - [ ] partially working
  - [ ] unverified
- [ ] Document any Google Home quirks
- [ ] Document any Apple Home quirks

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

- [ ] recording / Push AV upload flows
- [ ] zone management
- [ ] chime-related behavior
- [ ] broader controller compatibility work
- [ ] more polished Docker/full-stack deployment symmetry

---

## Known Limitations

- WebRTC behavior is bridge-limited and controller-oriented
- `SolicitOffer` is handled conservatively (`deferredOffer=true`)
- `ProvideAnswer` / `ProvideICECandidates` are accepted/validated but do not currently drive backend renegotiation
- snapshot support is not identical across media backends
- this repo is not intended to fully clone the native CHIP Linux camera app

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

Phase 1 has been merged into `main` (PR #14). Remaining items:

- [x] merge final implementation into `main`
- [x] close extra/unneeded PRs
- [ ] begin Google Home validation
- [ ] begin Apple Home validation
- [ ] update README with actual tested-controller results

---

## Personal Notes

Use this file instead of trying to mentally track:
- multiple PRs
- multiple branches
- half-finished audit comments

If something is important, add it here.
