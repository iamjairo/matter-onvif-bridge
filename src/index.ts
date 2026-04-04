/**
 * Matter-ONVIF Bridge
 *
 * Discovers ONVIF IP cameras on the local network and exposes them
 * as Matter camera devices via a Matter bridge. Uses go2rtc for
 * RTSP-to-WebRTC media bridging.
 */
import "@matter/nodejs";
import { createChildLogger } from "./logger.js";
import { Config } from "./config.js";
import { OnvifDiscovery } from "./onvif/discovery.js";
import { CameraRegistry } from "./registry/camera-registry.js";
import { Go2RtcManager } from "./media/go2rtc-manager.js";
import { Go2RtcApi } from "./media/go2rtc-api.js";
import { StreamManager } from "./media/stream-manager.js";
import { MatterBridge } from "./matter/bridge.js";

const log = createChildLogger("main");

async function main() {
  log.info("Starting Matter-ONVIF Bridge");

  // 1. Camera registry — central store for discovered cameras
  const registry = new CameraRegistry();

  // 2. Start go2rtc media server
  const go2rtcManager = new Go2RtcManager();
  await go2rtcManager.start();
  const go2rtcApi = new Go2RtcApi();

  // 3. Stream manager — registers RTSP streams in go2rtc as cameras are discovered
  const streamManager = new StreamManager(registry, go2rtcApi);
  streamManager.start();

  // 4. Start Matter bridge
  const bridge = new MatterBridge(registry, go2rtcApi);
  await bridge.start();

  // 5. Start ONVIF discovery — finds cameras and populates the registry
  const discovery = new OnvifDiscovery();
  discovery.on("cameraFound", (cameraInfo) => {
    registry.addCamera(cameraInfo);
  });
  discovery.on("cameraLost", (cameraId) => {
    registry.removeCamera(cameraId);
  });
  discovery.on("cameraUnreachable", (cameraId) => {
    registry.markUnreachable(cameraId);
  });
  discovery.on("error", (err) => {
    log.error({ err }, "ONVIF discovery error");
  });
  await discovery.start();

  log.info(
    {
      matterPort: Config.matter.port,
      go2rtcPort: Config.go2rtc.apiPort,
      discoveryInterval: Config.onvif.discoveryInterval,
    },
    "Matter-ONVIF Bridge is running",
  );

  // Graceful shutdown
  const shutdown = async (signal: string) => {
    log.info({ signal }, "Shutting down");
    discovery.stop();
    await bridge.stop();
    await go2rtcManager.stop();
    process.exit(0);
  };

  process.on("SIGINT", () => shutdown("SIGINT"));
  process.on("SIGTERM", () => shutdown("SIGTERM"));
}

main().catch((err) => {
  log.fatal({ err }, "Failed to start Matter-ONVIF Bridge");
  process.exit(1);
});
