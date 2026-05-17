//! Custom Matter cluster implementations for camera devices.
//!
//! Implements the camera-specific clusters not yet available in rs-matter:
//! - CameraAvStreamManagement (0x0551)
//! - ZoneManagement (0x0550)
//! - WebRtcTransportProvider (0x0553)
//! - WebRtcTransportRequestor (0x0554)
//! - PushAvStreamTransport (0x0555)
//! - Chime (0x0556)

pub mod cluster_av_stream;
pub mod cluster_chime;
pub mod cluster_occupancy;
pub mod cluster_push_av;
pub mod cluster_webrtc;
pub mod cluster_webrtc_requestor;
pub mod cluster_zone_mgmt;
pub mod types;

pub use cluster_av_stream::{AV_STREAM_CLUSTER, AvStreamHandler};
pub use cluster_chime::{CHIME_CLUSTER, ChimeHandler};
pub use cluster_occupancy::{OCCUPANCY_CLUSTER, OccupancyDataver, OccupancyHandler};
pub use cluster_push_av::{PUSH_AV_CLUSTER, PushAvHandler};
pub use cluster_webrtc::{WEBRTC_CLUSTER, WebRtcHandler};
pub use cluster_webrtc_requestor::{WEBRTC_REQUESTOR_CLUSTER, WebRtcRequestorHandler};
pub use cluster_zone_mgmt::{ZONE_MGMT_CLUSTER, ZoneManagementHandler};
pub use types::CameraEndpointState;

use rs_matter::error::{Error, ErrorCode};

/// Parse a Matter StreamUsageEnum value into our `StreamUsage` type.
pub(crate) fn parse_stream_usage(v: u8) -> types::StreamUsage {
    match v {
        0 => types::StreamUsage::Internal,
        1 => types::StreamUsage::Recording,
        2 => types::StreamUsage::Analysis,
        _ => types::StreamUsage::LiveView,
    }
}

/// Parse a TLV element as a nullable u16 (returns `None` for null/empty).
pub(crate) fn parse_nullable_u16(
    elem: rs_matter::tlv::TLVElement<'_>,
) -> Result<Option<u16>, Error> {
    if elem.is_empty() || elem.null().is_ok() {
        return Ok(None);
    }
    elem.u16()
        .map(Some)
        .map_err(|_| ErrorCode::InvalidCommand.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::StreamUsage;

    #[test]
    fn test_parse_stream_usage_known_values() {
        assert_eq!(parse_stream_usage(0), StreamUsage::Internal);
        assert_eq!(parse_stream_usage(1), StreamUsage::Recording);
        assert_eq!(parse_stream_usage(2), StreamUsage::Analysis);
        assert_eq!(parse_stream_usage(3), StreamUsage::LiveView);
    }

    #[test]
    fn test_parse_stream_usage_unknown_defaults() {
        // Any value outside the 0-3 range should default to LiveView.
        assert_eq!(parse_stream_usage(4), StreamUsage::LiveView);
        assert_eq!(parse_stream_usage(99), StreamUsage::LiveView);
        assert_eq!(parse_stream_usage(255), StreamUsage::LiveView);
    }
}
