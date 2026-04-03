import onvif from "onvif";
const { Discovery } = onvif;
import { createChildLogger } from "../logger.js";
import { Config } from "../config.js";
import { OnvifClient } from "./client.js";
import type { OnvifCameraInfo } from "./types.js";

const log = createChildLogger("onvif-discovery");

export interface DiscoveryEvents {
  cameraFound: (camera: OnvifCameraInfo) => void;
  cameraLost: (cameraId: string) => void;
  error: (error: Error) => void;
}

interface DeviceAddress {
  hostname: string;
  port: number;
}

export class OnvifDiscovery {
  private discoveredCameras = new Map<string, OnvifCameraInfo>();
  private scanTimer: ReturnType<typeof setInterval> | null = null;
  private listeners: Partial<{ [K in keyof DiscoveryEvents]: DiscoveryEvents[K][] }> = {};

  constructor(
    private readonly username: string = Config.onvif.username,
    private readonly password: string = Config.onvif.password,
    private readonly scanInterval: number = Config.onvif.discoveryInterval,
  ) {}

  on<K extends keyof DiscoveryEvents>(event: K, listener: DiscoveryEvents[K]): void {
    if (!this.listeners[event]) {
      this.listeners[event] = [];
    }
    this.listeners[event]!.push(listener);
  }

  private emit<K extends keyof DiscoveryEvents>(event: K, ...args: Parameters<DiscoveryEvents[K]>): void {
    const handlers = this.listeners[event];
    if (handlers) {
      for (const handler of handlers) {
        (handler as (...args: any[]) => void)(...args);
      }
    }
  }

  async start(): Promise<void> {
    log.info({ interval: this.scanInterval }, "Starting ONVIF discovery");
    await this.scan();
    this.scanTimer = setInterval(() => this.scan(), this.scanInterval);
  }

  stop(): void {
    if (this.scanTimer) {
      clearInterval(this.scanTimer);
      this.scanTimer = null;
    }
    log.info("Stopped ONVIF discovery");
  }

  getDiscoveredCameras(): OnvifCameraInfo[] {
    return Array.from(this.discoveredCameras.values());
  }

  private async scan(): Promise<void> {
    try {
      // Merge WS-Discovery results with static camera list
      const probed = await this.probe();
      const staticDevices = this.parseStaticCameras();
      const devices = this.mergeDevices(probed, staticDevices);

      log.info(
        {
          probed: probed.length,
          static: staticDevices.length,
          total: devices.length,
          devices: devices.map((d) => `${d.hostname}:${d.port}`),
        },
        "ONVIF devices to connect",
      );

      const foundIds = new Set<string>();

      // Connect sequentially — concurrent ONVIF SOAP connections can overwhelm cameras
      for (const device of devices) {
        try {
          const camera = await this.connectToDevice(device);
          foundIds.add(camera.id);
          if (!this.discoveredCameras.has(camera.id)) {
            this.discoveredCameras.set(camera.id, camera);
            this.emit("cameraFound", camera);
          }
        } catch (err) {
          const msg = (err as Error)?.message || String(err);
          if (msg.includes("ECONNREFUSED")) {
            log.debug({ host: device.hostname, port: device.port }, "Device refused connection (not an ONVIF camera?)");
          } else if (msg.includes("SOAP Fault")) {
            log.warn({ host: device.hostname, port: device.port }, "ONVIF auth failed — check credentials, or device is not a camera");
          } else if (msg.includes("ETIMEDOUT") || msg.includes("EHOSTUNREACH")) {
            log.debug({ host: device.hostname, port: device.port }, "Device unreachable");
          } else {
            log.warn({ host: device.hostname, port: device.port, err: msg }, "Failed to connect to ONVIF device");
          }
        }
      }

      // Detect cameras that disappeared
      for (const [id] of this.discoveredCameras) {
        if (!foundIds.has(id)) {
          this.discoveredCameras.delete(id);
          this.emit("cameraLost", id);
          log.info({ cameraId: id }, "Camera went offline");
        }
      }

      log.info(
        { connected: foundIds.size, total: this.discoveredCameras.size },
        "ONVIF scan complete",
      );
    } catch (err) {
      log.error({ err }, "ONVIF scan failed");
      this.emit("error", err instanceof Error ? err : new Error(String(err)));
    }
  }

  private probe(): Promise<DeviceAddress[]> {
    return new Promise((resolve, reject) => {
      Discovery.probe({ timeout: 5000 }, (err: Error | null, cams: any[]) => {
        if (err) {
          reject(err);
          return;
        }

        const devices: DeviceAddress[] = (cams || []).map((cam: any) => {
          // Extract port from the XAddr URL if available
          let port = 80;
          const hostname = cam.hostname || cam.address;

          // The onvif library populates probeMatch.XAddrs with the service URL
          const xaddr: string | undefined =
            cam.probeMatch?.XAddrs || cam.xml?.Body?.ProbeMatches?.ProbeMatch?.XAddrs;
          if (xaddr) {
            try {
              const url = new URL(xaddr.split(" ")[0]); // XAddrs can have multiple URLs separated by space
              port = parseInt(url.port, 10) || 80;
            } catch {
              // Fall back to default
            }
          }

          // Also check if the cam object itself has a port
          if (cam.port && cam.port !== 80) {
            port = cam.port;
          }

          return { hostname, port };
        });

        log.debug({ count: devices.length }, "WS-Discovery probe complete");
        resolve(devices);
      });
    });
  }

  /** Parse ONVIF_STATIC_CAMERAS env var: "host:port,host:port,..." */
  private parseStaticCameras(): DeviceAddress[] {
    const raw = Config.onvif.staticCameras;
    if (!raw) return [];

    return raw
      .split(",")
      .map((entry) => entry.trim())
      .filter(Boolean)
      .map((entry) => {
        const [hostname, portStr] = entry.split(":");
        return {
          hostname,
          port: parseInt(portStr || "80", 10),
        };
      });
  }

  /** Merge probed and static devices, deduplicating by hostname */
  private mergeDevices(probed: DeviceAddress[], staticDevices: DeviceAddress[]): DeviceAddress[] {
    const seen = new Map<string, DeviceAddress>();

    // Static cameras take priority (user-provided ports are more reliable)
    for (const device of staticDevices) {
      seen.set(device.hostname, device);
    }
    for (const device of probed) {
      if (!seen.has(device.hostname)) {
        seen.set(device.hostname, device);
      }
    }

    return Array.from(seen.values());
  }

  private async connectToDevice(device: DeviceAddress): Promise<OnvifCameraInfo> {
    const client = new OnvifClient(device.hostname, device.port, this.username, this.password);
    const info = await client.connect();
    client.disconnect();
    return info;
  }
}
