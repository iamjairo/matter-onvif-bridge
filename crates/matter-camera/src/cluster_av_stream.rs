//! CameraAvStreamManagement cluster (0x0551) — Matter 1.5 §4.20
//!
//! Manages audio/video/snapshot stream lifecycle for Matter camera devices.

use std::sync::{Arc, RwLock};

use rs_matter::attributes;
use rs_matter::commands;
use rs_matter::dm::{
    Access, Attribute, Cluster, Command, Dataver, Handler, InvokeContext, InvokeReply,
    NonBlockingHandler, Quality, ReadContext, ReadReply, Reply, WriteContext,
};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::tlv::{TLVTag, TLVWrite};
use strum::FromRepr;
use tracing::debug;

use crate::types::CameraEndpointState;

pub const CLUSTER_ID: u32 = 0x0551;
const CLUSTER_REVISION: u16 = 1;
const IMAGE_CODEC_JPEG: u8 = 0;

/// Read-Write with View for read, Operate for write.
/// rs-matter doesn't define RWVO, so we compose it.
const RWVO: Access = Access::from_bits_truncate(
    Access::READ.bits()
        | Access::WRITE.bits()
        | Access::NEED_VIEW.bits()
        | Access::NEED_OPERATE.bits(),
);

// ── Attribute enum ──

/// Attribute IDs per Matter 1.5.0.1 spec (controller-clusters.matter)
#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
#[repr(u32)]
pub enum Attributes {
    MaxConcurrentEncoders = 0,
    MaxEncodedPixelRate = 1,
    VideoSensorParams = 2,
    NightVisionUsesInfrared = 3,
    MinViewportResolution = 4,
    RateDistortionTradeOffPoints = 5,
    MaxContentBufferSize = 6,
    MicrophoneCapabilities = 7,
    SpeakerCapabilities = 8,
    TwoWayTalkSupport = 9,
    SnapshotCapabilities = 10,
    MaxNetworkBandwidth = 11,
    CurrentFrameRate = 12,
    HdrModeEnabled = 13,
    SupportedStreamUsages = 14,
    AllocatedVideoStreams = 15,
    AllocatedAudioStreams = 16,
    AllocatedSnapshotStreams = 17,
    StreamUsagePriorities = 18,
    SoftRecordingPrivacyModeEnabled = 19,
    SoftLivestreamPrivacyModeEnabled = 20,
    HardPrivacyModeOn = 21,
    NightVision = 22,
    NightVisionIllum = 23,
    Viewport = 24,
    SpeakerMuted = 25,
    SpeakerVolumeLevel = 26,
    SpeakerMaxLevel = 27,
    SpeakerMinLevel = 28,
    MicrophoneMuted = 29,
    MicrophoneVolumeLevel = 30,
    MicrophoneMaxLevel = 31,
    MicrophoneMinLevel = 32,
    MicrophoneAgcEnabled = 33,
    ImageRotation = 34,
    ImageFlipHorizontal = 35,
    ImageFlipVertical = 36,
    LocalVideoRecordingEnabled = 37,
    LocalSnapshotRecordingEnabled = 38,
    StatusLightEnabled = 39,
    StatusLightBrightness = 40,
}

rs_matter::attribute_enum!(Attributes);

// ── Command enum ──

#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
#[repr(u32)]
pub enum Commands {
    AudioStreamAllocate = 0x0000,
    AudioStreamAllocateResponse = 0x0001,
    AudioStreamDeallocate = 0x0002,
    VideoStreamAllocate = 0x0003,
    VideoStreamAllocateResponse = 0x0004,
    VideoStreamModify = 0x0005,
    VideoStreamDeallocate = 0x0006,
    SnapshotStreamAllocate = 0x0007,
    SnapshotStreamAllocateResponse = 0x0008,
    SnapshotStreamModify = 0x0009,
    SnapshotStreamDeallocate = 0x000A,
    SetStreamPriorities = 0x000B,
    CaptureSnapshot = 0x000C,
    CaptureSnapshotResponse = 0x000D,
}

rs_matter::command_enum!(Commands);

// ── Cluster metadata ──

pub const AV_STREAM_CLUSTER: Cluster<'static> = Cluster::new(
    CLUSTER_ID,
    CLUSTER_REVISION,
    0, // feature_map
    // Attributes per Matter 1.5.0.1 spec
    attributes!(
        Attribute::new(
            Attributes::MaxConcurrentEncoders as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::MaxEncodedPixelRate as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::VideoSensorParams as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::NightVisionUsesInfrared as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::MinViewportResolution as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::RateDistortionTradeOffPoints as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::MaxContentBufferSize as _,
            Access::RV,
            Quality::NONE
        ),
        Attribute::new(
            Attributes::MicrophoneCapabilities as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::SpeakerCapabilities as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::TwoWayTalkSupport as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::SnapshotCapabilities as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::MaxNetworkBandwidth as _,
            Access::RV,
            Quality::NONE
        ),
        Attribute::new(
            Attributes::CurrentFrameRate as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::HdrModeEnabled as _,
            Access::RWVM,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::SupportedStreamUsages as _,
            Access::RV,
            Quality::NONE
        ),
        Attribute::new(
            Attributes::AllocatedVideoStreams as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::AllocatedAudioStreams as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::AllocatedSnapshotStreams as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::StreamUsagePriorities as _,
            Access::RV,
            Quality::NONE
        ),
        Attribute::new(
            Attributes::SoftRecordingPrivacyModeEnabled as _,
            RWVO,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::SoftLivestreamPrivacyModeEnabled as _,
            RWVO,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::HardPrivacyModeOn as _,
            Access::RV,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::NightVision as _,
            Access::RWVM,
            Quality::OPTIONAL
        ),
        Attribute::new(
            Attributes::NightVisionIllum as _,
            Access::RWVM,
            Quality::OPTIONAL
        ),
        Attribute::new(Attributes::Viewport as _, Access::RWVM, Quality::OPTIONAL)
    ),
    commands!(
        // Accepted commands (requests)
        Command::new(
            Commands::AudioStreamAllocate as _,
            Some(Commands::AudioStreamAllocateResponse as _),
            Access::WO
        ),
        Command::new(Commands::AudioStreamDeallocate as _, None, Access::WO),
        Command::new(
            Commands::VideoStreamAllocate as _,
            Some(Commands::VideoStreamAllocateResponse as _),
            Access::WO
        ),
        Command::new(Commands::VideoStreamModify as _, None, Access::WO),
        Command::new(Commands::VideoStreamDeallocate as _, None, Access::WO),
        Command::new(
            Commands::SnapshotStreamAllocate as _,
            Some(Commands::SnapshotStreamAllocateResponse as _),
            Access::WO
        ),
        Command::new(Commands::SnapshotStreamModify as _, None, Access::WO),
        Command::new(Commands::SnapshotStreamDeallocate as _, None, Access::WO),
        Command::new(Commands::SetStreamPriorities as _, None, Access::WO),
        Command::new(
            Commands::CaptureSnapshot as _,
            Some(Commands::CaptureSnapshotResponse as _),
            Access::WO
        )
    ),
    &[],            // events: none
    |_, _, _| true, // with_attrs
    |_, _, _| true, // with_cmds
    |_, _, _| true, // with_events
);

// ── Handler ──

pub struct AvStreamHandler {
    dataver: Dataver,
    state: Arc<RwLock<CameraEndpointState>>,
    snapshot_capture: Option<SnapshotCaptureFn>,
}

#[derive(Debug, Clone)]
pub struct SnapshotCaptureRequest {
    pub snapshot_stream_id: Option<u16>,
    pub requested_resolution: crate::types::VideoResolution,
}

#[derive(Debug, Clone)]
pub struct SnapshotCaptureResult {
    pub jpeg_bytes: Vec<u8>,
    pub resolution: crate::types::VideoResolution,
}

pub type SnapshotCaptureFn =
    Box<dyn Fn(SnapshotCaptureRequest) -> Result<SnapshotCaptureResult, String> + Send + Sync>;

impl AvStreamHandler {
    pub fn new(dataver: Dataver, state: Arc<RwLock<CameraEndpointState>>) -> Self {
        Self {
            dataver,
            state,
            snapshot_capture: None,
        }
    }

    /// Set the snapshot capture callback.
    pub fn set_snapshot_capture(&mut self, f: SnapshotCaptureFn) {
        self.snapshot_capture = Some(f);
    }
}

impl Handler for AvStreamHandler {
    fn read(&self, ctx: impl ReadContext, reply: impl ReadReply) -> Result<(), Error> {
        let attr = ctx.attr();

        if let Some(mut writer) = reply.with_dataver(self.dataver.get())? {
            let state = self.state.read().map_err(|_| ErrorCode::Busy)?;

            if attr.is_system() {
                return AV_STREAM_CLUSTER.read(attr, writer);
            }

            match attr.attr_id.try_into()? {
                Attributes::MaxConcurrentEncoders => {
                    writer.set(state.max_concurrent_video_encoders)
                }
                Attributes::MaxEncodedPixelRate => writer.set(state.max_encoded_pixel_rate),
                Attributes::VideoSensorParams => {
                    let tag = writer.tag().clone();
                    let mut tw = writer.writer();
                    tw.start_struct(&tag)?;
                    tw.u16(&TLVTag::Context(0), state.video_sensor_params.sensor_width)?;
                    tw.u16(&TLVTag::Context(1), state.video_sensor_params.sensor_height)?;
                    tw.u16(&TLVTag::Context(2), state.video_sensor_params.max_fps)?;
                    tw.end_container()?;
                    drop(tw);
                    writer.complete()
                }
                Attributes::NightVisionUsesInfrared => writer.set(false),
                Attributes::MinViewportResolution => {
                    write_video_resolution(writer, &state.min_viewport)
                }
                Attributes::RateDistortionTradeOffPoints => write_empty_array(writer),
                Attributes::MaxContentBufferSize => writer.set(0_u32),
                Attributes::MicrophoneCapabilities => write_empty_array(writer),
                Attributes::SpeakerCapabilities => write_empty_array(writer),
                Attributes::TwoWayTalkSupport => writer.set(state.two_way_talk_support as u8),
                Attributes::SnapshotCapabilities => write_snapshot_capabilities(writer, &state),
                Attributes::MaxNetworkBandwidth => writer.set(state.max_network_bandwidth),
                Attributes::CurrentFrameRate => writer.set(state.current_frame_rate),
                Attributes::HdrModeEnabled => writer.set(false),
                Attributes::SupportedStreamUsages => write_supported_stream_usages(writer),
                Attributes::AllocatedVideoStreams => {
                    let tag = writer.tag().clone();
                    let mut tw = writer.writer();
                    tw.start_array(&tag)?;
                    for vs in &state.allocated_video_streams {
                        tw.start_struct(&TLVTag::Anonymous)?;
                        tw.u16(&TLVTag::Context(0), vs.video_stream_id)?;
                        tw.u8(&TLVTag::Context(1), vs.stream_usage as u8)?;
                        tw.u8(&TLVTag::Context(2), vs.video_codec as u8)?;
                        tw.u16(&TLVTag::Context(3), vs.min_frame_rate)?;
                        tw.u16(&TLVTag::Context(4), vs.max_frame_rate)?;
                        tw.start_struct(&TLVTag::Context(5))?; // minResolution
                        tw.u16(&TLVTag::Context(0), vs.min_resolution.width)?;
                        tw.u16(&TLVTag::Context(1), vs.min_resolution.height)?;
                        tw.end_container()?;
                        tw.start_struct(&TLVTag::Context(6))?; // maxResolution
                        tw.u16(&TLVTag::Context(0), vs.max_resolution.width)?;
                        tw.u16(&TLVTag::Context(1), vs.max_resolution.height)?;
                        tw.end_container()?;
                        tw.u32(&TLVTag::Context(7), vs.min_bit_rate)?;
                        tw.u32(&TLVTag::Context(8), vs.max_bit_rate)?;
                        tw.u16(&TLVTag::Context(9), vs.key_frame_interval)?;
                        tw.u8(&TLVTag::Context(12), vs.reference_count.unwrap_or(0))?;
                        tw.end_container()?;
                    }
                    tw.end_container()?;
                    drop(tw);
                    writer.complete()
                }
                Attributes::AllocatedAudioStreams => {
                    let tag = writer.tag().clone();
                    let mut tw = writer.writer();
                    tw.start_array(&tag)?;
                    for aus in &state.allocated_audio_streams {
                        tw.start_struct(&TLVTag::Anonymous)?;
                        tw.u16(&TLVTag::Context(0), aus.audio_stream_id)?;
                        tw.u8(&TLVTag::Context(1), aus.stream_usage as u8)?;
                        tw.u8(&TLVTag::Context(2), aus.audio_codec as u8)?;
                        tw.u8(&TLVTag::Context(3), aus.channel_count)?;
                        tw.u32(&TLVTag::Context(4), aus.sample_rate)?;
                        tw.u32(&TLVTag::Context(5), aus.bit_rate)?;
                        tw.u8(&TLVTag::Context(6), aus.bit_depth)?;
                        tw.u8(&TLVTag::Context(7), aus.reference_count.unwrap_or(0))?;
                        tw.end_container()?;
                    }
                    tw.end_container()?;
                    drop(tw);
                    writer.complete()
                }
                Attributes::AllocatedSnapshotStreams => {
                    let tag = writer.tag().clone();
                    let mut tw = writer.writer();
                    tw.start_array(&tag)?;
                    for ss in &state.allocated_snapshot_streams {
                        tw.start_struct(&TLVTag::Anonymous)?;
                        tw.u16(&TLVTag::Context(0), ss.snapshot_stream_id)?;
                        tw.u8(&TLVTag::Context(1), ss.image_codec as u8)?;
                        tw.u16(&TLVTag::Context(2), ss.frame_rate)?;
                        tw.start_struct(&TLVTag::Context(3))?;
                        tw.u16(&TLVTag::Context(0), ss.min_resolution.width)?;
                        tw.u16(&TLVTag::Context(1), ss.min_resolution.height)?;
                        tw.end_container()?;
                        tw.start_struct(&TLVTag::Context(4))?;
                        tw.u16(&TLVTag::Context(0), ss.max_resolution.width)?;
                        tw.u16(&TLVTag::Context(1), ss.max_resolution.height)?;
                        tw.end_container()?;
                        tw.u8(&TLVTag::Context(5), ss.quality)?;
                        tw.u8(&TLVTag::Context(6), ss.reference_count.unwrap_or(0))?;
                        tw.end_container()?;
                    }
                    tw.end_container()?;
                    drop(tw);
                    writer.complete()
                }
                Attributes::StreamUsagePriorities => write_stream_usage_priorities(writer),
                Attributes::SoftRecordingPrivacyModeEnabled => {
                    writer.set(state.soft_recording_privacy_mode_enabled)
                }
                Attributes::SoftLivestreamPrivacyModeEnabled => {
                    writer.set(state.soft_livestream_privacy_mode_enabled)
                }
                Attributes::HardPrivacyModeOn => writer.set(state.hard_privacy_mode_on),
                Attributes::NightVision => writer.set(state.night_vision as u8),
                Attributes::NightVisionIllum => writer.set(state.night_vision_illum as u8),
                Attributes::Viewport => write_video_resolution(writer, &state.viewport),
                // Optional attributes we don't implement yet — return default values
                Attributes::SpeakerMuted
                | Attributes::MicrophoneMuted
                | Attributes::MicrophoneAgcEnabled
                | Attributes::ImageFlipHorizontal
                | Attributes::ImageFlipVertical
                | Attributes::LocalVideoRecordingEnabled
                | Attributes::LocalSnapshotRecordingEnabled
                | Attributes::StatusLightEnabled => writer.set(false),
                Attributes::SpeakerVolumeLevel
                | Attributes::SpeakerMaxLevel
                | Attributes::SpeakerMinLevel
                | Attributes::MicrophoneVolumeLevel
                | Attributes::MicrophoneMaxLevel
                | Attributes::MicrophoneMinLevel => writer.set(0_u8),
                Attributes::ImageRotation => writer.set(0_u16),
                Attributes::StatusLightBrightness => writer.set(0_u8),
            }
        } else {
            Ok(())
        }
    }

    fn write(&self, ctx: impl WriteContext) -> Result<(), Error> {
        let attr = ctx.attr();
        let data = ctx.data();
        attr.check_dataver(self.dataver.get())?;

        let mut state = self.state.write().map_err(|_| ErrorCode::Busy)?;

        match attr.attr_id.try_into()? {
            Attributes::SoftRecordingPrivacyModeEnabled => {
                state.soft_recording_privacy_mode_enabled = data.bool()?;
            }
            Attributes::SoftLivestreamPrivacyModeEnabled => {
                state.soft_livestream_privacy_mode_enabled = data.bool()?;
            }
            Attributes::NightVision => {
                state.night_vision = match data.u8()? {
                    0 => crate::types::TriStateAuto::Off,
                    1 => crate::types::TriStateAuto::On,
                    _ => crate::types::TriStateAuto::Auto,
                };
            }
            Attributes::NightVisionIllum => {
                state.night_vision_illum = match data.u8()? {
                    0 => crate::types::TriStateAuto::Off,
                    1 => crate::types::TriStateAuto::On,
                    _ => crate::types::TriStateAuto::Auto,
                };
            }
            _ => return Err(ErrorCode::AttributeNotFound.into()),
        }

        self.dataver.changed();
        Ok(())
    }

    fn invoke(&self, ctx: impl InvokeContext, reply: impl InvokeReply) -> Result<(), Error> {
        let cmd = ctx.cmd();
        let data = ctx.data();
        // Helper: parse a struct field from the incoming TLV command data
        let fields = data.structure()?;

        match cmd.cmd_id.try_into()? {
            Commands::VideoStreamAllocate => {
                let mut state = self.state.write().map_err(|_| ErrorCode::Busy)?;
                let id = state.next_video_stream_id;
                state.next_video_stream_id += 1;

                let stream_usage = fields.find_ctx(0)?.u8().unwrap_or(3);
                let video_codec = fields.find_ctx(1)?.u8().unwrap_or(0);
                let min_frame_rate = fields.find_ctx(2)?.u16().unwrap_or(0);
                let max_frame_rate = fields.find_ctx(3)?.u16().unwrap_or(30);

                debug!(video_stream_id = id, "VideoStreamAllocate");

                let vs = crate::types::VideoStream {
                    video_stream_id: id,
                    stream_usage: parse_stream_usage(stream_usage),
                    video_codec: parse_video_codec(video_codec),
                    min_frame_rate,
                    max_frame_rate,
                    min_resolution: crate::types::VideoResolution {
                        width: 320,
                        height: 240,
                    },
                    max_resolution: crate::types::VideoResolution {
                        width: state.video_sensor_params.sensor_width,
                        height: state.video_sensor_params.sensor_height,
                    },
                    min_bit_rate: 100_000,
                    max_bit_rate: 10_000_000,
                    key_frame_interval: 30,
                    reference_count: None,
                };
                state.allocated_video_streams.push(vs);
                self.dataver.changed();

                let writer = reply.with_command(Commands::VideoStreamAllocateResponse as _)?;
                writer.set(id)
            }
            Commands::VideoStreamDeallocate => {
                let mut state = self.state.write().map_err(|_| ErrorCode::Busy)?;
                let stream_id = fields.find_ctx(0)?.u16().unwrap_or(0);
                state
                    .allocated_video_streams
                    .retain(|s| s.video_stream_id != stream_id);
                self.dataver.changed();
                debug!(video_stream_id = stream_id, "VideoStreamDeallocate");
                Ok(())
            }
            Commands::VideoStreamModify => {
                debug!("VideoStreamModify (no-op)");
                Ok(())
            }
            Commands::AudioStreamAllocate => {
                let mut state = self.state.write().map_err(|_| ErrorCode::Busy)?;
                let id = state.next_audio_stream_id;
                state.next_audio_stream_id += 1;

                let stream_usage = fields.find_ctx(0)?.u8().unwrap_or(3);
                let audio_codec = fields.find_ctx(1)?.u8().unwrap_or(0);
                let channel_count = fields.find_ctx(2)?.u8().unwrap_or(1);
                let sample_rate = fields.find_ctx(3)?.u32().unwrap_or(48000);
                let bit_rate = fields.find_ctx(4)?.u32().unwrap_or(64000);
                let bit_depth = fields.find_ctx(5)?.u8().unwrap_or(16);

                debug!(audio_stream_id = id, "AudioStreamAllocate");

                let aus = crate::types::AudioStream {
                    audio_stream_id: id,
                    stream_usage: parse_stream_usage(stream_usage),
                    audio_codec: parse_audio_codec(audio_codec),
                    channel_count,
                    sample_rate,
                    bit_rate,
                    bit_depth,
                    reference_count: None,
                };
                state.allocated_audio_streams.push(aus);
                self.dataver.changed();

                let writer = reply.with_command(Commands::AudioStreamAllocateResponse as _)?;
                writer.set(id)
            }
            Commands::AudioStreamDeallocate => {
                let mut state = self.state.write().map_err(|_| ErrorCode::Busy)?;
                let stream_id = fields.find_ctx(0)?.u16().unwrap_or(0);
                state
                    .allocated_audio_streams
                    .retain(|s| s.audio_stream_id != stream_id);
                self.dataver.changed();
                debug!(audio_stream_id = stream_id, "AudioStreamDeallocate");
                Ok(())
            }
            Commands::SnapshotStreamAllocate => {
                let mut state = self.state.write().map_err(|_| ErrorCode::Busy)?;
                let id = state.next_snapshot_stream_id;
                state.next_snapshot_stream_id += 1;
                let image_codec =
                    parse_image_codec(fields.find_ctx(0)?.u8().unwrap_or(IMAGE_CODEC_JPEG));
                let frame_rate = fields
                    .find_ctx(1)?
                    .u16()
                    .unwrap_or(state.video_sensor_params.max_fps);
                let min_resolution = parse_resolution_struct(fields.find_ctx(2)?)
                    .unwrap_or_else(|| state.min_viewport.clone());
                let max_resolution = parse_resolution_struct(fields.find_ctx(3)?)
                    .unwrap_or_else(|| state.viewport.clone());
                let quality = fields.find_ctx(4)?.u8().unwrap_or(90).min(100);

                state
                    .allocated_snapshot_streams
                    .push(crate::types::SnapshotStream {
                        snapshot_stream_id: id,
                        image_codec,
                        frame_rate,
                        min_resolution,
                        max_resolution,
                        quality,
                        reference_count: None,
                    });
                debug!(snapshot_stream_id = id, "SnapshotStreamAllocate");
                self.dataver.changed();

                let writer = reply.with_command(Commands::SnapshotStreamAllocateResponse as _)?;
                writer.set(id)
            }
            Commands::SnapshotStreamModify => {
                debug!("SnapshotStreamModify (no-op)");
                Ok(())
            }
            Commands::SnapshotStreamDeallocate => {
                let mut state = self.state.write().map_err(|_| ErrorCode::Busy)?;
                let stream_id = fields.find_ctx(0)?.u16().unwrap_or(0);
                state
                    .allocated_snapshot_streams
                    .retain(|s| s.snapshot_stream_id != stream_id);
                self.dataver.changed();
                Ok(())
            }
            Commands::SetStreamPriorities => {
                debug!("SetStreamPriorities (stub)");
                Ok(())
            }
            Commands::CaptureSnapshot => {
                let snapshot_stream_id = parse_nullable_u16(fields.find_ctx(0)?)?;
                let requested_resolution = parse_resolution_struct(fields.find_ctx(1)?)
                    .ok_or(ErrorCode::InvalidCommand)?;

                let snapshot_capture = self
                    .snapshot_capture
                    .as_ref()
                    .ok_or(ErrorCode::CommandNotFound)?;

                {
                    let state = self.state.read().map_err(|_| ErrorCode::Busy)?;
                    if let Some(stream_id) = snapshot_stream_id {
                        let stream_exists = state
                            .allocated_snapshot_streams
                            .iter()
                            .any(|s| s.snapshot_stream_id == stream_id);
                        if !stream_exists {
                            return Err(ErrorCode::InvalidCommand.into());
                        }
                    }
                }

                let result = snapshot_capture(SnapshotCaptureRequest {
                    snapshot_stream_id,
                    requested_resolution,
                })
                .map_err(|_| ErrorCode::Failure)?;

                let mut writer = reply.with_command(Commands::CaptureSnapshotResponse as _)?;
                let tag = writer.tag().clone();
                let mut tw = writer.writer();
                tw.start_struct(&tag)?;
                tw.str(&TLVTag::Context(0), &result.jpeg_bytes)?;
                tw.u8(&TLVTag::Context(1), IMAGE_CODEC_JPEG)?;
                tw.start_struct(&TLVTag::Context(2))?;
                tw.u16(&TLVTag::Context(0), result.resolution.width)?;
                tw.u16(&TLVTag::Context(1), result.resolution.height)?;
                tw.end_container()?;
                tw.end_container()?;
                drop(tw);
                writer.complete()
            }
            Commands::VideoStreamAllocateResponse
            | Commands::AudioStreamAllocateResponse
            | Commands::SnapshotStreamAllocateResponse
            | Commands::CaptureSnapshotResponse => Err(ErrorCode::CommandNotFound.into()),
        }
    }
}

impl NonBlockingHandler for AvStreamHandler {}

// ── Helpers ──

fn write_video_resolution(
    mut writer: impl Reply,
    res: &crate::types::VideoResolution,
) -> Result<(), Error> {
    let tag = writer.tag().clone();
    let mut tw = writer.writer();
    tw.start_struct(&tag)?;
    tw.u16(&TLVTag::Context(0), res.width)?;
    tw.u16(&TLVTag::Context(1), res.height)?;
    tw.end_container()?;
    drop(tw);
    writer.complete()
}

fn write_empty_array(mut writer: impl Reply) -> Result<(), Error> {
    let tag = writer.tag().clone();
    let mut tw = writer.writer();
    tw.start_array(&tag)?;
    tw.end_container()?;
    drop(tw);
    writer.complete()
}

fn write_snapshot_capabilities(
    mut writer: impl Reply,
    state: &CameraEndpointState,
) -> Result<(), Error> {
    let tag = writer.tag().clone();
    let mut tw = writer.writer();
    tw.start_array(&tag)?;
    tw.start_struct(&TLVTag::Anonymous)?;
    tw.start_struct(&TLVTag::Context(0))?;
    tw.u16(&TLVTag::Context(0), state.video_sensor_params.sensor_width)?;
    tw.u16(&TLVTag::Context(1), state.video_sensor_params.sensor_height)?;
    tw.end_container()?;
    tw.u16(&TLVTag::Context(1), state.video_sensor_params.max_fps)?;
    tw.u8(&TLVTag::Context(2), IMAGE_CODEC_JPEG)?;
    tw.bool(&TLVTag::Context(3), false)?;
    tw.end_container()?;
    tw.end_container()?;
    drop(tw);
    writer.complete()
}

fn write_supported_stream_usages(mut writer: impl Reply) -> Result<(), Error> {
    let tag = writer.tag().clone();
    let mut tw = writer.writer();
    tw.start_array(&tag)?;
    tw.u8(
        &TLVTag::Anonymous,
        crate::types::StreamUsage::LiveView as u8,
    )?;
    tw.end_container()?;
    drop(tw);
    writer.complete()
}

fn write_stream_usage_priorities(mut writer: impl Reply) -> Result<(), Error> {
    let tag = writer.tag().clone();
    let mut tw = writer.writer();
    tw.start_array(&tag)?;
    tw.u8(
        &TLVTag::Anonymous,
        crate::types::StreamUsage::LiveView as u8,
    )?;
    tw.end_container()?;
    drop(tw);
    writer.complete()
}

fn parse_stream_usage(v: u8) -> crate::types::StreamUsage {
    match v {
        0 => crate::types::StreamUsage::Internal,
        1 => crate::types::StreamUsage::Recording,
        2 => crate::types::StreamUsage::Analysis,
        _ => crate::types::StreamUsage::LiveView,
    }
}

fn parse_video_codec(v: u8) -> crate::types::VideoCodec {
    match v {
        0 => crate::types::VideoCodec::H264,
        1 => crate::types::VideoCodec::Hevc,
        2 => crate::types::VideoCodec::Vvc,
        _ => crate::types::VideoCodec::Av1,
    }
}

fn parse_audio_codec(v: u8) -> crate::types::AudioCodec {
    match v {
        0 => crate::types::AudioCodec::Opus,
        1 => crate::types::AudioCodec::AacLc,
        2 => crate::types::AudioCodec::G711A,
        _ => crate::types::AudioCodec::G711U,
    }
}

fn parse_image_codec(v: u8) -> crate::types::ImageCodec {
    match v {
        0 => crate::types::ImageCodec::Jpeg,
        _ => crate::types::ImageCodec::Heic,
    }
}

fn parse_resolution_struct(
    elem: rs_matter::tlv::TLVElement<'_>,
) -> Option<crate::types::VideoResolution> {
    let Ok(res_struct) = elem.structure() else {
        return None;
    };

    Some(crate::types::VideoResolution {
        width: res_struct.find_ctx(0).ok()?.u16().ok()?,
        height: res_struct.find_ctx(1).ok()?.u16().ok()?,
    })
}

fn parse_nullable_u16(elem: rs_matter::tlv::TLVElement<'_>) -> Result<Option<u16>, Error> {
    if elem.is_empty() {
        return Ok(None);
    }
    if elem.null().is_ok() {
        return Ok(None);
    }

    elem.u16()
        .map(Some)
        .map_err(|_| ErrorCode::InvalidCommand.into())
}

#[cfg(test)]
mod tests {
    use rs_matter::tlv::{TLVElement, TLVTag, TLVWrite};
    use rs_matter::utils::storage::WriteBuf;

    use super::{parse_nullable_u16, parse_resolution_struct};

    #[test]
    fn parse_resolution_struct_reads_width_and_height() {
        let mut backing = vec![0_u8; 128];
        let mut tw = WriteBuf::new(backing.as_mut_slice());
        tw.start_struct(&TLVTag::Anonymous).unwrap();
        tw.start_struct(&TLVTag::Context(1)).unwrap();
        tw.u16(&TLVTag::Context(0), 1280).unwrap();
        tw.u16(&TLVTag::Context(1), 720).unwrap();
        tw.end_container().unwrap();
        tw.end_container().unwrap();

        let encoded = tw.as_slice().to_vec();
        let root = TLVElement::new(&encoded);
        let fields = root.structure().unwrap();
        let parsed = parse_resolution_struct(fields.find_ctx(1).unwrap()).unwrap();
        assert_eq!(parsed.width, 1280);
        assert_eq!(parsed.height, 720);
    }

    #[test]
    fn parse_nullable_u16_accepts_null() {
        let mut backing = vec![0_u8; 128];
        let mut tw = WriteBuf::new(backing.as_mut_slice());
        tw.start_struct(&TLVTag::Anonymous).unwrap();
        tw.null(&TLVTag::Context(0)).unwrap();
        tw.end_container().unwrap();

        let encoded = tw.as_slice().to_vec();
        let root = TLVElement::new(&encoded);
        let fields = root.structure().unwrap();
        let parsed = parse_nullable_u16(fields.find_ctx(0).unwrap()).unwrap();
        assert_eq!(parsed, None);
    }
}
