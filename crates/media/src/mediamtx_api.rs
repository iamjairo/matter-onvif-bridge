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
            return Err(format!("POST /whep/{stream_name} returned {status}"));
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

    /// Capture a JPEG snapshot for a stream.
    ///
    /// MediaMTX does not expose a direct "grab frame as JPEG" API in this bridge flow.
    pub async fn snapshot_jpeg(&self, stream_name: &str) -> Result<Vec<u8>, String> {
        Err(format!(
            "JPEG snapshot capture is not supported by MediaMTX backend for stream '{stream_name}'"
        ))
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::MediaMtxApi;

    struct MockRequest {
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: String,
    }

    async fn read_http_request(
        stream: &mut tokio::net::TcpStream,
    ) -> Result<MockRequest, io::Error> {
        let mut buf = Vec::<u8>::with_capacity(2048);
        let mut chunk = [0_u8; 512];
        let header_end = loop {
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "socket closed",
                ));
            }
            buf.extend_from_slice(&chunk[..n]);
            if let Some(idx) = find_bytes(&buf, b"\r\n\r\n") {
                break idx;
            }
        };

        let headers_raw = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let mut lines = headers_raw.lines();
        let request_line = lines
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request line"))?;
        let mut request_line_parts = request_line.split_whitespace();
        let method = request_line_parts
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing method"))?
            .to_string();
        let path = request_line_parts
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing path"))?
            .to_string();

        let mut headers = HashMap::<String, String>::new();
        for line in lines {
            if let Some((name, value)) = line.split_once(':') {
                headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
            }
        }

        let content_length = headers
            .get("content-length")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);

        let body_start = header_end + 4;
        while buf.len() < body_start + content_length {
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }

        let body =
            String::from_utf8_lossy(&buf[body_start..body_start + content_length]).to_string();
        Ok(MockRequest {
            method,
            path,
            headers,
            body,
        })
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    async fn spawn_single_request_server(
        status: u16,
        body: &'static str,
    ) -> Result<(u16, tokio::task::JoinHandle<MockRequest>), io::Error> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();

        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept failed");
            let req = read_http_request(&mut socket)
                .await
                .expect("request parsing failed");

            let status_text = match status {
                200 => "OK",
                201 => "Created",
                404 => "Not Found",
                500 => "Internal Server Error",
                _ => "OK",
            };
            let response = format!(
                "HTTP/1.1 {status} {status_text}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("response write failed");
            req
        });

        Ok((port, handle))
    }

    #[tokio::test]
    async fn readiness_probe_returns_true_on_200() {
        let (api_port, handle) = spawn_single_request_server(200, "{}").await.unwrap();
        let api = MediaMtxApi::new("127.0.0.1", api_port, api_port);

        assert!(api.is_ready().await);

        let req = handle.await.unwrap();
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/v3/paths/list");
    }

    #[tokio::test]
    async fn add_stream_posts_expected_path_and_payload() {
        let (api_port, handle) = spawn_single_request_server(200, "{}").await.unwrap();
        let api = MediaMtxApi::new("127.0.0.1", api_port, api_port);

        api.add_stream("cam1", "rtsp://u:p@127.0.0.2:554/live")
            .await
            .unwrap();

        let req = handle.await.unwrap();
        assert_eq!(req.method, "POST");
        assert_eq!(req.path, "/v3/config/paths/add/cam1");
        assert_eq!(
            req.headers
                .get("content-type")
                .map(String::as_str)
                .unwrap_or(""),
            "application/json"
        );
        assert!(
            req.body
                .contains("\"source\":\"rtsp://u:p@127.0.0.2:554/live\"")
        );
    }

    #[tokio::test]
    async fn remove_stream_success_path() {
        let (api_port, handle) = spawn_single_request_server(200, "{}").await.unwrap();
        let api = MediaMtxApi::new("127.0.0.1", api_port, api_port);

        api.remove_stream("cam1").await.unwrap();

        let req = handle.await.unwrap();
        assert_eq!(req.method, "DELETE");
        assert_eq!(req.path, "/v3/config/paths/remove/cam1");
    }

    #[tokio::test]
    async fn remove_stream_tolerates_404() {
        let (api_port, _handle) = spawn_single_request_server(404, "{}").await.unwrap();
        let api = MediaMtxApi::new("127.0.0.1", api_port, api_port);

        api.remove_stream("missing").await.unwrap();
    }

    #[tokio::test]
    async fn whep_exchange_posts_sdp_and_returns_answer() {
        let (whep_port, whep_handle) = spawn_single_request_server(201, "v=0\r\na=answer\r\n")
            .await
            .unwrap();
        let api = MediaMtxApi::new("127.0.0.1", 1, whep_port);

        let answer = api
            .exchange_sdp("cam1", "v=0\r\na=offer\r\n")
            .await
            .unwrap();

        assert_eq!(answer, "v=0\r\na=answer\r\n");

        let req = whep_handle.await.unwrap();
        assert_eq!(req.method, "POST");
        assert_eq!(req.path, "/whep/cam1");
        assert_eq!(
            req.headers
                .get("content-type")
                .map(String::as_str)
                .unwrap_or(""),
            "application/sdp"
        );
        assert_eq!(req.body, "v=0\r\na=offer\r\n");
    }

    #[tokio::test]
    async fn snapshot_jpeg_returns_explicit_unsupported_error() {
        let api = MediaMtxApi::new("127.0.0.1", 9997, 8889);
        let err = api.snapshot_jpeg("cam1").await.unwrap_err();
        assert!(err.contains("not supported"));
    }
}
