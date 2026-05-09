//! Bridges ONVIF discovery and the configured media server to the Matter camera endpoint states.
//!
//! Runs on a separate tokio runtime thread that:
//! 1. Starts the media server manager (go2rtc or MediaMTX) and waits for readiness
//! 2. Runs ONVIF WS-Discovery loop
//! 3. Feeds discovered cameras into the CameraRegistry
//! 4. Registers RTSP streams in the media server via StreamManager
//! 5. Populates pre-allocated camera endpoint states with real ONVIF data
//! 6. Stores the media API and slot map for WebRTC command handling

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use matter_camera::cluster_occupancy::OccupancyDataver;
use matter_camera::types::{CameraEndpointState, VideoResolution, VideoSensorParams};
use media::go2rtc_api::Go2RtcApi;
use media::go2rtc_manager::{Go2RtcManager, Go2RtcMode};
use media::media_api::AnyMediaApi;
use media::mediamtx_api::MediaMtxApi;
use media::mediamtx_manager::{MediaMtxManager, MediaMtxMode};
use onvif_client::discovery::{DiscoveryConfig, DiscoveryEvent, DiscoveryMode};
use onvif_client::motion::{MotionPumpConfig, spawn_motion_pump};
use onvif_client::registry::{CameraRegistry, RegistryEvent};
use onvif_client::types::CameraDevice;
use tokio::sync::mpsc;

use crate::config::{self, Config, ManualRtspCameraConfig, MediaConfig};
use crate::slot_persistence::SlotMap;
use crate::{MAX_CAMERAS, WITH_OCCUPANCY_CAMERAS};

const FALLBACK_VIDEO_WIDTH: u16 = 1280;
const FALLBACK_VIDEO_HEIGHT: u16 = 720;
const FALLBACK_VIDEO_FPS: u16 = 15;

/// Shared state accessible from the Matter handler thread for WebRTC negotiation.
#[derive(Clone)]
pub struct MediaBridge {
    /// Media-server API client (go2rtc or MediaMTX) for SDP exchange.
    pub api: AnyMediaApi,
    /// Maps camera ID → endpoint slot index (0-based, endpoint = slot + 2).
    pub slot_map: Arc<RwLock<HashMap<String, usize>>>,
    /// Maps endpoint slot index → stream name for the media server.
    pub stream_names: Arc<RwLock<HashMap<usize, String>>>,
}

/// Start the ONVIF + media-server bridge on a separate tokio runtime thread.
///
/// Returns a `MediaBridge` that can be used by the Matter WebRTC handler
/// to perform SDP negotiation via the configured media server.
pub fn start_onvif_bridge(
    cfg: &Config,
    camera_states: &[Arc<RwLock<CameraEndpointState>>],
    occupancy_datavers: &[OccupancyDataver],
    registry: CameraRegistry,
) -> MediaBridge {
    // Build the unified API client and start the media-server manager.
    let (media_api, media_server_label): (AnyMediaApi, &'static str) = match &cfg.media {
        MediaConfig::Go2Rtc(go2rtc_cfg) => {
            let api = AnyMediaApi::Go2Rtc(Go2RtcApi::new(&go2rtc_cfg.host, go2rtc_cfg.api_port));
            (api, "go2rtc")
        }
        MediaConfig::MediaMtx(mtx_cfg) => {
            let api = AnyMediaApi::MediaMtx(MediaMtxApi::new(
                &mtx_cfg.host,
                mtx_cfg.api_port,
                mtx_cfg.whep_port,
            ));
            (api, "mediamtx")
        }
    };

    tracing::info!("Media server: {media_server_label}");

    let media_bridge = MediaBridge {
        api: media_api,
        slot_map: Arc::new(RwLock::new(HashMap::new())),
        stream_names: Arc::new(RwLock::new(HashMap::new())),
    };

    let discovery_config = DiscoveryConfig {
        username: cfg.onvif.username.clone(),
        password: cfg.onvif.password.clone(),
        scan_interval: Duration::from_millis(cfg.onvif.discovery_interval_ms),
        mode: match cfg.onvif.discovery_mode {
            config::DiscoveryMode::Static => DiscoveryMode::Static,
            config::DiscoveryMode::Auto => DiscoveryMode::Auto,
        },
        static_cameras: cfg.onvif.static_cameras.clone(),
    };

    let states = camera_states.to_vec();
    let occupancy_datavers = occupancy_datavers.to_vec();
    let registry_clone = registry.clone();
    let bridge_clone = media_bridge.clone();
    let onvif_username = cfg.onvif.username.clone();
    let onvif_password = cfg.onvif.password.clone();
    let camera_names = cfg.onvif.camera_names.clone();
    let manual_rtsp_cameras = cfg.manual_rtsp_cameras.clone();
    let storage_dir = std::path::PathBuf::from(&cfg.matter.storage_path);
    let media_cfg = cfg.media.clone();

    std::thread::Builder::new()
        .name("onvif-media-bridge".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            rt.block_on(async move {
                log::info!("ONVIF/media bridge thread started");

                // 1. Start media server (go2rtc or MediaMTX)
                let start_result = match media_cfg {
                    MediaConfig::Go2Rtc(ref c) => {
                        let mode = match c.mode {
                            config::Go2RtcMode::External => Go2RtcMode::External,
                            config::Go2RtcMode::Local => Go2RtcMode::Local,
                        };
                        Go2RtcManager::new(&c.host, c.api_port, c.webrtc_port, mode, &c.path)
                            .start()
                            .await
                    }
                    MediaConfig::MediaMtx(ref c) => {
                        let mode = match c.mode {
                            config::MediaMtxMode::External => MediaMtxMode::External,
                            config::MediaMtxMode::Local => MediaMtxMode::Local,
                        };
                        MediaMtxManager::new(&c.host, c.api_port, c.whep_port, mode, &c.path)
                            .start()
                            .await
                    }
                };
                if let Err(e) = start_result {
                    log::error!("Failed to start media server: {e}");
                    // Continue anyway — the media server may come up later
                }

                log::info!("Media server started, launching stream manager and ONVIF discovery");

                // 2. Spawn stream manager (registers RTSP streams in the media server)
                let api_for_streams = bridge_clone.api.clone();
                let registry_for_streams = registry_clone.clone();
                let stream_user = onvif_username.clone();
                let stream_pass = onvif_password.clone();
                tokio::spawn(async move {
                    media::stream_manager::run_stream_manager(
                        &registry_for_streams,
                        api_for_streams,
                        &stream_user,
                        &stream_pass,
                    )
                    .await;
                });

                // 3. Spawn ONVIF discovery
                let (discovery_tx, mut discovery_rx) = mpsc::channel(64);
                tokio::spawn(onvif_client::discovery::run_discovery(
                    discovery_config,
                    discovery_tx,
                ));

                // 4. Process discovery events → registry → state population
                let mut registry_rx = registry_clone.subscribe();
                // Persistent slot allocator: keyed by camera id (ONVIF
                // serial). Stable across restarts so Google Home room
                // assignments don't drift when discovery order changes.
                let mut slot_map = SlotMap::load(
                    &storage_dir,
                    MAX_CAMERAS,
                    WITH_OCCUPANCY_CAMERAS,
                );
                // Tracks the spawned motion pump per slot so we can abort on remove.
                let mut motion_tasks: HashMap<usize, tokio::task::JoinHandle<()>> = HashMap::new();

                if !manual_rtsp_cameras.is_empty() {
                    log::info!(
                        "Registering {} manual RTSP fallback camera(s)",
                        manual_rtsp_cameras.len()
                    );
                    for camera in &manual_rtsp_cameras {
                        registry_clone.add_camera(manual_rtsp_camera_device(camera));
                    }
                }

                loop {
                    tokio::select! {
                        Some(event) = discovery_rx.recv() => {
                            match event {
                                DiscoveryEvent::CameraFound(camera) => {
                                    registry_clone.add_camera(*camera);
                                }
                                DiscoveryEvent::CameraLost(id) => {
                                    registry_clone.remove_camera(&id);
                                }
                                DiscoveryEvent::CameraUnreachable(_) | DiscoveryEvent::Error(_) => {}
                            }
                        }
                        Ok(event) = registry_rx.recv() => {
                            match event {
                                RegistryEvent::Added(camera) => {
                                    let slot = slot_map
                                        .assign(&camera.id, camera.supports_motion);

                                    if let Some(slot) = slot {
                                        // Update slot maps
                                        let stream_name = sanitize_stream_name(&camera.id);
                                        if let Ok(mut map) = bridge_clone.slot_map.write() {
                                            map.insert(camera.id.clone(), slot);
                                        }
                                        if let Ok(mut map) = bridge_clone.stream_names.write() {
                                            map.insert(slot, stream_name);
                                        }

                                        // Prefer serial-keyed lookup (stable across IP changes),
                                        // fall back to host-keyed.
                                        let friendly_name = camera_names
                                            .get(&camera.device_info.serial_number)
                                            .or_else(|| camera_names.get(&camera.id))
                                            .or_else(|| camera_names.get(&camera.host))
                                            .cloned();
                                        populate_camera_state(
                                            &states[slot],
                                            &camera,
                                            friendly_name.as_deref(),
                                        );
                                        log::info!(
                                            "Camera '{}' ({}) → endpoint {} (motion={}), stream registered",
                                            friendly_name.as_deref().unwrap_or(&camera.device_info.model),
                                            camera.id,
                                            slot + 2,
                                            camera.supports_motion,
                                        );

                                        // If this slot supports occupancy and the camera advertised
                                        // a MotionAlarm topic, spawn the PullPoint pump now.
                                        if camera.supports_motion
                                            && slot < WITH_OCCUPANCY_CAMERAS
                                            && let Some(events_url) = camera.events_url.clone()
                                        {
                                            let pump_cfg = MotionPumpConfig {
                                                host: camera.host.clone(),
                                                port: camera.port,
                                                username: onvif_username.clone(),
                                                password: onvif_password.clone(),
                                                events_url,
                                                label: friendly_name.clone().unwrap_or_else(|| {
                                                    format!(
                                                        "{} {} @ {}",
                                                        camera.device_info.manufacturer,
                                                        camera.device_info.model,
                                                        camera.host
                                                    )
                                                }),
                                            };
                                            let state_for_pump = Arc::clone(&states[slot]);
                                            let dataver_for_pump =
                                                occupancy_datavers[slot].clone();
                                            let handle = spawn_motion_pump(
                                                pump_cfg,
                                                move |motion| {
                                                    let Ok(mut s) = state_for_pump.write() else {
                                                        return;
                                                    };
                                                    if s.motion_detected == motion {
                                                        return;
                                                    }
                                                    s.motion_detected = motion;
                                                    dataver_for_pump.bump();
                                                },
                                            );
                                            motion_tasks.insert(slot, handle);
                                        }
                                    } else {
                                        log::warn!(
                                            "No endpoint slots left for camera {} (max {})",
                                            camera.id,
                                            MAX_CAMERAS
                                        );
                                    }
                                }
                                RegistryEvent::Updated(camera) => {
                                    if let Ok(map) = bridge_clone.slot_map.read()
                                        && let Some(&slot) = map.get(&camera.id)
                                    {
                                        let friendly_name = camera_names
                                            .get(&camera.device_info.serial_number)
                                            .or_else(|| camera_names.get(&camera.id))
                                            .or_else(|| camera_names.get(&camera.host))
                                            .map(String::as_str);
                                        populate_camera_state(
                                            &states[slot],
                                            &camera,
                                            friendly_name,
                                        );
                                    }
                                }
                                RegistryEvent::Removed(id) => {
                                    if let Ok(mut map) = bridge_clone.slot_map.write()
                                        && let Some(slot) = map.remove(&id)
                                    {
                                        if let Ok(mut state) = states[slot].write() {
                                            *state = CameraEndpointState::default();
                                        }
                                        if let Ok(mut names) = bridge_clone.stream_names.write() {
                                            names.remove(&slot);
                                        }
                                        if let Some(handle) = motion_tasks.remove(&slot) {
                                            handle.abort();
                                        }
                                        log::info!("Camera {} removed from endpoint {}", id, slot + 2);
                                    }
                                }
                            }
                        }
                    }
                }
            });
        })
        .expect("Failed to spawn ONVIF/media bridge thread");

    media_bridge
}

/// Populate a camera endpoint's cluster state from ONVIF device data.
///
/// `friendly_name` overrides the auto-generated `manufacturer model` label
/// when supplied via `ONVIF_CAMERA_NAMES`.
fn populate_camera_state(
    state_lock: &Arc<RwLock<CameraEndpointState>>,
    camera: &CameraDevice,
    friendly_name: Option<&str>,
) {
    let Ok(mut state) = state_lock.write() else {
        log::error!("Failed to lock camera state for writing");
        return;
    };

    // Mark slot as occupied
    state.occupied = true;
    state.motion_supported = camera.supports_motion;
    state.motion_detected = false;

    // BDBI fields from ONVIF device info
    state.vendor_name = camera.device_info.manufacturer.clone();
    state.product_name = camera.device_info.model.clone();
    state.serial_number = camera.device_info.serial_number.clone();
    state.hardware_version_string = camera.device_info.hardware_id.clone();
    state.software_version_string = camera.device_info.firmware_version.clone();
    state.unique_id = camera.id.clone();
    state.node_label = friendly_name.map(str::to_string).unwrap_or_else(|| {
        format!(
            "{} {}",
            camera.device_info.manufacturer, camera.device_info.model
        )
    });

    // Video params from ONVIF profiles; if unavailable, advertise conservative
    // fallback defaults so manual RTSP cameras still present coherent
    // video-only capability state.
    let video_encoders: Vec<_> = camera
        .profiles
        .iter()
        .filter_map(|profile| profile.video_encoder.as_ref())
        .collect();
    if let Some(video_encoder) = video_encoders
        .iter()
        .max_by_key(|enc| enc.width as u64 * enc.height as u64 * enc.frame_rate as u64)
        .copied()
    {
        state.video_sensor_params = VideoSensorParams {
            sensor_width: video_encoder.width,
            sensor_height: video_encoder.height,
            max_hdr_fps: None,
            max_fps: video_encoder.frame_rate,
        };
        state.viewport = VideoResolution {
            width: video_encoder.width,
            height: video_encoder.height,
        };
        state.current_frame_rate = video_encoder.frame_rate;
        state.max_encoded_pixel_rate = video_encoders.iter().fold(0_u32, |acc, enc| {
            acc.saturating_add(enc.width as u32 * enc.height as u32 * enc.frame_rate as u32)
        });
        state.max_concurrent_video_encoders =
            (video_encoders.len().max(1)).min(u8::MAX as usize) as u8;
        let derived_bandwidth = video_encoders
            .iter()
            .fold(0_u32, |acc, enc| acc.saturating_add(enc.bitrate));
        state.max_network_bandwidth = if derived_bandwidth > 0 {
            derived_bandwidth
        } else {
            10_000
        };
    } else {
        apply_fallback_video_state(&mut state);
        state.max_network_bandwidth = 10_000;
    }
}

fn apply_fallback_video_state(state: &mut CameraEndpointState) {
    state.video_sensor_params = VideoSensorParams {
        sensor_width: FALLBACK_VIDEO_WIDTH,
        sensor_height: FALLBACK_VIDEO_HEIGHT,
        max_hdr_fps: None,
        max_fps: FALLBACK_VIDEO_FPS,
    };
    state.viewport = VideoResolution {
        width: FALLBACK_VIDEO_WIDTH,
        height: FALLBACK_VIDEO_HEIGHT,
    };
    state.current_frame_rate = FALLBACK_VIDEO_FPS;
    state.max_encoded_pixel_rate =
        FALLBACK_VIDEO_WIDTH as u32 * FALLBACK_VIDEO_HEIGHT as u32 * FALLBACK_VIDEO_FPS as u32;
    state.max_concurrent_video_encoders = 1;
}

/// Sanitize camera ID into a go2rtc-compatible stream name.
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

fn manual_rtsp_camera_device(camera: &ManualRtspCameraConfig) -> CameraDevice {
    let id = manual_camera_id(camera);
    let (host, port) = parse_rtsp_host_and_port(&camera.rtsp_url);

    CameraDevice {
        id: id.clone(),
        host,
        port,
        device_info: onvif_client::types::DeviceInfo {
            manufacturer: "Manual RTSP".into(),
            model: camera.name.clone(),
            firmware_version: "manual-rtsp".into(),
            serial_number: id.clone(),
            hardware_id: "manual-rtsp".into(),
        },
        profiles: Vec::new(),
        stream_uri: camera.rtsp_url.clone(),
        events_url: None,
        supports_motion: false,
    }
}

fn manual_camera_id(camera: &ManualRtspCameraConfig) -> String {
    if let Some(stable_id) = camera.stable_id.as_deref() {
        return format!("manual:{stable_id}");
    }

    format!("manual:{}", derived_manual_camera_key(camera))
}

fn derived_manual_camera_key(camera: &ManualRtspCameraConfig) -> String {
    let endpoint_hint = camera
        .rtsp_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(&camera.rtsp_url)
        .rsplit('@')
        .next()
        .unwrap_or(&camera.rtsp_url);

    format!(
        "{}-{}",
        sanitize_stream_name(&camera.name),
        sanitize_stream_name(endpoint_hint)
    )
}

fn parse_rtsp_host_and_port(rtsp_url: &str) -> (String, u16) {
    let authority = rtsp_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(rtsp_url)
        .split('/')
        .next()
        .unwrap_or(rtsp_url)
        .rsplit('@')
        .next()
        .unwrap_or(rtsp_url);

    if let Some(rest) = authority.strip_prefix('[')
        && let Some((host, remainder)) = rest.split_once(']')
    {
        let port = remainder
            .strip_prefix(':')
            .and_then(|value| value.parse().ok())
            .unwrap_or(554);
        return (host.to_string(), port);
    }

    if let Some((host, port)) = authority.rsplit_once(':')
        && let Ok(port) = port.parse::<u16>()
    {
        return (host.to_string(), port);
    }

    (authority.to_string(), 554)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use matter_camera::types::CameraEndpointState;
    use onvif_client::types::{CameraDevice, DeviceInfo, MediaProfile, VideoEncoderConfig};

    use super::{
        FALLBACK_VIDEO_FPS, FALLBACK_VIDEO_HEIGHT, FALLBACK_VIDEO_WIDTH, populate_camera_state,
    };

    fn sample_camera(profiles: Vec<MediaProfile>) -> CameraDevice {
        CameraDevice {
            id: "camera-1".into(),
            host: "192.168.1.10".into(),
            port: 554,
            device_info: DeviceInfo {
                manufacturer: "Manual RTSP".into(),
                model: "Front Door".into(),
                firmware_version: "manual-rtsp".into(),
                serial_number: "manual:front-door".into(),
                hardware_id: "manual-rtsp".into(),
            },
            profiles,
            stream_uri: "rtsp://192.168.1.10:554/stream".into(),
            events_url: None,
            supports_motion: false,
        }
    }

    #[test]
    fn populate_camera_state_uses_conservative_fallbacks_without_profiles() {
        let state = Arc::new(RwLock::new(CameraEndpointState::default()));
        let camera = sample_camera(Vec::new());

        populate_camera_state(&state, &camera, Some("Front Door"));

        let state = state.read().unwrap();
        assert!(state.occupied);
        assert_eq!(state.node_label, "Front Door");
        assert_eq!(state.video_sensor_params.sensor_width, FALLBACK_VIDEO_WIDTH);
        assert_eq!(
            state.video_sensor_params.sensor_height,
            FALLBACK_VIDEO_HEIGHT
        );
        assert_eq!(state.video_sensor_params.max_fps, FALLBACK_VIDEO_FPS);
        assert_eq!(state.viewport.width, FALLBACK_VIDEO_WIDTH);
        assert_eq!(state.viewport.height, FALLBACK_VIDEO_HEIGHT);
        assert_eq!(state.current_frame_rate, FALLBACK_VIDEO_FPS);
        assert_eq!(state.max_concurrent_video_encoders, 1);
    }

    #[test]
    fn populate_camera_state_prefers_profile_video_metadata_when_available() {
        let state = Arc::new(RwLock::new(CameraEndpointState::default()));
        let camera = sample_camera(vec![MediaProfile {
            token: "main".into(),
            name: "Main".into(),
            video_encoder: Some(VideoEncoderConfig {
                codec: "h264".into(),
                width: 1920,
                height: 1080,
                frame_rate: 20,
                bitrate: 4_000,
                quality: 5.0,
            }),
            audio_encoder: None,
            ptz_config_token: None,
        }]);

        populate_camera_state(&state, &camera, Some("Front Door"));

        let state = state.read().unwrap();
        assert_eq!(state.video_sensor_params.sensor_width, 1920);
        assert_eq!(state.video_sensor_params.sensor_height, 1080);
        assert_eq!(state.video_sensor_params.max_fps, 20);
        assert_eq!(state.viewport.width, 1920);
        assert_eq!(state.viewport.height, 1080);
        assert_eq!(state.current_frame_rate, 20);
        assert_eq!(state.max_concurrent_video_encoders, 1);
    }

    #[test]
    fn populate_camera_state_aggregates_multiple_video_encoders() {
        let state = Arc::new(RwLock::new(CameraEndpointState::default()));
        let camera = sample_camera(vec![
            MediaProfile {
                token: "sub".into(),
                name: "Sub".into(),
                video_encoder: Some(VideoEncoderConfig {
                    codec: "h264".into(),
                    width: 640,
                    height: 360,
                    frame_rate: 15,
                    bitrate: 512,
                    quality: 5.0,
                }),
                audio_encoder: None,
                ptz_config_token: None,
            },
            MediaProfile {
                token: "main".into(),
                name: "Main".into(),
                video_encoder: Some(VideoEncoderConfig {
                    codec: "h264".into(),
                    width: 1920,
                    height: 1080,
                    frame_rate: 20,
                    bitrate: 4_000,
                    quality: 5.0,
                }),
                audio_encoder: None,
                ptz_config_token: None,
            },
        ]);

        populate_camera_state(&state, &camera, Some("Front Door"));

        let state = state.read().unwrap();
        assert_eq!(state.video_sensor_params.sensor_width, 1920);
        assert_eq!(state.video_sensor_params.sensor_height, 1080);
        assert_eq!(state.current_frame_rate, 20);
        assert_eq!(state.max_concurrent_video_encoders, 2);
        assert_eq!(state.max_network_bandwidth, 4_512);
        assert!(
            state.max_encoded_pixel_rate
                >= (1920_u32 * 1080_u32 * 20_u32) + (640_u32 * 360_u32 * 15_u32)
        );
    }
}
