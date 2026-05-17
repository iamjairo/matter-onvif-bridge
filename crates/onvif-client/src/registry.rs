//! Camera registry — central store of discovered cameras with broadcast events.
//!
//! Mirrors the TypeScript CameraRegistry with event-based add/remove notifications.

use std::collections::{HashMap, hash_map::Entry};
use std::sync::{Arc, RwLock};

use tokio::sync::broadcast;
use tracing::{debug, info};

use crate::types::CameraDevice;

/// Events emitted by the camera registry.
#[derive(Debug, Clone)]
pub enum RegistryEvent {
    Added(CameraDevice),
    Removed(String),
    Updated(CameraDevice),
}

/// Central store of discovered ONVIF cameras.
///
/// Thread-safe via Arc<RwLock<>> for state and broadcast channels for events.
#[derive(Clone)]
pub struct CameraRegistry {
    cameras: Arc<RwLock<HashMap<String, CameraDevice>>>,
    tx: broadcast::Sender<RegistryEvent>,
}

impl CameraRegistry {
    /// Create a new camera registry with the given broadcast channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            cameras: Arc::new(RwLock::new(HashMap::new())),
            tx,
        }
    }

    /// Subscribe to registry events.
    pub fn subscribe(&self) -> broadcast::Receiver<RegistryEvent> {
        self.tx.subscribe()
    }

    /// Add or update a camera in the registry.
    pub fn add_camera(&self, camera: CameraDevice) -> Result<(), String> {
        let mut cameras = self
            .cameras
            .write()
            .map_err(|e| format!("registry lock poisoned: {e}"))?;
        match cameras.entry(camera.id.clone()) {
            Entry::Occupied(mut entry) => {
                info!(camera_id = entry.key(), "Camera updated in registry");
                entry.insert(camera.clone());
                let _ = self.tx.send(RegistryEvent::Updated(camera));
            }
            Entry::Vacant(entry) => {
                info!(
                    camera_id = entry.key(),
                    manufacturer = camera.device_info.manufacturer,
                    model = camera.device_info.model,
                    "Camera added to registry"
                );
                entry.insert(camera.clone());
                let _ = self.tx.send(RegistryEvent::Added(camera));
            }
        }
        Ok(())
    }

    /// Remove a camera from the registry by ID.
    pub fn remove_camera(&self, id: &str) -> Result<(), String> {
        let mut cameras = self
            .cameras
            .write()
            .map_err(|e| format!("registry lock poisoned: {e}"))?;
        if cameras.remove(id).is_some() {
            info!(camera_id = id, "Camera removed from registry");
            let _ = self.tx.send(RegistryEvent::Removed(id.to_string()));
        } else {
            debug!(camera_id = id, "Attempted to remove unknown camera");
        }
        Ok(())
    }

    /// Get a camera by ID.
    pub fn get(&self, id: &str) -> Option<CameraDevice> {
        let cameras = self.cameras.read().ok()?;
        cameras.get(id).cloned()
    }

    /// Get all cameras.
    pub fn get_all(&self) -> Vec<CameraDevice> {
        let cameras = self
            .cameras
            .read()
            .unwrap_or_else(|e| e.into_inner());
        cameras.values().cloned().collect()
    }

    /// Get the number of cameras in the registry.
    pub fn len(&self) -> usize {
        self.cameras.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CameraDevice, DeviceInfo};

    fn test_camera(id: &str) -> CameraDevice {
        CameraDevice {
            id: id.to_string(),
            host: "192.168.1.100".to_string(),
            port: 80,
            device_info: DeviceInfo {
                manufacturer: "TestCorp".to_string(),
                model: "TestCam".to_string(),
                firmware_version: "1.0".to_string(),
                serial_number: id.to_string(),
                hardware_id: "hw-1".to_string(),
            },
            profiles: vec![],
            stream_uri: format!("rtsp://192.168.1.100/stream/{id}"),
            events_url: None,
            supports_motion: false,
        }
    }

    #[test]
    fn test_add_and_get_camera() {
        let registry = CameraRegistry::new(16);
        let cam = test_camera("cam-1");
        registry.add_camera(cam.clone()).unwrap();

        let retrieved = registry.get("cam-1").expect("camera should be present");
        assert_eq!(retrieved.id, "cam-1");
        assert_eq!(retrieved.host, cam.host);
    }

    #[test]
    fn test_remove_camera() {
        let registry = CameraRegistry::new(16);
        registry.add_camera(test_camera("cam-1")).unwrap();
        assert!(registry.get("cam-1").is_some());

        registry.remove_camera("cam-1").unwrap();
        assert!(registry.get("cam-1").is_none());
    }

    #[test]
    fn test_add_camera_emits_event() {
        let registry = CameraRegistry::new(16);
        let mut rx = registry.subscribe();

        registry.add_camera(test_camera("cam-1")).unwrap();

        let event = rx.try_recv().expect("should receive an event");
        match event {
            RegistryEvent::Added(cam) => assert_eq!(cam.id, "cam-1"),
            other => panic!("expected Added event, got {other:?}"),
        }
    }

    #[test]
    fn test_remove_nonexistent_is_ok() {
        let registry = CameraRegistry::new(16);
        // Removing a camera that was never added should succeed without error.
        assert!(registry.remove_camera("no-such-camera").is_ok());
    }

    #[test]
    fn test_len_and_is_empty() {
        let registry = CameraRegistry::new(16);
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.add_camera(test_camera("cam-1")).unwrap();
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);

        registry.add_camera(test_camera("cam-2")).unwrap();
        assert_eq!(registry.len(), 2);

        registry.remove_camera("cam-1").unwrap();
        assert_eq!(registry.len(), 1);
    }
}
