import { createChildLogger } from "../logger.js";
import { Config } from "../config.js";
import type { CameraRegistry } from "../registry/camera-registry.js";
import type { CameraDevice } from "../registry/camera.js";
import { Go2RtcApi } from "./go2rtc-api.js";

const log = createChildLogger("stream-manager");

export class StreamManager {
  private api: Go2RtcApi;

  constructor(
    private readonly registry: CameraRegistry,
    api?: Go2RtcApi,
  ) {
    this.api = api || new Go2RtcApi();
  }

  /** Wire up to camera registry events and register existing cameras */
  start(): void {
    this.registry.on("added", (camera) => this.onCameraAdded(camera));
    this.registry.on("removed", (camera) => this.onCameraRemoved(camera));

    // Register any cameras that were already discovered
    for (const camera of this.registry.getAllCameras()) {
      this.onCameraAdded(camera);
    }

    log.info("Stream manager started");
  }

  private async onCameraAdded(camera: CameraDevice): Promise<void> {
    try {
      // Inject ONVIF credentials into the RTSP URI if not present
      const rtspUrl = this.buildAuthenticatedRtspUrl(camera);
      await this.api.addStream(camera.streamName, rtspUrl);
    } catch (err) {
      log.error({ err, cameraId: camera.info.id }, "Failed to register stream in go2rtc");
    }
  }

  private async onCameraRemoved(camera: CameraDevice): Promise<void> {
    try {
      await this.api.removeStream(camera.streamName);
    } catch (err) {
      log.error({ err, cameraId: camera.info.id }, "Failed to remove stream from go2rtc");
    }
  }

  /** Build RTSP URL with embedded credentials if not already present */
  private buildAuthenticatedRtspUrl(camera: CameraDevice): string {
    const uri = camera.info.streamUri;
    if (!uri) return "";

    try {
      const url = new URL(uri);
      if (!url.username) {
        // Use the ONVIF credentials for RTSP auth
        const { username, password } = Config.onvif;
        url.username = username;
        url.password = password;
      }
      return url.toString();
    } catch {
      // If URL parsing fails, return as-is
      return uri;
    }
  }
}
