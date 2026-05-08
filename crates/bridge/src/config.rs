use std::collections::HashMap;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub onvif: OnvifConfig,
    pub media: MediaConfig,
    pub matter: MatterConfig,
    pub log_level: String,
}

/// Which media server handles RTSP→WebRTC bridging.
#[derive(Debug, Clone)]
pub enum MediaConfig {
    Go2Rtc(Go2RtcConfig),
    MediaMtx(MediaMtxConfig),
}

#[derive(Debug, Clone)]
pub struct OnvifConfig {
    pub username: String,
    pub password: String,
    pub discovery_interval_ms: u64,
    pub discovery_mode: DiscoveryMode,
    pub static_cameras: Vec<String>,
    /// Optional friendly-name overrides. Keys may be either a camera serial
    /// number (preferred — survives IP changes) or a host/IP fallback.
    /// Sourced from `ONVIF_CAMERA_NAMES="key=Name,key=Name,..."`.
    pub camera_names: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum DiscoveryMode {
    Auto,
    Static,
}

#[derive(Debug, Clone)]
pub struct Go2RtcConfig {
    pub mode: Go2RtcMode,
    pub path: String,
    pub host: String,
    pub api_port: u16,
    pub webrtc_port: u16,
}

#[derive(Debug, Clone)]
pub enum Go2RtcMode {
    Local,
    External,
}

#[derive(Debug, Clone)]
pub struct MediaMtxConfig {
    pub mode: MediaMtxMode,
    pub path: String,
    pub host: String,
    pub api_port: u16,
    pub whep_port: u16,
}

#[derive(Debug, Clone)]
pub enum MediaMtxMode {
    Local,
    External,
}

#[derive(Debug, Clone)]
pub struct MatterConfig {
    pub port: u16,
    pub passcode: u32,
    pub discriminator: u16,
    pub storage_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            onvif: OnvifConfig {
                username: "admin".into(),
                password: "admin".into(),
                discovery_interval_ms: 60_000,
                discovery_mode: DiscoveryMode::Auto,
                static_cameras: Vec::new(),
                camera_names: HashMap::new(),
            },
            media: MediaConfig::Go2Rtc(Go2RtcConfig {
                mode: Go2RtcMode::External,
                path: "./bin/go2rtc".into(),
                host: "localhost".into(),
                api_port: 1984,
                webrtc_port: 8555,
            }),
            matter: MatterConfig {
                port: 5540,
                passcode: 20202021,
                discriminator: 3840,
                storage_path: ".matter-storage".into(),
            },
            log_level: "info".into(),
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let defaults = Self::default();

        let static_cameras_str = env::var("ONVIF_STATIC_CAMERAS").unwrap_or_default();
        let static_cameras: Vec<String> = if static_cameras_str.is_empty() {
            Vec::new()
        } else {
            static_cameras_str.split(',').map(|s| s.trim().to_string()).collect()
        };

        // ONVIF_CAMERA_NAMES="192.168.86.5=Front Door,192.168.86.2=Backyard"
        let camera_names: HashMap<String, String> = env::var("ONVIF_CAMERA_NAMES")
            .unwrap_or_default()
            .split(',')
            .filter_map(|entry| {
                let entry = entry.trim();
                if entry.is_empty() {
                    return None;
                }
                let (host, name) = entry.split_once('=')?;
                Some((host.trim().to_string(), name.trim().to_string()))
            })
            .collect();

        let discovery_mode = match env::var("ONVIF_DISCOVERY_MODE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "static" => DiscoveryMode::Static,
            _ => DiscoveryMode::Auto,
        };

        // Determine which media server to use
        let media_server_str = env::var("MEDIA_SERVER")
            .unwrap_or_default()
            .to_lowercase();

        let media = if media_server_str == "mediamtx" {
            let default_mtx = MediaMtxConfig {
                mode: MediaMtxMode::External,
                path: "./bin/mediamtx".into(),
                host: "localhost".into(),
                api_port: 9997,
                whep_port: 8889,
            };
            let mode = match env::var("MEDIAMTX_MODE")
                .unwrap_or_default()
                .to_lowercase()
                .as_str()
            {
                "local" => MediaMtxMode::Local,
                _ => MediaMtxMode::External,
            };
            MediaConfig::MediaMtx(MediaMtxConfig {
                mode,
                path: env::var("MEDIAMTX_PATH").unwrap_or(default_mtx.path),
                host: env::var("MEDIAMTX_HOST").unwrap_or(default_mtx.host),
                api_port: env::var("MEDIAMTX_API_PORT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(default_mtx.api_port),
                whep_port: env::var("MEDIAMTX_WHEP_PORT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(default_mtx.whep_port),
            })
        } else {
            // Default: go2rtc
            let default_go2rtc = match defaults.media {
                MediaConfig::Go2Rtc(ref c) => c.clone(),
                MediaConfig::MediaMtx(_) => unreachable!(),
            };
            let go2rtc_mode = match env::var("GO2RTC_MODE")
                .unwrap_or_default()
                .to_lowercase()
                .as_str()
            {
                "local" => Go2RtcMode::Local,
                _ => Go2RtcMode::External,
            };
            MediaConfig::Go2Rtc(Go2RtcConfig {
                mode: go2rtc_mode,
                path: env::var("GO2RTC_PATH").unwrap_or(default_go2rtc.path),
                host: env::var("GO2RTC_HOST").unwrap_or(default_go2rtc.host),
                api_port: env::var("GO2RTC_API_PORT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(default_go2rtc.api_port),
                webrtc_port: env::var("GO2RTC_WEBRTC_PORT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(default_go2rtc.webrtc_port),
            })
        };

        Self {
            onvif: OnvifConfig {
                username: env::var("ONVIF_USERNAME").unwrap_or(defaults.onvif.username),
                password: env::var("ONVIF_PASSWORD").unwrap_or(defaults.onvif.password),
                discovery_interval_ms: env::var("ONVIF_DISCOVERY_INTERVAL")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(defaults.onvif.discovery_interval_ms),
                discovery_mode,
                static_cameras,
                camera_names,
            },
            media,
            matter: MatterConfig {
                port: env::var("MATTER_PORT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(defaults.matter.port),
                passcode: env::var("MATTER_PASSCODE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(defaults.matter.passcode),
                discriminator: env::var("MATTER_DISCRIMINATOR")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(defaults.matter.discriminator),
                storage_path: env::var("MATTER_STORAGE_PATH")
                    .unwrap_or(defaults.matter.storage_path),
            },
            log_level: env::var("LOG_LEVEL").unwrap_or(defaults.log_level),
        }
    }
}
