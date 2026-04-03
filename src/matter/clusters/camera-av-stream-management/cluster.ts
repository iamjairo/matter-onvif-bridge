/**
 * Camera AV Stream Management Cluster (0x0551)
 *
 * Defined in Matter 1.5 specification. This cluster manages the audio/video
 * stream lifecycle for camera devices, including video/audio stream allocation,
 * configuration, and deallocation.
 *
 * We define the cluster using ClusterModel (from @matter/model) which is the
 * proper way to create custom clusters in matter.js. The deprecated ClusterType
 * options bag doesn't work with ClusterBehavior.for().
 */
import { ClusterModel } from "@matter/model";
import { ClusterType } from "@matter/types/cluster";
import { ClusterBehavior } from "@matter/main";

// --- Cluster Model Definition ---

export const CameraAvStreamManagementModel = new ClusterModel({
  id: 0x0551,
  name: "CameraAvStreamManagement",
  classification: "application",

  children: [
    // --- Enums ---
    {
      tag: "datatype", name: "TwoWayTalkSupportTypeEnum", type: "enum8",
      children: [
        { tag: "field", id: 0, name: "NotSupported" },
        { tag: "field", id: 1, name: "HalfDuplex" },
        { tag: "field", id: 2, name: "FullDuplex" },
      ],
    },
    {
      tag: "datatype", name: "TriStateAutoEnum", type: "enum8",
      children: [
        { tag: "field", id: 0, name: "Off" },
        { tag: "field", id: 1, name: "On" },
        { tag: "field", id: 2, name: "Auto" },
      ],
    },
    {
      tag: "datatype", name: "VideoCodecEnum", type: "enum8",
      children: [
        { tag: "field", id: 0, name: "H264" },
        { tag: "field", id: 1, name: "HEVC" },
        { tag: "field", id: 2, name: "VVC" },
        { tag: "field", id: 3, name: "AV1" },
      ],
    },
    {
      tag: "datatype", name: "AudioCodecEnum", type: "enum8",
      children: [
        { tag: "field", id: 0, name: "OPUS" },
        { tag: "field", id: 1, name: "AacLc" },
        { tag: "field", id: 2, name: "G711A" },
        { tag: "field", id: 3, name: "G711U" },
      ],
    },
    {
      tag: "datatype", name: "StreamUsageEnum", type: "enum8",
      children: [
        { tag: "field", id: 0, name: "Internal" },
        { tag: "field", id: 1, name: "Recording" },
        { tag: "field", id: 2, name: "Analysis" },
        { tag: "field", id: 3, name: "LiveView" },
      ],
    },

    // --- Structs (as datatypes) ---
    {
      tag: "datatype", name: "VideoResolutionStruct", type: "struct",
      children: [
        { tag: "field", id: 0, name: "Width", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "Height", type: "uint16", conformance: "M" },
      ],
    },
    {
      tag: "datatype", name: "VideoSensorParamsStruct", type: "struct",
      children: [
        { tag: "field", id: 0, name: "SensorWidth", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "SensorHeight", type: "uint16", conformance: "M" },
        { tag: "field", id: 2, name: "HdrCapable", type: "bool", conformance: "M" },
        { tag: "field", id: 3, name: "MaxFps", type: "uint16", conformance: "M" },
      ],
    },
    {
      tag: "datatype", name: "AudioCapabilitiesStruct", type: "struct",
      children: [
        { tag: "field", id: 0, name: "MaxNumberOfChannels", type: "uint8", conformance: "M" },
        { tag: "field", id: 1, name: "SupportedCodecs", type: "list", conformance: "M",
          children: [{ tag: "field", name: "entry", type: "AudioCodecEnum" }],
        },
      ],
    },
    {
      tag: "datatype", name: "SnapshotParamsStruct", type: "struct",
      children: [
        { tag: "field", id: 0, name: "Resolution", type: "VideoResolutionStruct", conformance: "M" },
        { tag: "field", id: 1, name: "MaxFrameRate", type: "uint16", conformance: "M" },
        { tag: "field", id: 2, name: "ImageCodec", type: "VideoCodecEnum", conformance: "M" },
      ],
    },
    {
      tag: "datatype", name: "VideoStreamStruct", type: "struct",
      children: [
        { tag: "field", id: 0, name: "VideoStreamId", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "StreamUsage", type: "StreamUsageEnum", conformance: "M" },
        { tag: "field", id: 2, name: "VideoCodec", type: "VideoCodecEnum", conformance: "M" },
        { tag: "field", id: 3, name: "MinFrameRate", type: "uint16", conformance: "M" },
        { tag: "field", id: 4, name: "MaxFrameRate", type: "uint16", conformance: "M" },
        { tag: "field", id: 5, name: "MinResolution", type: "VideoResolutionStruct", conformance: "M" },
        { tag: "field", id: 6, name: "MaxResolution", type: "VideoResolutionStruct", conformance: "M" },
        { tag: "field", id: 7, name: "MinBitRate", type: "uint32", conformance: "M" },
        { tag: "field", id: 8, name: "MaxBitRate", type: "uint32", conformance: "M" },
        { tag: "field", id: 9, name: "MinFragmentLen", type: "uint16", conformance: "M" },
        { tag: "field", id: 10, name: "MaxFragmentLen", type: "uint16", conformance: "M" },
        { tag: "field", id: 11, name: "ReferenceCount", type: "uint8", conformance: "O" },
      ],
    },
    {
      tag: "datatype", name: "AudioStreamStruct", type: "struct",
      children: [
        { tag: "field", id: 0, name: "AudioStreamId", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "StreamUsage", type: "StreamUsageEnum", conformance: "M" },
        { tag: "field", id: 2, name: "AudioCodec", type: "AudioCodecEnum", conformance: "M" },
        { tag: "field", id: 3, name: "ChannelCount", type: "uint8", conformance: "M" },
        { tag: "field", id: 4, name: "SampleRate", type: "uint32", conformance: "M" },
        { tag: "field", id: 5, name: "BitRate", type: "uint32", conformance: "M" },
        { tag: "field", id: 6, name: "BitDepth", type: "uint8", conformance: "M" },
        { tag: "field", id: 7, name: "ReferenceCount", type: "uint8", conformance: "O" },
      ],
    },
    {
      tag: "datatype", name: "SnapshotStreamStruct", type: "struct",
      children: [
        { tag: "field", id: 0, name: "SnapshotStreamId", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "ImageCodec", type: "VideoCodecEnum", conformance: "M" },
        { tag: "field", id: 2, name: "FrameRate", type: "uint16", conformance: "M" },
        { tag: "field", id: 3, name: "MinResolution", type: "VideoResolutionStruct", conformance: "M" },
        { tag: "field", id: 4, name: "MaxResolution", type: "VideoResolutionStruct", conformance: "M" },
        { tag: "field", id: 5, name: "Quality", type: "uint8", conformance: "M" },
        { tag: "field", id: 6, name: "ReferenceCount", type: "uint8", conformance: "O" },
      ],
    },

    // --- Global attributes ---
    { tag: "attribute", id: 0xfffc, name: "FeatureMap", type: "map32", access: "R", conformance: "M", default: 0 },

    // --- Attributes ---
    { tag: "attribute", id: 0x0000, name: "MaxConcurrentVideoEncoders", type: "uint8", access: "R", conformance: "M", quality: "F", default: 1 },
    { tag: "attribute", id: 0x0001, name: "MaxEncodedPixelRate", type: "uint32", access: "R", conformance: "M", quality: "F" },
    { tag: "attribute", id: 0x0002, name: "VideoSensorParams", type: "VideoSensorParamsStruct", access: "R", conformance: "M", quality: "F" },
    { tag: "attribute", id: 0x0003, name: "NightVisionCapable", type: "bool", access: "R", conformance: "M", quality: "F", default: false },
    { tag: "attribute", id: 0x0004, name: "MinViewport", type: "VideoResolutionStruct", access: "R", conformance: "M", quality: "F" },
    { tag: "attribute", id: 0x0006, name: "MicrophoneCapabilities", type: "list", access: "R", conformance: "M", quality: "F",
      children: [{ tag: "field", name: "entry", type: "AudioCapabilitiesStruct" }],
    },
    { tag: "attribute", id: 0x0007, name: "SpeakerCapabilities", type: "list", access: "R", conformance: "M", quality: "F",
      children: [{ tag: "field", name: "entry", type: "AudioCapabilitiesStruct" }],
    },
    { tag: "attribute", id: 0x0008, name: "TwoWayTalkSupport", type: "TwoWayTalkSupportTypeEnum", access: "R", conformance: "M", quality: "F", default: 0 },
    { tag: "attribute", id: 0x0009, name: "SupportedSnapshotParams", type: "list", access: "R", conformance: "M", quality: "F",
      children: [{ tag: "field", name: "entry", type: "SnapshotParamsStruct" }],
    },
    { tag: "attribute", id: 0x000a, name: "HdrCapable", type: "bool", access: "R", conformance: "M", quality: "F", default: false },
    { tag: "attribute", id: 0x000b, name: "MaxNetworkBandwidth", type: "uint32", access: "R", conformance: "M", quality: "F" },
    { tag: "attribute", id: 0x000c, name: "CurrentFrameRate", type: "uint16", access: "R", conformance: "M", default: 0 },
    { tag: "attribute", id: 0x000e, name: "AllocatedVideoStreams", type: "list", access: "R", conformance: "M",
      children: [{ tag: "field", name: "entry", type: "VideoStreamStruct" }],
    },
    { tag: "attribute", id: 0x000f, name: "AllocatedAudioStreams", type: "list", access: "R", conformance: "M",
      children: [{ tag: "field", name: "entry", type: "AudioStreamStruct" }],
    },
    { tag: "attribute", id: 0x0010, name: "AllocatedSnapshotStreams", type: "list", access: "R", conformance: "M",
      children: [{ tag: "field", name: "entry", type: "SnapshotStreamStruct" }],
    },
    { tag: "attribute", id: 0x0011, name: "RankedVideoStreamPrioritiesList", type: "list", access: "RW", conformance: "M",
      children: [{ tag: "field", name: "entry", type: "uint16" }],
    },
    { tag: "attribute", id: 0x0012, name: "SoftRecordingPrivacyModeEnabled", type: "bool", access: "RW", conformance: "M", default: false },
    { tag: "attribute", id: 0x0013, name: "SoftLivestreamPrivacyModeEnabled", type: "bool", access: "RW", conformance: "M", default: false },
    { tag: "attribute", id: 0x0014, name: "HardPrivacyModeOn", type: "bool", access: "R", conformance: "M", default: false },
    { tag: "attribute", id: 0x0015, name: "NightVision", type: "TriStateAutoEnum", access: "RW", conformance: "O" },
    { tag: "attribute", id: 0x0016, name: "NightVisionIllum", type: "TriStateAutoEnum", access: "RW", conformance: "O" },
    { tag: "attribute", id: 0x0017, name: "AwbEnabled", type: "bool", access: "RW", conformance: "O", default: true },
    { tag: "attribute", id: 0x0018, name: "AutoShutterSpeedEnabled", type: "bool", access: "RW", conformance: "O", default: true },
    { tag: "attribute", id: 0x0019, name: "AutoIsoEnabled", type: "bool", access: "RW", conformance: "O", default: true },
    { tag: "attribute", id: 0x001a, name: "Viewport", type: "VideoResolutionStruct", access: "R", conformance: "O" },

    // --- Commands ---
    // AudioStreamAllocate (request 0x00 -> response 0x01)
    {
      tag: "command", id: 0x0000, name: "AudioStreamAllocate", direction: "request", access: "O", conformance: "M", response: "AudioStreamAllocateResponse",
      children: [
        { tag: "field", id: 0, name: "StreamUsage", type: "StreamUsageEnum", conformance: "M" },
        { tag: "field", id: 1, name: "AudioCodec", type: "AudioCodecEnum", conformance: "M" },
        { tag: "field", id: 2, name: "ChannelCount", type: "uint8", conformance: "M" },
        { tag: "field", id: 3, name: "SampleRate", type: "uint32", conformance: "M" },
        { tag: "field", id: 4, name: "BitRate", type: "uint32", conformance: "M" },
        { tag: "field", id: 5, name: "BitDepth", type: "uint8", conformance: "M" },
      ],
    },
    {
      tag: "command", id: 0x0001, name: "AudioStreamAllocateResponse", direction: "response", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "AudioStreamId", type: "uint16", conformance: "M" },
      ],
    },
    // AudioStreamDeallocate (request 0x02)
    {
      tag: "command", id: 0x0002, name: "AudioStreamDeallocate", direction: "request", access: "O", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "AudioStreamId", type: "uint16", conformance: "M" },
      ],
    },
    // VideoStreamAllocate (request 0x03 -> response 0x04)
    {
      tag: "command", id: 0x0003, name: "VideoStreamAllocate", direction: "request", access: "O", conformance: "M", response: "VideoStreamAllocateResponse",
      children: [
        { tag: "field", id: 0, name: "StreamUsage", type: "StreamUsageEnum", conformance: "M" },
        { tag: "field", id: 1, name: "VideoCodec", type: "VideoCodecEnum", conformance: "M" },
        { tag: "field", id: 2, name: "MinFrameRate", type: "uint16", conformance: "M" },
        { tag: "field", id: 3, name: "MaxFrameRate", type: "uint16", conformance: "M" },
        { tag: "field", id: 4, name: "MinResolution", type: "VideoResolutionStruct", conformance: "M" },
        { tag: "field", id: 5, name: "MaxResolution", type: "VideoResolutionStruct", conformance: "M" },
        { tag: "field", id: 6, name: "MinBitRate", type: "uint32", conformance: "M" },
        { tag: "field", id: 7, name: "MaxBitRate", type: "uint32", conformance: "M" },
        { tag: "field", id: 8, name: "MinFragmentLen", type: "uint16", conformance: "M" },
        { tag: "field", id: 9, name: "MaxFragmentLen", type: "uint16", conformance: "M" },
      ],
    },
    {
      tag: "command", id: 0x0004, name: "VideoStreamAllocateResponse", direction: "response", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "VideoStreamId", type: "uint16", conformance: "M" },
      ],
    },
    // VideoStreamModify (request 0x05)
    {
      tag: "command", id: 0x0005, name: "VideoStreamModify", direction: "request", access: "O", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "VideoStreamId", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "Resolution", type: "VideoResolutionStruct", conformance: "O" },
      ],
    },
    // VideoStreamDeallocate (request 0x06)
    {
      tag: "command", id: 0x0006, name: "VideoStreamDeallocate", direction: "request", access: "O", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "VideoStreamId", type: "uint16", conformance: "M" },
      ],
    },
    // SnapshotStreamAllocate (request 0x07 -> response 0x08)
    {
      tag: "command", id: 0x0007, name: "SnapshotStreamAllocate", direction: "request", access: "O", conformance: "M", response: "SnapshotStreamAllocateResponse",
      children: [
        { tag: "field", id: 0, name: "ImageCodec", type: "VideoCodecEnum", conformance: "M" },
        { tag: "field", id: 1, name: "FrameRate", type: "uint16", conformance: "M" },
        { tag: "field", id: 2, name: "MinResolution", type: "VideoResolutionStruct", conformance: "M" },
        { tag: "field", id: 3, name: "MaxResolution", type: "VideoResolutionStruct", conformance: "M" },
        { tag: "field", id: 4, name: "Quality", type: "uint8", conformance: "M" },
      ],
    },
    {
      tag: "command", id: 0x0008, name: "SnapshotStreamAllocateResponse", direction: "response", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "SnapshotStreamId", type: "uint16", conformance: "M" },
      ],
    },
    // SnapshotStreamDeallocate (request 0x09)
    {
      tag: "command", id: 0x0009, name: "SnapshotStreamDeallocate", direction: "request", access: "O", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "SnapshotStreamId", type: "uint16", conformance: "M" },
      ],
    },
    // SetStreamPriorities (request 0x0a)
    {
      tag: "command", id: 0x000a, name: "SetStreamPriorities", direction: "request", access: "O", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "StreamPriorities", type: "list", conformance: "M",
          children: [{ tag: "field", name: "entry", type: "uint16" }],
        },
      ],
    },
    // CaptureSnapshot (request 0x0b -> response 0x0c)
    {
      tag: "command", id: 0x000b, name: "CaptureSnapshot", direction: "request", access: "O", conformance: "M", response: "CaptureSnapshotResponse",
      children: [
        { tag: "field", id: 0, name: "SnapshotStreamId", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "RequestedResolution", type: "VideoResolutionStruct", conformance: "M" },
      ],
    },
    {
      tag: "command", id: 0x000c, name: "CaptureSnapshotResponse", direction: "response", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "Data", type: "octstr", conformance: "M" },
        { tag: "field", id: 1, name: "ImageCodec", type: "VideoCodecEnum", conformance: "M" },
        { tag: "field", id: 2, name: "Resolution", type: "VideoResolutionStruct", conformance: "M" },
      ],
    },
  ],
});

// Create the cluster namespace from the model
export const CameraAvStreamManagement = ClusterType(CameraAvStreamManagementModel as any);

// Create the behavior base class
export const CameraAvStreamManagementBehavior = ClusterBehavior.for(
  CameraAvStreamManagement,
  CameraAvStreamManagementModel,
);
