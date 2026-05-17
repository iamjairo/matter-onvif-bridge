//! go2rtc REST API client — stream registration and WebRTC SDP exchange.
//!
//! API endpoints (go2rtc v1.9+):
//! - PUT  /api/streams?name={name}     — Register RTSP stream source
//! - DELETE /api/streams?name={name}   — Unregister stream
//! - POST /api/webrtc?src={name}       — SDP offer/answer exchange
//! - GET  /api/streams                 — List registered streams

use std::collections::HashMap;

use serde::Deserialize;
use tracing::debug;

use crate::redact_rtsp_url;

#[derive(Debug, Deserialize)]
struct StreamProducer {
    url: String,
}

#[derive(Debug, Deserialize)]
struct StreamInfo {
    #[serde(default)]
    producers: Vec<StreamProducer>,
}

/// go2rtc REST API client.
#[derive(Clone)]
pub struct Go2RtcApi {
    client: reqwest::Client,
    base_url: String,
}

impl Go2RtcApi {
    /// Create a new API client pointing at the given host and port.
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: format!("http://{}:{}", host, port),
        }
    }

    /// Register an RTSP stream source in go2rtc.
    /// go2rtc 1.9+ expects the source URL as a `src` query parameter.
    ///
    /// go2rtc attempts to persist the stream to its config file on every PUT.
    /// When the config file is mounted read-only (the recommended setup), go2rtc
    /// returns 400 but **still registers the stream in memory**.  We detect this
    /// case by confirming the stream exists in the API after the 400 and treat it
    /// as a warning rather than a hard error.
    pub async fn add_stream(&self, name: &str, rtsp_url: &str) -> Result<(), String> {
        let url = format!(
            "{}/api/streams?name={}&src={}",
            self.base_url,
            urlencoding::encode(name),
            urlencoding::encode(rtsp_url),
        );

        debug!(name, redacted_url = %redact_rtsp_url(rtsp_url), "Registering stream in go2rtc");

        let resp = self
            .client
            .put(&url)
            .send()
            .await
            .map_err(|e| format!("PUT /api/streams failed: {e}"))?;

        if !resp.status().is_success() {
            // go2rtc returns 400 when it cannot persist the stream to the config
            // file (e.g. the file is mounted read-only), but it still registers
            // the stream in memory.  Confirm the stream is actually there before
            // surfacing an error.
            if resp.status().as_u16() == 400 && self.stream_matches(name, rtsp_url).await {
                debug!(
                    name,
                    "go2rtc returned 400 (config file is read-only) but stream is registered in memory"
                );
                return Ok(());
            }
            return Err(format!("PUT /api/streams returned {}", resp.status()));
        }

        Ok(())
    }

    /// Unregister a stream from go2rtc. Tolerates 404 (stream not found).
    pub async fn remove_stream(&self, name: &str) -> Result<(), String> {
        let url = format!(
            "{}/api/streams?name={}",
            self.base_url,
            urlencoding::encode(name)
        );

        debug!(name, "Removing stream from go2rtc");

        let resp = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| format!("DELETE /api/streams failed: {e}"))?;

        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            return Err(format!("DELETE /api/streams returned {}", resp.status()));
        }

        Ok(())
    }

    /// Exchange an SDP offer for an SDP answer via go2rtc's WebRTC endpoint.
    pub async fn exchange_sdp(&self, stream_name: &str, sdp_offer: &str) -> Result<String, String> {
        let url = format!(
            "{}/api/webrtc?src={}",
            self.base_url,
            urlencoding::encode(stream_name)
        );

        debug!(
            stream_name,
            sdp_offer_len = sdp_offer.len(),
            "Exchanging SDP with go2rtc"
        );

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/sdp")
            .body(sdp_offer.to_string())
            .send()
            .await
            .map_err(|e| format!("POST /api/webrtc failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("POST /api/webrtc returned {}", resp.status()));
        }

        let sdp_answer = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read SDP answer: {e}"))?;

        debug!(
            stream_name,
            sdp_answer_len = sdp_answer.len(),
            "SDP exchange complete"
        );

        Ok(sdp_answer)
    }

    /// Capture a JPEG snapshot for a stream via go2rtc.
    pub async fn snapshot_jpeg(&self, stream_name: &str) -> Result<Vec<u8>, String> {
        let url = format!(
            "{}/api/frame.jpeg?src={}",
            self.base_url,
            urlencoding::encode(stream_name)
        );

        debug!(stream_name, "Capturing JPEG snapshot via go2rtc");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("GET /api/frame.jpeg failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("GET /api/frame.jpeg returned {}", resp.status()));
        }

        resp.bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|e| format!("Failed to read JPEG snapshot body: {e}"))
    }

    /// Return true if go2rtc already has a stream named `name` whose first
    /// producer URL matches `expected_url`. Used to skip redundant PUTs that
    /// would otherwise return 400 Bad Request from go2rtc.
    pub async fn stream_matches(&self, name: &str, expected_url: &str) -> bool {
        let url = format!("{}/api/streams", self.base_url);
        let Ok(resp) = self.client.get(&url).send().await else {
            return false;
        };
        if !resp.status().is_success() {
            return false;
        }
        let Ok(streams) = resp.json::<HashMap<String, StreamInfo>>().await else {
            return false;
        };
        streams
            .get(name)
            .and_then(|s| s.producers.first())
            .map(|p| p.url == expected_url)
            .unwrap_or(false)
    }

    /// Check if go2rtc is ready by polling the API endpoint.
    pub async fn is_ready(&self) -> bool {
        let url = format!("{}/api", self.base_url);
        matches!(self.client.get(&url).send().await, Ok(r) if r.status().is_success())
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::Go2RtcApi;

    async fn spawn_single_request_server(
        status: u16,
        body: &'static [u8],
    ) -> Result<(u16, tokio::task::JoinHandle<String>), io::Error> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();

        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept failed");
            let mut buf = [0_u8; 2048];
            let n = socket.read(&mut buf).await.expect("read failed");
            let request = String::from_utf8_lossy(&buf[..n]).to_string();

            let status_text = match status {
                200 => "OK",
                404 => "Not Found",
                _ => "OK",
            };
            let response = format!(
                "HTTP/1.1 {status} {status_text}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("response head write failed");
            socket
                .write_all(body)
                .await
                .expect("response body write failed");

            request
        });

        Ok((port, handle))
    }

    async fn spawn_two_request_server(
        status1: u16,
        body1: &'static [u8],
        status2: u16,
        body2: &'static [u8],
    ) -> Result<(u16, tokio::task::JoinHandle<(String, String)>), io::Error> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();

        let handle = tokio::spawn(async move {
            // First request
            let (mut socket, _) = listener.accept().await.expect("accept 1 failed");
            let mut buf = [0_u8; 4096];
            let n = socket.read(&mut buf).await.expect("read 1 failed");
            let request1 = String::from_utf8_lossy(&buf[..n]).to_string();

            let status_text1 = match status1 {
                200 => "OK",
                400 => "Bad Request",
                404 => "Not Found",
                _ => "OK",
            };
            let response1 = format!(
                "HTTP/1.1 {status1} {status_text1}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body1.len()
            );
            socket
                .write_all(response1.as_bytes())
                .await
                .expect("response 1 head write failed");
            socket
                .write_all(body1)
                .await
                .expect("response 1 body write failed");
            drop(socket);

            // Second request
            let (mut socket, _) = listener.accept().await.expect("accept 2 failed");
            let n = socket.read(&mut buf).await.expect("read 2 failed");
            let request2 = String::from_utf8_lossy(&buf[..n]).to_string();

            let status_text2 = match status2 {
                200 => "OK",
                400 => "Bad Request",
                404 => "Not Found",
                _ => "OK",
            };
            let response2 = format!(
                "HTTP/1.1 {status2} {status_text2}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body2.len()
            );
            socket
                .write_all(response2.as_bytes())
                .await
                .expect("response 2 head write failed");
            socket
                .write_all(body2)
                .await
                .expect("response 2 body write failed");

            (request1, request2)
        });

        Ok((port, handle))
    }

    #[tokio::test]
    async fn snapshot_jpeg_uses_expected_endpoint_and_returns_bytes() {
        let jpeg_bytes = b"\xFF\xD8\xFF\xD9";
        let (port, handle) = spawn_single_request_server(200, jpeg_bytes).await.unwrap();
        let api = Go2RtcApi::new("127.0.0.1", port);

        let snapshot = api.snapshot_jpeg("front-door").await.unwrap();
        assert_eq!(snapshot, jpeg_bytes);

        let request = handle.await.unwrap();
        assert!(request.starts_with("GET /api/frame.jpeg?src=front-door "));
    }

    #[tokio::test]
    async fn add_stream_tolerates_400_when_stream_already_registered() {
        // go2rtc returns 400 when the config file is read-only, then the client
        // calls GET /api/streams to confirm the stream is registered in memory.
        let streams_json = br#"{"cam1":{"producers":[{"url":"rtsp://u:p@192.168.1.10:554/s"}]}}"#;
        let (port, handle) =
            spawn_two_request_server(400, b"config is read-only", 200, streams_json)
                .await
                .unwrap();
        let api = Go2RtcApi::new("127.0.0.1", port);

        let result = api
            .add_stream("cam1", "rtsp://u:p@192.168.1.10:554/s")
            .await;
        assert!(result.is_ok(), "expected Ok but got: {result:?}");

        let (req1, req2) = handle.await.unwrap();
        assert!(req1.starts_with("PUT /api/streams?"));
        assert!(req2.starts_with("GET /api/streams "));
    }
}
