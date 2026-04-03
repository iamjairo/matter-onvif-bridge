/**
 * WebRTC Transport Provider Cluster (0x0553)
 *
 * Defined in Matter 1.5 specification. This cluster enables WebRTC-based
 * peer-to-peer video/audio streaming between a camera and a controller.
 */
import { ClusterModel } from "@matter/model";
import { ClusterType } from "@matter/types/cluster";
import { ClusterBehavior } from "@matter/main";

export const WebRtcTransportProviderModel = new ClusterModel({
  id: 0x0553,
  name: "WebRtcTransportProvider",
  classification: "application",

  children: [
    // --- Enums ---
    {
      tag: "datatype", name: "StreamUsageEnum", type: "enum8",
      children: [
        { tag: "field", id: 0, name: "Internal" },
        { tag: "field", id: 1, name: "Recording" },
        { tag: "field", id: 2, name: "Analysis" },
        { tag: "field", id: 3, name: "LiveView" },
      ],
    },
    {
      tag: "datatype", name: "WebRtcEndReasonEnum", type: "enum8",
      children: [
        { tag: "field", id: 0, name: "IceFailed" },
        { tag: "field", id: 1, name: "IceTimeout" },
        { tag: "field", id: 2, name: "UserHangup" },
        { tag: "field", id: 3, name: "UserBusy" },
        { tag: "field", id: 4, name: "Replaced" },
        { tag: "field", id: 5, name: "NoUserMedia" },
        { tag: "field", id: 6, name: "InviteTimeout" },
        { tag: "field", id: 7, name: "AnsweredElsewhere" },
        { tag: "field", id: 8, name: "OutOfResources" },
        { tag: "field", id: 9, name: "MediaTimeout" },
        { tag: "field", id: 10, name: "LowPower" },
        { tag: "field", id: 11, name: "Unknown" },
      ],
    },

    // --- Structs ---
    {
      tag: "datatype", name: "IceServerStruct", type: "struct",
      children: [
        { tag: "field", id: 1, name: "Urls", type: "list", conformance: "M",
          children: [{ tag: "field", name: "entry", type: "string" }],
        },
        { tag: "field", id: 2, name: "Username", type: "string", conformance: "O" },
        { tag: "field", id: 3, name: "Credential", type: "string", conformance: "O" },
      ],
    },
    {
      tag: "datatype", name: "WebRtcSessionStruct", type: "struct",
      children: [
        { tag: "field", id: 1, name: "Id", type: "uint16", conformance: "M" },
        { tag: "field", id: 2, name: "PeerNodeId", type: "uint64", conformance: "M" },
        { tag: "field", id: 3, name: "PeerFabricIndex", type: "uint8", conformance: "M" },
        { tag: "field", id: 4, name: "StreamUsage", type: "StreamUsageEnum", conformance: "M" },
        { tag: "field", id: 5, name: "VideoStreamId", type: "uint16", conformance: "O", quality: "X" },
        { tag: "field", id: 6, name: "AudioStreamId", type: "uint16", conformance: "O", quality: "X" },
        { tag: "field", id: 7, name: "MetadataOptions", type: "uint8", conformance: "O" },
      ],
    },

    // --- Global attributes ---
    { tag: "attribute", id: 0xfffc, name: "FeatureMap", type: "map32", access: "R", conformance: "M", default: 0 },

    // --- Attributes ---
    {
      tag: "attribute", id: 0x0000, name: "CurrentSessions", type: "list", access: "R", conformance: "M",
      children: [{ tag: "field", name: "entry", type: "WebRtcSessionStruct" }],
    },

    // --- Commands ---
    // ProvideOffer (request 0x01 -> response 0x02)
    {
      tag: "command", id: 0x0001, name: "ProvideOffer", direction: "request", access: "O", conformance: "M", response: "ProvideOfferResponse",
      children: [
        { tag: "field", id: 0, name: "WebRtcSessionId", type: "uint16", conformance: "O", quality: "X" },
        { tag: "field", id: 1, name: "Sdp", type: "string", conformance: "M" },
        { tag: "field", id: 2, name: "StreamUsage", type: "StreamUsageEnum", conformance: "M" },
        { tag: "field", id: 3, name: "VideoStreamId", type: "uint16", conformance: "O", quality: "X" },
        { tag: "field", id: 4, name: "AudioStreamId", type: "uint16", conformance: "O", quality: "X" },
        { tag: "field", id: 5, name: "IceServers", type: "list", conformance: "O",
          children: [{ tag: "field", name: "entry", type: "IceServerStruct" }],
        },
        { tag: "field", id: 6, name: "IceTransportPolicy", type: "string", conformance: "O" },
        { tag: "field", id: 7, name: "MetadataOptions", type: "uint8", conformance: "O" },
      ],
    },
    {
      tag: "command", id: 0x0002, name: "ProvideOfferResponse", direction: "response", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "WebRtcSessionId", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "Sdp", type: "string", conformance: "M" },
        { tag: "field", id: 2, name: "VideoStreamId", type: "uint16", conformance: "O" },
        { tag: "field", id: 3, name: "AudioStreamId", type: "uint16", conformance: "O" },
      ],
    },
    // SolicitOffer (request 0x03 -> response 0x04)
    {
      tag: "command", id: 0x0003, name: "SolicitOffer", direction: "request", access: "O", conformance: "M", response: "SolicitOfferResponse",
      children: [
        { tag: "field", id: 0, name: "StreamUsage", type: "StreamUsageEnum", conformance: "M" },
        { tag: "field", id: 1, name: "VideoStreamId", type: "uint16", conformance: "O", quality: "X" },
        { tag: "field", id: 2, name: "AudioStreamId", type: "uint16", conformance: "O", quality: "X" },
        { tag: "field", id: 3, name: "IceServers", type: "list", conformance: "O",
          children: [{ tag: "field", name: "entry", type: "IceServerStruct" }],
        },
        { tag: "field", id: 4, name: "IceTransportPolicy", type: "string", conformance: "O" },
        { tag: "field", id: 5, name: "MetadataOptions", type: "uint8", conformance: "O" },
      ],
    },
    {
      tag: "command", id: 0x0004, name: "SolicitOfferResponse", direction: "response", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "WebRtcSessionId", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "DeferredOffer", type: "bool", conformance: "M" },
        { tag: "field", id: 2, name: "VideoStreamId", type: "uint16", conformance: "O" },
        { tag: "field", id: 3, name: "AudioStreamId", type: "uint16", conformance: "O" },
      ],
    },
    // ProvideAnswer (request 0x05)
    {
      tag: "command", id: 0x0005, name: "ProvideAnswer", direction: "request", access: "O", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "WebRtcSessionId", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "Sdp", type: "string", conformance: "M" },
      ],
    },
    // ProvideIceCandidates (request 0x06)
    {
      tag: "command", id: 0x0006, name: "ProvideIceCandidates", direction: "request", access: "O", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "WebRtcSessionId", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "IceCandidates", type: "list", conformance: "M",
          children: [{ tag: "field", name: "entry", type: "string" }],
        },
      ],
    },
    // EndSession (request 0x07)
    {
      tag: "command", id: 0x0007, name: "EndSession", direction: "request", access: "O", conformance: "M",
      children: [
        { tag: "field", id: 0, name: "WebRtcSessionId", type: "uint16", conformance: "M" },
        { tag: "field", id: 1, name: "Reason", type: "WebRtcEndReasonEnum", conformance: "M" },
      ],
    },
  ],
});

export const WebRtcTransportProvider = ClusterType(WebRtcTransportProviderModel as any);

export const WebRtcTransportProviderBehavior = ClusterBehavior.for(
  WebRtcTransportProvider,
  WebRtcTransportProviderModel,
);
