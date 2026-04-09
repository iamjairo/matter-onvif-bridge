//! ONVIF MotionAlarm pump.
//!
//! Spawns a per-camera tokio task that:
//! 1. Creates a PullPoint subscription filtered to `tns1:VideoSource/MotionAlarm`.
//! 2. Long-polls `PullMessages` and decodes each notification's `IsMotion` /
//!    `State` SimpleItem into a boolean.
//! 3. Renews the subscription periodically.
//! 4. On any error, drops the subscription and rebuilds it after a short delay.

use std::sync::Arc;
use std::time::Duration;

use oxvif::OnvifClient;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tracing::{debug, info, trace, warn};

/// Topic filter expression matching both motion topics VIGI cameras advertise.
/// `tns1:RuleEngine/CellMotionDetector/Motion` is what the C540V/C350 actually
/// emit; `tns1:VideoSource/MotionAlarm` is also advertised on some firmwares
/// but never fires on these models. The `||` is the ONVIF ConcreteSet OR
/// operator from the WS-BaseNotification topic dialect.
const MOTION_TOPIC: &str =
    "tns1:RuleEngine/CellMotionDetector/Motion|tns1:VideoSource/MotionAlarm";

/// Substrings (lowercased) that mark a notification as motion-related,
/// independent of the device's namespace prefix.
const MOTION_TOPIC_MARKERS: &[&str] = &["cellmotiondetector/motion", "motionalarm"];
const PULL_TIMEOUT: &str = "PT5S";
const PULL_MAX_MESSAGES: u32 = 16;
const SUBSCRIPTION_LIFETIME: &str = "PT60S";
const RENEW_INTERVAL: Duration = Duration::from_secs(45);
const RECONNECT_BACKOFF: Duration = Duration::from_secs(5);

/// Parameters needed to (re)build an authenticated `OnvifClient` and locate
/// the device's Events service.
#[derive(Clone)]
pub struct MotionPumpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub events_url: String,
    /// Human-readable label used in logs (typically `manufacturer model`).
    pub label: String,
}

/// Spawn a background task that delivers motion-alarm boolean transitions to
/// `on_change`. The task runs forever; abort the returned handle to stop it.
///
/// On every reconnect the task rebuilds the `OnvifClient` and resyncs its UTC
/// offset against the camera's clock — this is necessary because WS-Security
/// timestamps drift and stale clients eventually start failing auth.
pub fn spawn_motion_pump<F>(cfg: MotionPumpConfig, on_change: F) -> JoinHandle<()>
where
    F: Fn(bool) + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let on_change = Arc::new(on_change);
        loop {
            match build_client_and_run(&cfg, Arc::clone(&on_change)).await {
                Ok(()) => {
                    debug!(camera = %cfg.label, "motion pump exited cleanly, reconnecting");
                }
                Err(e) => {
                    warn!(camera = %cfg.label, error = %e, "motion pump error, reconnecting");
                }
            }
            tokio::time::sleep(RECONNECT_BACKOFF).await;
        }
    })
}

async fn build_client_and_run<F>(cfg: &MotionPumpConfig, on_change: Arc<F>) -> Result<(), String>
where
    F: Fn(bool) + Send + Sync + 'static,
{
    let device_url = format!("http://{}:{}/onvif/device_service", cfg.host, cfg.port);
    let client = OnvifClient::new(&device_url).with_credentials(&cfg.username, &cfg.password);
    let dt = client
        .get_system_date_and_time()
        .await
        .map_err(|e| format!("GetSystemDateAndTime failed: {e}"))?;
    let client = Arc::new(client.with_utc_offset(dt.utc_offset_secs()));
    run_subscription(&client, &cfg.events_url, &cfg.label, on_change).await
}

async fn run_subscription<F>(
    client: &OnvifClient,
    events_url: &str,
    label: &str,
    on_change: Arc<F>,
) -> Result<(), String>
where
    F: Fn(bool) + Send + Sync + 'static,
{
    let sub = client
        .create_pull_point_subscription(events_url, Some(MOTION_TOPIC), Some(SUBSCRIPTION_LIFETIME))
        .await
        .map_err(|e| format!("CreatePullPointSubscription failed: {e}"))?;

    info!(
        camera = %label,
        reference = %sub.reference_url,
        "ONVIF motion subscription created"
    );

    let sub_url = sub.reference_url.clone();
    let mut last_renew = Instant::now();

    loop {
        // Long-poll for new messages.
        let messages = client
            .pull_messages(&sub_url, PULL_TIMEOUT, PULL_MAX_MESSAGES)
            .await
            .map_err(|e| format!("PullMessages failed: {e}"))?;

        if !messages.is_empty() {
            trace!(camera = %label, count = messages.len(), "PullMessages returned");
        }
        for msg in messages {
            let topic_lc = msg.topic.to_ascii_lowercase();
            if !MOTION_TOPIC_MARKERS.iter().any(|m| topic_lc.contains(m)) {
                debug!(camera = %label, topic = %msg.topic, "ignoring non-motion topic");
                continue;
            }
            if let Some(state) = decode_motion_state(&msg.data) {
                info!(camera = %label, motion = state, "motion event");
                on_change(state);
            } else {
                warn!(
                    camera = %label,
                    data = ?msg.data,
                    "motion notification without recognised state field"
                );
            }
        }

        // Renew before the device's 60s deadline.
        if last_renew.elapsed() >= RENEW_INTERVAL {
            client
                .renew_subscription(&sub_url, SUBSCRIPTION_LIFETIME)
                .await
                .map_err(|e| format!("Renew failed: {e}"))?;
            last_renew = Instant::now();
            debug!(camera = %label, "subscription renewed");
        }
    }
}

/// Inspect a notification's `Data` SimpleItem map for the motion boolean.
/// VIGI/TP-Link cameras vary between `IsMotion` and `State`; some emit `Value`.
fn decode_motion_state(
    data: &std::collections::HashMap<String, String>,
) -> Option<bool> {
    for key in ["IsMotion", "State", "Value", "isMotion", "state"] {
        if let Some(v) = data.get(key) {
            return Some(parse_bool(v));
        }
    }
    None
}

fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1")
}
