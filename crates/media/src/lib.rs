//! Media-server integration for RTSP-to-WebRTC bridging.
//!
//! Provides:
//! - `AnyMediaApi` — unified enum over go2rtc and MediaMTX
//! - REST API clients for go2rtc and MediaMTX
//! - Stream manager that syncs the camera registry with the active media server
//! - WebRTC session for SDP/ICE negotiation
//! - Process lifecycle management for both go2rtc and MediaMTX

pub mod go2rtc_api;
pub mod go2rtc_manager;
pub mod media_api;
pub mod mediamtx_api;
pub mod mediamtx_manager;
pub mod stream_manager;
pub(crate) mod url_redaction;
pub mod webrtc_session;
