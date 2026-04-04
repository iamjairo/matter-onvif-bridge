/**
 * CameraAvStreamManagement Server Behavior
 *
 * Implements the server-side behavior for camera AV stream management.
 * Manages video/audio stream allocation and lifecycle.
 */
import { CameraAvStreamManagementBehavior, CameraAvStreamManagement } from "./cluster.js";
import { createChildLogger } from "../../../logger.js";

const log = createChildLogger("camera-av-stream-server");

let nextVideoStreamId = 1;
let nextAudioStreamId = 1;
let nextSnapshotStreamId = 1;

class CameraAvStreamManagementServerBase extends CameraAvStreamManagementBehavior {
  declare state: any;

  override initialize() {
    // State is initialized from the endpoint configuration in bridge.ts
  }

  audioStreamAllocate(request: any) {
    const audioStreamId = nextAudioStreamId++;
    const stream = {
      audioStreamId,
      streamUsage: request.streamUsage,
      audioCodec: request.audioCodec,
      channelCount: request.channelCount,
      sampleRate: request.sampleRate,
      bitRate: request.bitRate,
      bitDepth: request.bitDepth,
      referenceCount: 1,
    };
    const streams = [...((this.state as any).allocatedAudioStreams || []), stream];
    (this.state as any).allocatedAudioStreams = streams;
    log.debug({ audioStreamId }, "Allocated audio stream");
    return { audioStreamId };
  }

  audioStreamDeallocate(request: any) {
    const streams = ((this.state as any).allocatedAudioStreams || []).filter(
      (s: any) => s.audioStreamId !== request.audioStreamId,
    );
    (this.state as any).allocatedAudioStreams = streams;
    log.debug({ audioStreamId: request.audioStreamId }, "Deallocated audio stream");
  }

  videoStreamAllocate(request: any) {
    const videoStreamId = nextVideoStreamId++;
    const stream = {
      videoStreamId,
      streamUsage: request.streamUsage,
      videoCodec: request.videoCodec,
      minFrameRate: request.minFrameRate,
      maxFrameRate: request.maxFrameRate,
      minResolution: request.minResolution,
      maxResolution: request.maxResolution,
      minBitRate: request.minBitRate,
      maxBitRate: request.maxBitRate,
      minFragmentLen: request.minFragmentLen,
      maxFragmentLen: request.maxFragmentLen,
      referenceCount: 1,
    };
    const streams = [...((this.state as any).allocatedVideoStreams || []), stream];
    (this.state as any).allocatedVideoStreams = streams;
    log.debug({ videoStreamId }, "Allocated video stream");
    return { videoStreamId };
  }

  videoStreamModify(_request: any) {
    // No-op for MVP — go2rtc handles actual stream config
  }

  videoStreamDeallocate(request: any) {
    const streams = ((this.state as any).allocatedVideoStreams || []).filter(
      (s: any) => s.videoStreamId !== request.videoStreamId,
    );
    (this.state as any).allocatedVideoStreams = streams;
    log.debug({ videoStreamId: request.videoStreamId }, "Deallocated video stream");
  }

  snapshotStreamAllocate(request: any) {
    const snapshotStreamId = nextSnapshotStreamId++;
    const stream = {
      snapshotStreamId,
      imageCodec: request.imageCodec,
      frameRate: request.frameRate,
      minResolution: request.minResolution,
      maxResolution: request.maxResolution,
      quality: request.quality,
      referenceCount: 1,
    };
    const streams = [...((this.state as any).allocatedSnapshotStreams || []), stream];
    (this.state as any).allocatedSnapshotStreams = streams;
    log.debug({ snapshotStreamId }, "Allocated snapshot stream");
    return { snapshotStreamId };
  }

  snapshotStreamDeallocate(request: any) {
    const streams = ((this.state as any).allocatedSnapshotStreams || []).filter(
      (s: any) => s.snapshotStreamId !== request.snapshotStreamId,
    );
    (this.state as any).allocatedSnapshotStreams = streams;
    log.debug({ snapshotStreamId: request.snapshotStreamId }, "Deallocated snapshot stream");
  }

  setStreamPriorities(request: any) {
    (this.state as any).rankedVideoStreamPrioritiesList = request.streamPriorities;
  }

  captureSnapshot(_request: any): any {
    throw new Error("Snapshot capture not yet implemented");
  }
}

// Fix featureMap: matter.js BitmapManager expects an object, not a number.
const OrigState = CameraAvStreamManagementServerBase.State;
const FixedState = function (this: any) {
  OrigState.call(this);
  this.featureMap = {};
};
FixedState.prototype = Object.create(OrigState.prototype);
Object.defineProperty(CameraAvStreamManagementServerBase, "State", { value: FixedState });

// Map command handlers to the numeric keys that ValidatedElements expects.
// ClusterType(model) produces commands keyed by index ("0","1",...), not camelCase names.
// Response commands (odd indices for request/response pairs) are mapped as no-ops.
const commands = CameraAvStreamManagement.commands as Record<string, any> | undefined;
const proto = CameraAvStreamManagementServerBase.prototype as any;
const methodMap: Record<string, string> = {
  AudioStreamAllocate: "audioStreamAllocate",
  AudioStreamDeallocate: "audioStreamDeallocate",
  VideoStreamAllocate: "videoStreamAllocate",
  VideoStreamModify: "videoStreamModify",
  VideoStreamDeallocate: "videoStreamDeallocate",
  SnapshotStreamAllocate: "snapshotStreamAllocate",
  SnapshotStreamDeallocate: "snapshotStreamDeallocate",
  SetStreamPriorities: "setStreamPriorities",
  CaptureSnapshot: "captureSnapshot",
};
for (const key of Object.keys(commands ?? {})) {
  const cmd = commands![key];
  if (!cmd) continue;
  const method = methodMap[cmd.name];
  if (method && proto[method]) {
    // Map numeric key to the named method
    proto[key] = proto[method];
  } else if (cmd.name.endsWith("Response")) {
    // Response commands are not invocable — provide a no-op
    proto[key] = function () {};
  }
}

export const CameraAvStreamManagementServer = CameraAvStreamManagementServerBase;
