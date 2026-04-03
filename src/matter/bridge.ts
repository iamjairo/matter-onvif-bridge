import { ServerNode, Endpoint, VendorId } from "@matter/main";
import "@matter/nodejs";
import { AggregatorEndpoint, BridgedNodeEndpoint } from "@matter/main/endpoints";
import { ManualPairingCodeCodec } from "@matter/types";
import { createChildLogger } from "../logger.js";
import { Config } from "../config.js";
import type { CameraDevice } from "../registry/camera.js";
import type { CameraRegistry } from "../registry/camera-registry.js";
import { CameraAvStreamManagementServer } from "./clusters/camera-av-stream-management/server.js";
import { WebRtcTransportProviderServer } from "./clusters/webrtc-transport-provider/server.js";
import type { Go2RtcApi } from "../media/go2rtc-api.js";

const log = createChildLogger("matter-bridge");

export class MatterBridge {
  private node: ServerNode | null = null;
  private aggregator: Endpoint | null = null;
  private cameraEndpoints = new Map<string, Endpoint>();

  constructor(
    private readonly registry: CameraRegistry,
    private readonly go2rtcApi: Go2RtcApi,
  ) {}

  async start(): Promise<void> {
    log.info("Starting Matter bridge");

    this.node = await ServerNode.create({
      id: "matter-onvif-bridge",

      basicInformation: {
        vendorName: "matter-onvif-bridge",
        vendorId: VendorId(0xfff1),
        nodeLabel: "ONVIF Camera Bridge",
        productName: "ONVIF Camera Bridge",
        productLabel: "ONVIF Camera Bridge",
        productId: 0x8000,
        serialNumber: "MOB-0001",
        uniqueId: "matter-onvif-bridge-001",
      },

      commissioning: {
        passcode: Config.matter.passcode,
        discriminator: Config.matter.discriminator,
      },

      network: {
        port: Config.matter.port,
      },
    } as any);

    this.aggregator = new Endpoint(AggregatorEndpoint, { id: "aggregator" });
    await this.node.add(this.aggregator);

    // Wire up to camera registry events
    this.registry.on("added", (camera) => this.addCameraEndpoint(camera));
    this.registry.on("removed", (camera) => this.removeCameraEndpoint(camera));
    this.registry.on("updated", (camera) => this.updateCameraEndpoint(camera));

    // Add any cameras already discovered
    for (const camera of this.registry.getAllCameras()) {
      await this.addCameraEndpoint(camera);
    }

    await this.node.start();

    const manualCode = ManualPairingCodeCodec.encode({
      discriminator: Config.matter.discriminator,
      passcode: Config.matter.passcode,
    });

    log.info(
      { port: Config.matter.port, manualPairingCode: manualCode },
      "Matter bridge started — ready for commissioning",
    );
    // Print prominently to stdout
    console.log("\n╔══════════════════════════════════════════════╗");
    console.log("║  Matter ONVIF Camera Bridge                  ║");
    console.log("║                                              ║");
    console.log(`║  Manual pairing code: ${manualCode}      ║`);
    console.log(`║  Passcode:            ${Config.matter.passcode}          ║`);
    console.log(`║  Discriminator:       ${Config.matter.discriminator}              ║`);
    console.log(`║  Port:                ${Config.matter.port}              ║`);
    console.log("║                                              ║");
    console.log("║  Commission via:                             ║");
    console.log("║   • Apple Home → Add Accessory → More opts   ║");
    console.log("║   • Google Home → Add device → Matter        ║");
    console.log("║   • chip-tool pairing onnetwork <code>       ║");
    console.log("╚══════════════════════════════════════════════╝\n");
  }

  async stop(): Promise<void> {
    if (this.node) {
      await this.node.close();
      this.node = null;
      this.aggregator = null;
      this.cameraEndpoints.clear();
    }
    log.info("Matter bridge stopped");
  }

  private async addCameraEndpoint(camera: CameraDevice): Promise<void> {
    if (!this.aggregator) return;
    if (this.cameraEndpoints.has(camera.info.id)) return;

    try {
      // Compose the bridged endpoint with camera behaviors
      const CameraEndpoint = BridgedNodeEndpoint.with(
        CameraAvStreamManagementServer,
        WebRtcTransportProviderServer,
      );

      const profile = camera.info.profiles[0];
      const width = profile?.videoEncoderConfig?.resolution.width || 1920;
      const height = profile?.videoEncoderConfig?.resolution.height || 1080;
      const fps = profile?.videoEncoderConfig?.frameRateLimit || 30;

      const endpoint = new Endpoint(CameraEndpoint, {
        id: `camera-${camera.streamName}`,
        bridgedDeviceBasicInformation: {
          reachable: camera.reachable,
          uniqueId: camera.info.id,
          vendorName: camera.info.manufacturer.slice(0, 32),
          productName: camera.info.model.slice(0, 32),
          nodeLabel: camera.info.name.slice(0, 32),
          serialNumber: camera.info.serialNumber.slice(0, 32),
          softwareVersionString: camera.info.firmwareVersion.slice(0, 64),
          softwareVersion: 1,
          hardwareVersionString: camera.info.hardwareId.slice(0, 64),
          hardwareVersion: 1,
        },
        cameraAvStreamManagement: {
          maxConcurrentVideoEncoders: camera.info.profiles.length || 1,
          maxEncodedPixelRate: width * height * fps,
          videoSensorParams: { sensorWidth: width, sensorHeight: height, hdrCapable: false, maxFps: fps },
          nightVisionCapable: false,
          minViewport: { width: 320, height: 240 },
          microphoneCapabilities: [],
          speakerCapabilities: [],
          twoWayTalkSupport: 0,
          supportedSnapshotParams: [],
          hdrCapable: false,
          maxNetworkBandwidth: 10000,
          currentFrameRate: fps,
          allocatedVideoStreams: [],
          allocatedAudioStreams: [],
          allocatedSnapshotStreams: [],
          rankedVideoStreamPrioritiesList: [],
          softRecordingPrivacyModeEnabled: false,
          softLivestreamPrivacyModeEnabled: false,
          hardPrivacyModeOn: false,
          nightVision: 0,
          nightVisionIllum: 0,
          awbEnabled: true,
          autoShutterSpeedEnabled: true,
          autoIsoEnabled: true,
          viewport: { width, height },
        },
        webRtcTransportProvider: {
          currentSessions: [],
        },
      } as any);

      // Inject camera context so behaviors can access it
      (endpoint as any).__cameraDevice = camera;
      (endpoint as any).__go2rtcApi = this.go2rtcApi;

      await this.aggregator.add(endpoint);
      this.cameraEndpoints.set(camera.info.id, endpoint);

      log.info({ cameraId: camera.info.id, name: camera.info.name }, "Added camera endpoint to bridge");
    } catch (err) {
      log.error({ err, cameraId: camera.info.id }, "Failed to add camera endpoint");
    }
  }

  private async removeCameraEndpoint(camera: CameraDevice): Promise<void> {
    const endpoint = this.cameraEndpoints.get(camera.info.id);
    if (endpoint) {
      await endpoint.close();
      this.cameraEndpoints.delete(camera.info.id);
      log.info({ cameraId: camera.info.id }, "Removed camera endpoint from bridge");
    }
  }

  private async updateCameraEndpoint(camera: CameraDevice): Promise<void> {
    const endpoint = this.cameraEndpoints.get(camera.info.id);
    if (endpoint) {
      try {
        await endpoint.set({
          bridgedDeviceBasicInformation: {
            reachable: camera.reachable,
          },
        } as any);
      } catch (err) {
        log.error({ err, cameraId: camera.info.id }, "Failed to update camera endpoint");
      }
    }
  }
}
