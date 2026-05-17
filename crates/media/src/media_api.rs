//! Unified media-server API — abstracts go2rtc and MediaMTX behind a single type.
//!
//! `AnyMediaApi` is an enum that delegates to whichever backend was configured,
//! so `stream_manager` and `webrtc_session` remain backend-agnostic.

use crate::go2rtc_api::Go2RtcApi;
use crate::mediamtx_api::MediaMtxApi;

/// A media-server API client that can back either go2rtc or MediaMTX.
#[derive(Clone)]
pub enum AnyMediaApi {
    Go2Rtc(Go2RtcApi),
    MediaMtx(MediaMtxApi),
}

impl AnyMediaApi {
    /// Register an RTSP stream source under the given name.
    pub async fn add_stream(&self, name: &str, rtsp_url: &str) -> Result<(), String> {
        match self {
            Self::Go2Rtc(api) => api.add_stream(name, rtsp_url).await,
            Self::MediaMtx(api) => api.add_stream(name, rtsp_url).await,
        }
    }

    /// Unregister a stream. Tolerates 404.
    pub async fn remove_stream(&self, name: &str) -> Result<(), String> {
        match self {
            Self::Go2Rtc(api) => api.remove_stream(name).await,
            Self::MediaMtx(api) => api.remove_stream(name).await,
        }
    }

    /// Return true if the backend already has this stream with the expected URL.
    pub async fn stream_matches(&self, name: &str, expected_url: &str) -> bool {
        match self {
            Self::Go2Rtc(api) => api.stream_matches(name, expected_url).await,
            Self::MediaMtx(api) => api.stream_matches(name, expected_url).await,
        }
    }

    /// Exchange an SDP offer for an SDP answer via the backend.
    pub async fn exchange_sdp(&self, stream_name: &str, sdp_offer: &str) -> Result<String, String> {
        match self {
            Self::Go2Rtc(api) => api.exchange_sdp(stream_name, sdp_offer).await,
            Self::MediaMtx(api) => api.exchange_sdp(stream_name, sdp_offer).await,
        }
    }

    /// Capture a JPEG snapshot for a stream.
    pub async fn snapshot_jpeg(&self, stream_name: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::Go2Rtc(api) => api.snapshot_jpeg(stream_name).await,
            Self::MediaMtx(api) => api.snapshot_jpeg(stream_name).await,
        }
    }

    /// Check whether the backend is accepting requests.
    pub async fn is_ready(&self) -> bool {
        match self {
            Self::Go2Rtc(api) => api.is_ready().await,
            Self::MediaMtx(api) => api.is_ready().await,
        }
    }
}

/// Extract ICE candidates from an SDP answer string.
pub fn extract_ice_candidates(sdp: &str) -> Vec<String> {
    sdp.lines()
        .filter(|line| line.starts_with("a=candidate:"))
        .map(|line| line.trim().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ice_candidates_from_sdp() {
        let sdp = "\
v=0\r\n\
o=- 123456 2 IN IP4 127.0.0.1\r\n\
s=-\r\n\
t=0 0\r\n\
m=video 9 UDP/TLS/RTP/SAVPF 96\r\n\
a=candidate:1 1 UDP 2130706431 192.168.1.100 5004 typ host\r\n\
a=candidate:2 1 UDP 1694498815 203.0.113.5 5004 typ srflx raddr 192.168.1.100 rport 5004\r\n\
a=rtpmap:96 H264/90000\r\n";

        let candidates = extract_ice_candidates(sdp);
        assert_eq!(candidates.len(), 2);
        assert!(candidates[0].starts_with("a=candidate:1"));
        assert!(candidates[1].starts_with("a=candidate:2"));
    }

    #[test]
    fn test_extract_ice_candidates_empty() {
        let sdp = "\
v=0\r\n\
o=- 123456 2 IN IP4 127.0.0.1\r\n\
s=-\r\n\
t=0 0\r\n\
m=video 9 UDP/TLS/RTP/SAVPF 96\r\n\
a=rtpmap:96 H264/90000\r\n";

        let candidates = extract_ice_candidates(sdp);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_extract_ice_candidates_mixed() {
        let sdp = "\
v=0\r\n\
a=group:BUNDLE 0\r\n\
m=video 9 UDP/TLS/RTP/SAVPF 96\r\n\
a=mid:0\r\n\
a=candidate:1 1 UDP 2130706431 10.0.0.1 5004 typ host\r\n\
a=rtpmap:96 H264/90000\r\n\
a=fmtp:96 profile-level-id=42e01f\r\n\
a=candidate:2 1 TCP 1015022591 10.0.0.1 9 typ host tcptype active\r\n\
a=end-of-candidates\r\n";

        let candidates = extract_ice_candidates(sdp);
        assert_eq!(candidates.len(), 2);
        assert!(candidates[0].contains("typ host"));
        assert!(candidates[1].contains("TCP"));
    }
}
