import onvif from "onvif";
const { Cam } = onvif;
import { createChildLogger } from "../logger.js";
import type { OnvifCameraInfo, OnvifProfile, OnvifStreamUri } from "./types.js";

const log = createChildLogger("onvif-client");

/** Safely convert any ONVIF XML-parsed value to a flat string */
function stringify(val: unknown): string {
  if (val == null) return "";
  if (typeof val === "string") return val;
  if (typeof val === "number") return String(val);
  // xml2js can return { _: "value", $: { ... } } for text nodes
  if (typeof val === "object" && "_" in (val as any)) return String((val as any)._);
  return "";
}

export class OnvifClient {
  private cam: InstanceType<typeof Cam> | null = null;

  constructor(
    private readonly hostname: string,
    private readonly port: number,
    private readonly username: string,
    private readonly password: string,
  ) {}

  async connect(): Promise<OnvifCameraInfo> {
    this.cam = await this.createCam();

    const deviceInfo = await this.getDeviceInfo();
    const profiles = await this.getProfiles();
    const streamUri = await this.getStreamUri(profiles[0]?.token);

    const info: OnvifCameraInfo = {
      id: this.deriveId(deviceInfo),
      name: deviceInfo.model || `Camera ${this.hostname}`,
      hostname: this.hostname,
      port: this.port,
      manufacturer: deviceInfo.manufacturer || "Unknown",
      model: deviceInfo.model || "Unknown",
      firmwareVersion: deviceInfo.firmwareVersion || "Unknown",
      serialNumber: deviceInfo.serialNumber || "Unknown",
      hardwareId: deviceInfo.hardwareId || "Unknown",
      profiles,
      streamUri: streamUri?.uri || `rtsp://${this.hostname}:554/stream1`,
      hasPtz: profiles.some((p) => p.ptzConfig != null),
    };

    log.info({ camera: info.name, host: this.hostname, profiles: profiles.length }, "Connected to ONVIF camera");
    return info;
  }

  disconnect(): void {
    this.cam = null;
  }

  private createCam(): Promise<InstanceType<typeof Cam>> {
    return new Promise((resolve, reject) => {
      const cam = new Cam(
        {
          hostname: this.hostname,
          port: this.port,
          username: this.username,
          password: this.password,
          timeout: 10000,
          preserveAddress: true, // Prevent VIGI cameras from redirecting to wrong internal URLs
        },
        (err: Error | null) => {
          if (err) {
            log.error({ err, host: this.hostname, port: this.port }, "Failed to connect to ONVIF camera");
            reject(err);
          } else {
            log.debug(
              { host: this.hostname, timeShift: (cam as any).timeShift },
              "ONVIF connection established",
            );
            resolve(cam);
          }
        },
      );
    });
  }

  private getDeviceInfo(): Promise<{
    manufacturer: string;
    model: string;
    firmwareVersion: string;
    serialNumber: string;
    hardwareId: string;
  }> {
    return new Promise((resolve, reject) => {
      this.cam!.getDeviceInformation((err: Error | null, info: any) => {
        if (err) reject(err);
        else
          resolve({
            manufacturer: stringify(info?.manufacturer),
            model: stringify(info?.model),
            firmwareVersion: stringify(info?.firmwareVersion),
            serialNumber: stringify(info?.serialNumber),
            hardwareId: stringify(info?.hardwareId),
          });
      });
    });
  }

  private getProfiles(): Promise<OnvifProfile[]> {
    return new Promise((resolve, reject) => {
      this.cam!.getProfiles((err: Error | null, profiles: any[]) => {
        if (err) {
          reject(err);
          return;
        }

        const result: OnvifProfile[] = (profiles || []).map((p: any) => ({
          name: p.name || p.$.token,
          token: p.$.token,
          videoEncoderConfig: p.videoEncoderConfiguration
            ? {
                encoding: p.videoEncoderConfiguration.encoding,
                resolution: {
                  width: parseInt(p.videoEncoderConfiguration.resolution?.width || "1920", 10),
                  height: parseInt(p.videoEncoderConfiguration.resolution?.height || "1080", 10),
                },
                frameRateLimit: parseInt(p.videoEncoderConfiguration.rateControl?.frameRateLimit || "30", 10),
                bitrateLimit: parseInt(p.videoEncoderConfiguration.rateControl?.bitrateLimit || "4096", 10),
                quality: parseFloat(p.videoEncoderConfiguration.quality || "5"),
              }
            : undefined,
          audioEncoderConfig: p.audioEncoderConfiguration
            ? {
                encoding: p.audioEncoderConfiguration.encoding,
                bitrate: parseInt(p.audioEncoderConfiguration.bitrate || "64", 10),
                sampleRate: parseInt(p.audioEncoderConfiguration.sampleRate || "8000", 10),
              }
            : undefined,
          ptzConfig: p.PTZConfiguration
            ? {
                nodeToken: p.PTZConfiguration.nodeToken,
              }
            : undefined,
        }));

        resolve(result);
      });
    });
  }

  private getStreamUri(profileToken?: string): Promise<OnvifStreamUri | null> {
    if (!profileToken) return Promise.resolve(null);

    return new Promise((resolve, reject) => {
      this.cam!.getStreamUri(
        { protocol: "RTSP", profileToken },
        (err: Error | null, stream: any) => {
          if (err) {
            log.warn({ err, profileToken }, "Failed to get stream URI, using fallback");
            resolve(null);
          } else {
            resolve({
              uri: stream?.uri || "",
              protocol: "RTSP",
            });
          }
        },
      );
    });
  }

  private deriveId(deviceInfo: { serialNumber: string; manufacturer: string; model: string }): string {
    const serial = String(deviceInfo.serialNumber || "");
    if (serial && serial !== "Unknown" && serial !== "undefined" && serial !== "[object Object]") {
      return serial;
    }
    // Fallback: use hostname + port as ID
    return `${this.hostname}:${this.port}`;
  }
}
