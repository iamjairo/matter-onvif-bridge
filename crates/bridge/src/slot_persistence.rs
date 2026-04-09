//! Persistent camera-to-slot mapping.
//!
//! Without persistence the slot allocator assigns endpoints in WS-Discovery
//! response order, so a camera lands on a different Matter endpoint id every
//! time the bridge restarts. Google Home keys room assignments by endpoint
//! id, so this caused rooms to be reassigned on every restart.
//!
//! This module persists `serial → slot` to a JSON file in the Matter
//! storage directory so each camera always lands on the same endpoint.
//! Unknown cameras receive the next free slot, preferring the occupancy
//! pool when their ONVIF device advertises motion events.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// On-disk schema. Keyed by camera id (typically the ONVIF serial number).
#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedSlots {
    /// Maps camera id → slot index (0-based; endpoint id = slot + 2).
    cameras: HashMap<String, usize>,
}

/// In-memory + persistent slot allocator.
#[derive(Debug)]
pub struct SlotMap {
    path: PathBuf,
    /// camera_id → slot index
    map: HashMap<String, usize>,
    /// Total slot count (matches `MAX_CAMERAS`).
    capacity: usize,
    /// Slot count within the occupancy-supporting prefix
    /// (matches `WITH_OCCUPANCY_CAMERAS`). Slots `[0, with_occupancy)` are
    /// occupancy-capable; slots `[with_occupancy, capacity)` are camera-only.
    with_occupancy: usize,
}

impl SlotMap {
    /// Load the persisted mapping from `<matter_storage_path>.slots.json`.
    /// `matter_storage_path` is the same value passed to rs-matter's `Psm`
    /// (which uses it as a single binary file, not a directory). We sit
    /// next to it with a `.slots.json` suffix.
    ///
    /// Missing or unparseable files start empty — they'll be rewritten on
    /// the first successful allocation.
    pub fn load(matter_storage_path: &Path, capacity: usize, with_occupancy: usize) -> Self {
        let mut file_name = matter_storage_path
            .file_name()
            .map(|s| s.to_os_string())
            .unwrap_or_else(|| std::ffi::OsString::from(".matter-storage"));
        file_name.push(".slots.json");
        let path = matter_storage_path
            .parent()
            .map(|p| p.join(&file_name))
            .unwrap_or_else(|| PathBuf::from(&file_name));
        let map = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<PersistedSlots>(&s).ok())
            .map(|p| {
                // Drop entries that exceed the current capacity (e.g. after
                // shrinking the slot pool). They'll be re-allocated.
                p.cameras.into_iter().filter(|(_, slot)| *slot < capacity).collect()
            })
            .unwrap_or_default();
        Self {
            path,
            map,
            capacity,
            with_occupancy,
        }
    }

    /// Resolve a camera id to a slot index. If the camera was seen before
    /// the previously assigned slot is reused. Otherwise the next free slot
    /// is allocated, preferring the occupancy pool when `wants_occupancy`.
    /// Returns `None` if the relevant pool is exhausted.
    ///
    /// The mapping is persisted to disk on every successful new allocation.
    pub fn assign(&mut self, camera_id: &str, wants_occupancy: bool) -> Option<usize> {
        if let Some(&slot) = self.map.get(camera_id) {
            return Some(slot);
        }

        let used: std::collections::HashSet<usize> = self.map.values().copied().collect();
        let slot = if wants_occupancy {
            (0..self.with_occupancy)
                .find(|s| !used.contains(s))
                .or_else(|| (self.with_occupancy..self.capacity).find(|s| !used.contains(s)))
        } else {
            (self.with_occupancy..self.capacity)
                .find(|s| !used.contains(s))
                .or_else(|| (0..self.with_occupancy).find(|s| !used.contains(s)))
        }?;

        self.map.insert(camera_id.to_string(), slot);
        if let Err(e) = self.save() {
            log::warn!("Failed to persist camera slot map: {e}");
        }
        Some(slot)
    }

    /// Drop a camera from the map (e.g. after a permanent removal). Slot
    /// stays unused until reassigned.
    #[allow(dead_code)]
    pub fn forget(&mut self, camera_id: &str) {
        if self.map.remove(camera_id).is_some() {
            let _ = self.save();
        }
    }

    fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            // Empty parent means CWD — skip create_dir_all in that case.
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let data = PersistedSlots {
            cameras: self.map.clone(),
        };
        let json = serde_json::to_string_pretty(&data).map_err(std::io::Error::other)?;
        std::fs::write(&self.path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn assigns_then_reuses_slot() {
        let dir = tempdir().unwrap();
        let storage = dir.path().join(".matter-storage");
        let mut m = SlotMap::load(&storage, 8, 7);
        let s1 = m.assign("cam-a", true).unwrap();
        let s2 = m.assign("cam-b", true).unwrap();
        assert_ne!(s1, s2);

        // Reload from disk and confirm the same slots come back.
        let m2 = SlotMap::load(&storage, 8, 7);
        assert_eq!(m2.map.get("cam-a"), Some(&s1));
        assert_eq!(m2.map.get("cam-b"), Some(&s2));
    }

    #[test]
    fn occupancy_preference() {
        let dir = tempdir().unwrap();
        let storage = dir.path().join(".matter-storage");
        let mut m = SlotMap::load(&storage, 8, 2);
        // First two with occupancy fill 0..2
        assert!(m.assign("a", true).unwrap() < 2);
        assert!(m.assign("b", true).unwrap() < 2);
        // Third with occupancy spills into plain pool 2..8
        let spill = m.assign("c", true).unwrap();
        assert!((2..8).contains(&spill));
        // A camera that doesn't want occupancy goes straight to plain pool
        let plain = m.assign("d", false).unwrap();
        assert!((2..8).contains(&plain));
    }
}
