import type { OnvifCameraInfo } from "../onvif/types.js";

export interface CameraDevice {
  /** ONVIF camera info */
  info: OnvifCameraInfo;
  /** go2rtc stream name (used as identifier for go2rtc API) */
  streamName: string;
  /** Whether the camera is currently reachable */
  reachable: boolean;
  /** Matter endpoint ID (assigned when added to bridge) */
  endpointId?: number;
}

export function createCameraDevice(info: OnvifCameraInfo): CameraDevice {
  return {
    info,
    streamName: sanitizeStreamName(info.id),
    reachable: true,
  };
}

/** Create a go2rtc-safe stream name from camera ID */
function sanitizeStreamName(id: string): string {
  return id.replace(/[^a-zA-Z0-9_-]/g, "_").toLowerCase();
}
