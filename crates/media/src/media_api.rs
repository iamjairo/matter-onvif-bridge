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
