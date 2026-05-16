//! Stream manager — syncs camera registry with media-server stream registration.
//!
//! Listens for camera Added/Removed events and registers/deregisters
//! RTSP streams in the active media server (go2rtc or MediaMTX).

use onvif_client::registry::{CameraRegistry, RegistryEvent};
use tracing::{error, info};

use crate::media_api::AnyMediaApi;

/// Run the stream manager loop, listening for registry events.
pub async fn run_stream_manager(
    registry: &CameraRegistry,
    api: AnyMediaApi,
    onvif_username: &str,
    onvif_password: &str,
) {
    // Subscribe before the startup snapshot so we don't miss events that
    // arrive while we're processing the initial batch.
    let mut rx = registry.subscribe();

    info!("Stream manager started — listening for camera events");

    // Register any cameras that were added to the registry before this task
    // had a chance to run (e.g. manual RTSP cameras added at startup).
    for camera in registry.get_all() {
        let stream_name = sanitize_stream_name(&camera.id);
        let rtsp_url = inject_credentials(&camera.stream_uri, onvif_username, onvif_password);

        if rtsp_url.is_empty() {
            error!(
                camera_id = camera.id,
                "No RTSP URL for camera, skipping stream registration"
            );
            continue;
        }

        if api.stream_matches(&stream_name, &rtsp_url).await {
            info!(
                camera_id = camera.id,
                stream_name, "Stream already registered in go2rtc with matching URL, skipping"
            );
            continue;
        }

        match api.add_stream(&stream_name, &rtsp_url).await {
            Ok(()) => {
                info!(
                    camera_id = camera.id,
                    stream_name, "Registered RTSP stream in go2rtc (startup snapshot)"
                );
            }
            Err(e) => {
                error!(
                    camera_id = camera.id,
                    stream_name,
                    err = %e,
                    "Failed to register stream in go2rtc (startup snapshot)"
                );
            }
        }
    }

    loop {
        match rx.recv().await {
            Ok(RegistryEvent::Added(camera)) | Ok(RegistryEvent::Updated(camera)) => {
                let stream_name = sanitize_stream_name(&camera.id);
                let rtsp_url =
                    inject_credentials(&camera.stream_uri, onvif_username, onvif_password);

                if rtsp_url.is_empty() {
                    error!(
                        camera_id = camera.id,
                        "No RTSP URL for camera, skipping stream registration"
                    );
                    continue;
                }

                if api.stream_matches(&stream_name, &rtsp_url).await {
                    info!(
                        camera_id = camera.id,
                        stream_name,
                        "Stream already registered in go2rtc with matching URL, skipping"
                    );
                    continue;
                }

                match api.add_stream(&stream_name, &rtsp_url).await {
                    Ok(()) => {
                        info!(
                            camera_id = camera.id,
                            stream_name, "Registered RTSP stream in go2rtc"
                        );
                    }
                    Err(e) => {
                        error!(
                            camera_id = camera.id,
                            stream_name,
                            err = %e,
                            "Failed to register stream in go2rtc"
                        );
                    }
                }
            }
            Ok(RegistryEvent::Removed(id)) => {
                let stream_name = sanitize_stream_name(&id);

                match api.remove_stream(&stream_name).await {
                    Ok(()) => {
                        info!(camera_id = id, stream_name, "Removed stream from go2rtc");
                    }
                    Err(e) => {
                        error!(
                            camera_id = id,
                            stream_name,
                            err = %e,
                            "Failed to remove stream from go2rtc"
                        );
                    }
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                error!(
                    missed = n,
                    "Stream manager missed events — registry channel lagged"
                );
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                info!("Registry channel closed, stream manager exiting");
                break;
            }
        }
    }
}

/// Sanitize camera ID into a go2rtc-compatible stream name.
/// Replaces non-alphanumeric characters (except - and _) with underscores.
fn sanitize_stream_name(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Inject ONVIF credentials into an RTSP URL if not already present.
fn inject_credentials(uri: &str, username: &str, password: &str) -> String {
    if uri.is_empty() {
        return String::new();
    }

    match url::Url::parse(uri) {
        Ok(mut parsed) => {
            if parsed.username().is_empty() {
                let _ = parsed.set_username(username);
                let _ = parsed.set_password(Some(password));
            }
            parsed.to_string()
        }
        Err(_) => {
            // Can't parse — return as-is
            uri.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::time::Duration;

    use onvif_client::registry::CameraRegistry;
    use onvif_client::types::{CameraDevice, DeviceInfo};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;
    use crate::go2rtc_api::Go2RtcApi;
    use crate::media_api::AnyMediaApi;

    const TEST_USERNAME: &str = "admin";
    const TEST_PASSWORD: &str = "pass";

    fn test_camera(id: &str, stream_uri: &str) -> CameraDevice {
        CameraDevice {
            id: id.to_string(),
            host: "127.0.0.1".to_string(),
            port: 80,
            device_info: DeviceInfo {
                manufacturer: "test".to_string(),
                model: "test".to_string(),
                firmware_version: "1.0".to_string(),
                serial_number: "123".to_string(),
                hardware_id: "abc".to_string(),
            },
            profiles: Vec::new(),
            stream_uri: stream_uri.to_string(),
            events_url: None,
            supports_motion: false,
        }
    }

    async fn spawn_scripted_go2rtc_server(
        responses: Vec<(u16, &'static str)>,
    ) -> Result<(u16, tokio::task::JoinHandle<Vec<String>>), io::Error> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let handle = tokio::spawn(async move {
            let mut requests = Vec::with_capacity(responses.len());
            for (status, body) in responses {
                let (mut socket, _) = listener.accept().await.expect("accept failed");
                let mut buf = [0_u8; 2048];
                let n = socket.read(&mut buf).await.expect("read failed");
                requests.push(String::from_utf8_lossy(&buf[..n]).to_string());

                let status_text = match status {
                    200 => "OK",
                    400 => "Bad Request",
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
            }
            requests
        });

        Ok((port, handle))
    }

    #[test]
    fn test_sanitize_stream_name() {
        assert_eq!(sanitize_stream_name("camera-1"), "camera-1");
        assert_eq!(sanitize_stream_name("192.168.1.1:554"), "192_168_1_1_554");
        assert_eq!(sanitize_stream_name("ABC123"), "ABC123");
    }

    #[test]
    fn test_inject_credentials() {
        assert_eq!(
            inject_credentials(
                "rtsp://192.168.1.1:554/stream",
                TEST_USERNAME,
                TEST_PASSWORD
            ),
            "rtsp://admin:pass@192.168.1.1:554/stream"
        );
        assert_eq!(
            inject_credentials(
                "rtsp://user:existing@192.168.1.1:554/stream",
                TEST_USERNAME,
                TEST_PASSWORD
            ),
            "rtsp://user:existing@192.168.1.1:554/stream"
        );
        assert_eq!(inject_credentials("", TEST_USERNAME, TEST_PASSWORD), "");
    }

    #[tokio::test]
    async fn startup_snapshot_registers_existing_camera() {
        let (port, requests_handle) = spawn_scripted_go2rtc_server(vec![(200, "{}"), (200, "{}")])
            .await
            .unwrap();
        let registry = CameraRegistry::new(8);
        registry.add_camera(test_camera("cam.1", "rtsp://10.0.0.2/live"));
        let api = AnyMediaApi::Go2Rtc(Go2RtcApi::new("127.0.0.1", port));
        let registry_for_task = registry.clone();

        let manager_handle = tokio::spawn(async move {
            run_stream_manager(&registry_for_task, api, TEST_USERNAME, TEST_PASSWORD).await
        });

        let requests = tokio::time::timeout(Duration::from_secs(2), requests_handle)
            .await
            .unwrap()
            .unwrap();
        assert!(requests[0].starts_with("GET /api/streams "));
        assert!(requests[1].starts_with(
            "PUT /api/streams?name=cam_1&src=rtsp%3A%2F%2Fadmin%3Apass%4010.0.0.2%2Flive "
        ));

        manager_handle.abort();
        let _ = manager_handle.await;
    }

    #[tokio::test]
    async fn startup_snapshot_skips_add_when_stream_already_matches() {
        let (port, requests_handle) = spawn_scripted_go2rtc_server(vec![(
            200,
            r#"{"cam_1":{"producers":[{"url":"rtsp://admin:pass@10.0.0.2/live"}]}}"#,
        )])
        .await
        .unwrap();
        let registry = CameraRegistry::new(8);
        registry.add_camera(test_camera("cam.1", "rtsp://10.0.0.2/live"));
        let api = AnyMediaApi::Go2Rtc(Go2RtcApi::new("127.0.0.1", port));
        let registry_for_task = registry.clone();

        let manager_handle = tokio::spawn(async move {
            run_stream_manager(&registry_for_task, api, TEST_USERNAME, TEST_PASSWORD).await
        });

        let requests = tokio::time::timeout(Duration::from_secs(2), requests_handle)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("GET /api/streams "));

        manager_handle.abort();
        let _ = manager_handle.await;
    }
}
