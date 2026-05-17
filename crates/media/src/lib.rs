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
pub mod webrtc_session;

/// Redact credentials from an RTSP URL for safe logging.
///
/// Replaces the `user:password@` portion with `***@`.
/// Returns `"<invalid-url>"` if the URL cannot be parsed.
pub fn redact_rtsp_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut parsed) => {
            if !parsed.username().is_empty() || parsed.password().is_some() {
                let _ = parsed.set_username("***");
                let _ = parsed.set_password(None);
            }
            parsed.to_string()
        }
        Err(_) => "<invalid-url>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_rtsp_url_strips_credentials() {
        assert_eq!(
            redact_rtsp_url("rtsp://user:pass@192.168.1.10:554/stream1"),
            "rtsp://***@192.168.1.10:554/stream1"
        );
    }

    #[test]
    fn redact_rtsp_url_no_credentials_unchanged() {
        assert_eq!(
            redact_rtsp_url("rtsp://192.168.1.10:554/stream1"),
            "rtsp://192.168.1.10:554/stream1"
        );
    }

    #[test]
    fn redact_rtsp_url_rtsps_scheme() {
        assert_eq!(
            redact_rtsp_url("rtsps://admin:secret@cam.local/live"),
            "rtsps://***@cam.local/live"
        );
    }

    #[test]
    fn redact_rtsp_url_empty_string() {
        assert_eq!(redact_rtsp_url(""), "<invalid-url>");
    }
}
