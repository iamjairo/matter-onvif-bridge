//! OccupancySensing cluster (0x0406) — Matter 1.4 §2.7
//!
//! Backed by ONVIF MotionAlarm events. The cluster is only added to a camera
//! endpoint when the underlying ONVIF device advertises a MotionAlarm topic
//! (see slot pool split in `bridge::main`).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::RwLock;

use rs_matter::attributes;
use rs_matter::commands;
use rs_matter::dm::{
    Access, Attribute, Cluster, Handler, InvokeContext, InvokeReply, NonBlockingHandler,
    Quality, ReadContext, ReadReply, Reply, WriteContext,
};
use rs_matter::error::{Error, ErrorCode};
use strum::FromRepr;

use crate::types::CameraEndpointState;

pub const CLUSTER_ID: u32 = 0x0406;
const CLUSTER_REVISION: u16 = 5;

/// Matter 1.4 OccupancySensing attribute IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
#[repr(u32)]
pub enum Attributes {
    Occupancy = 0x0000,
    OccupancySensorType = 0x0001,
    OccupancySensorTypeBitmap = 0x0002,
}

rs_matter::attribute_enum!(Attributes);

// OccupancySensing has no commands — no Commands enum needed.

/// Sensor type per Matter 1.4 §2.7.5.2: 0 = PIR, 1 = Ultrasonic, 2 = PIR+Ultrasonic, 3 = PhysicalContact.
/// Cameras are best modelled as PIR for controller compatibility.
const SENSOR_TYPE_PIR: u8 = 0;
/// Bitmap §2.7.5.3 — bit 0 = PIR.
const SENSOR_TYPE_BITMAP_PIR: u8 = 0b0000_0001;

/// FeatureMap bit 0 = PIR. Required by Google Home for the cluster to be
/// surfaced as a motion sensor entity.
const FEATURE_MAP_PIR: u32 = 0b0000_0001;

pub const OCCUPANCY_CLUSTER: Cluster<'static> = Cluster::new(
    CLUSTER_ID,
    CLUSTER_REVISION,
    FEATURE_MAP_PIR,
    attributes!(
        Attribute::new(Attributes::Occupancy as _, Access::RV, Quality::NONE),
        Attribute::new(Attributes::OccupancySensorType as _, Access::RV, Quality::FIXED),
        Attribute::new(
            Attributes::OccupancySensorTypeBitmap as _,
            Access::RV,
            Quality::FIXED
        )
    ),
    commands!(),
    |_, _, _| true,
    |_, _, _| true,
);

/// Cluster handler that reads `motion_detected` from the shared camera state.
///
/// rs-matter's `Dataver` is intentionally single-threaded (it wraps a `Cell`
/// behind a `NoopRawMutex`), so we can't share it with the tokio bridge
/// thread that pumps ONVIF events. Instead the dataver counter is an
/// `AtomicU32` cloned via `Arc` — the bridge thread calls
/// [`OccupancyDataver::bump`] when a new event arrives.
#[derive(Clone, Default)]
pub struct OccupancyDataver(Arc<AtomicU32>);

impl OccupancyDataver {
    pub fn new(initial: u32) -> Self {
        Self(Arc::new(AtomicU32::new(initial)))
    }

    pub fn get(&self) -> u32 {
        self.0.load(Ordering::Acquire)
    }

    /// Increment and return the new value. Safe to call from any thread.
    pub fn bump(&self) -> u32 {
        // wrapping add via fetch_add (u32 wraps naturally)
        self.0.fetch_add(1, Ordering::AcqRel).wrapping_add(1)
    }
}

pub struct OccupancyHandler {
    dataver: OccupancyDataver,
    state: Arc<RwLock<CameraEndpointState>>,
}

impl OccupancyHandler {
    pub fn new(dataver: OccupancyDataver, state: Arc<RwLock<CameraEndpointState>>) -> Self {
        Self { dataver, state }
    }

    /// Clone of the dataver handle — handed to the motion pump so it can bump
    /// the version when a new event arrives.
    pub fn dataver_handle(&self) -> OccupancyDataver {
        self.dataver.clone()
    }
}

impl Handler for OccupancyHandler {
    fn read(&self, ctx: impl ReadContext, reply: impl ReadReply) -> Result<(), Error> {
        let attr = ctx.attr();

        let dv = self.dataver.get();
        if let Some(writer) = reply.with_dataver(dv)? {
            if attr.is_system() {
                return OCCUPANCY_CLUSTER.read(attr, writer);
            }

            let state = self.state.read().map_err(|_| ErrorCode::Busy)?;

            match attr.attr_id.try_into()? {
                Attributes::Occupancy => {
                    // Occupancy is a bitmap8, bit 0 = occupied
                    let bits: u8 = if state.motion_detected { 0b0000_0001 } else { 0 };
                    writer.set(bits)
                }
                Attributes::OccupancySensorType => writer.set(SENSOR_TYPE_PIR),
                Attributes::OccupancySensorTypeBitmap => writer.set(SENSOR_TYPE_BITMAP_PIR),
            }
        } else {
            Ok(())
        }
    }

    fn write(&self, _ctx: impl WriteContext) -> Result<(), Error> {
        Err(ErrorCode::AttributeNotFound.into())
    }

    fn invoke(&self, _ctx: impl InvokeContext, _reply: impl InvokeReply) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }
}

impl NonBlockingHandler for OccupancyHandler {}
