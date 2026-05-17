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
                stream_name,
                "Stream already registered in media server with matching URL, skipping"
            );
            continue;
        }

        match api.add_stream(&stream_name, &rtsp_url).await {
            Ok(()) => {
                info!(
                    camera_id = camera.id,
                    stream_name,
                    "Registered RTSP stream in media server (startup snapshot)"
                );
            }
            Err(e) => {
                error!(
                    camera_id = camera.id,
                    stream_name,
                    err = %e,
                    "Failed to register stream in media server (startup snapshot)"
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
                        "Stream already registered in media server with matching URL, skipping"
                    );
                    continue;
                }

                match api.add_stream(&stream_name, &rtsp_url).await {
                    Ok(()) => {
                        info!(
                            camera_id = camera.id,
                            stream_name, "Registered RTSP stream in media server"
                        );
                    }
                    Err(e) => {
                        error!(
                            camera_id = camera.id,
                            stream_name,
                            err = %e,
                            "Failed to register stream in media server"
                        );
                    }
                }
            }
            Ok(RegistryEvent::Removed(id)) => {
                let stream_name = sanitize_stream_name(&id);

                match api.remove_stream(&stream_name).await {
                    Ok(()) => {
                        info!(camera_id = id, stream_name, "Removed stream from media server");
                    }
                    Err(e) => {
                        error!(
                            camera_id = id,
                            stream_name,
                            err = %e,
                            "Failed to remove stream from media server"
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

/// Sanitize camera ID into a media-server-compatible stream name.
/// Replaces non-alphanumeric characters (except `-` and `_`) with underscores.
pub fn sanitize_stream_name(id: &str) -> String {
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
    use super::*;

    const TEST_USERNAME: &str = "admin";
    const TEST_PASSWORD: &str = "pass";

    #[test]
    fn test_sanitize_stream_name() {
        assert_eq!(sanitize_stream_name("camera-1"), "camera-1");
        assert_eq!(sanitize_stream_name("192.168.1.1:554"), "192_168_1_1_554");
        assert_eq!(sanitize_stream_name("ABC123"), "ABC123");
    }

    #[test]
    fn test_inject_credentials() {
        assert_eq!(
            inject_credentials("rtsp://192.168.1.1:554/stream", TEST_USERNAME, TEST_PASSWORD),
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
}
