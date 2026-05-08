//! MediaMTX HTTP API client — stream path management and WebRTC via WHEP.
//!
//! API endpoints (MediaMTX v1.x):
//! - POST   /v3/config/paths/add/{name}    — Add a path with an RTSP source
//! - DELETE /v3/config/paths/remove/{name} — Remove a path
//! - GET    /v3/config/paths/get/{name}    — Get path configuration
//! - GET    /v3/paths/list                 — List active paths (readiness probe)
//!
//! WHEP endpoint (WebRTC-HTTP Egress Protocol, RFC draft):
//! - POST   /whep/{name}                   — SDP offer → SDP answer exchange

use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Serialize)]
struct PathConfig {
    source: String,
}

#[derive(Debug, Deserialize)]
struct PathConfigResponse {
    source: Option<String>,
}

/// MediaMTX HTTP API client.
#[derive(Clone)]
pub struct MediaMtxApi {
    client: reqwest::Client,
    /// Base URL for the HTTP API (e.g., `http://localhost:9997`).
    api_base: String,
    /// Base URL for the WHEP endpoint (e.g., `http://localhost:8889`).
    whep_base: String,
}

impl MediaMtxApi {
    /// Create a new client pointing at the given host, API port, and WHEP port.
    pub fn new(host: &str, api_port: u16, whep_port: u16) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_base: format!("http://{host}:{api_port}"),
            whep_base: format!("http://{host}:{whep_port}"),
        }
    }

    /// Add or overwrite a path with an RTSP source URL.
    pub async fn add_stream(&self, name: &str, rtsp_url: &str) -> Result<(), String> {
        let url = format!("{}/v3/config/paths/add/{}", self.api_base, name);
        let body = PathConfig {
            source: rtsp_url.to_string(),
        };

        debug!(name, rtsp_url, "Registering path in MediaMTX");

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("POST /v3/config/paths/add failed: {e}"))?;

        // 200 OK (created/updated) or 400 if already exists with same config — treat both as ok.
        if !resp.status().is_success() {
            return Err(format!(
                "POST /v3/config/paths/add/{name} returned {}",
                resp.status()
            ));
        }

        Ok(())
    }

    /// Remove a path from MediaMTX. Tolerates 404 (path not found).
    pub async fn remove_stream(&self, name: &str) -> Result<(), String> {
        let url = format!("{}/v3/config/paths/remove/{}", self.api_base, name);

        debug!(name, "Removing path from MediaMTX");

        let resp = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| format!("DELETE /v3/config/paths/remove failed: {e}"))?;

        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            return Err(format!(
                "DELETE /v3/config/paths/remove/{name} returned {}",
                resp.status()
            ));
        }

        Ok(())
    }

    /// Return true if MediaMTX already has a path named `name` whose source
    /// matches `expected_url`. Used to skip redundant add calls.
    pub async fn stream_matches(&self, name: &str, expected_url: &str) -> bool {
        let url = format!("{}/v3/config/paths/get/{}", self.api_base, name);
        let Ok(resp) = self.client.get(&url).send().await else {
            return false;
        };
        if !resp.status().is_success() {
            return false;
        }
        let Ok(config) = resp.json::<PathConfigResponse>().await else {
            return false;
        };
        config
            .source
            .as_deref()
            .map(|s| s == expected_url)
            .unwrap_or(false)
    }

    /// Exchange an SDP offer for an SDP answer via MediaMTX's WHEP endpoint.
    ///
    /// WHEP (WebRTC-HTTP Egress Protocol) POST /whep/{path}:
    /// - Request: Content-Type: application/sdp, body = SDP offer
    /// - Response: 201 Created or 200 OK, body = SDP answer
    pub async fn exchange_sdp(&self, stream_name: &str, sdp_offer: &str) -> Result<String, String> {
        let url = format!("{}/whep/{}", self.whep_base, stream_name);

        debug!(
            stream_name,
            sdp_offer_len = sdp_offer.len(),
            "Exchanging SDP with MediaMTX WHEP"
        );

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/sdp")
            .body(sdp_offer.to_string())
            .send()
            .await
            .map_err(|e| format!("POST /whep/{stream_name} failed: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(format!(
                "POST /whep/{stream_name} returned {status}"
            ));
        }

        let sdp_answer = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read WHEP SDP answer: {e}"))?;

        debug!(
            stream_name,
            sdp_answer_len = sdp_answer.len(),
            "WHEP SDP exchange complete"
        );

        Ok(sdp_answer)
    }

    /// Check if MediaMTX is ready by polling the paths list endpoint.
    pub async fn is_ready(&self) -> bool {
        let url = format!("{}/v3/paths/list", self.api_base);
        match self.client.get(&url).send().await {
            Ok(r) if r.status().is_success() => {
                // Tolerate an empty body — readiness just means the API responded.
                true
            }
            _ => false,
        }
    }
}

/// Extract ICE candidates from an SDP string (shared with go2rtc path).
pub fn extract_ice_candidates(sdp: &str) -> Vec<String> {
    sdp.lines()
        .filter(|line| line.starts_with("a=candidate:"))
        .map(|line| line.trim().to_string())
        .collect()
}
