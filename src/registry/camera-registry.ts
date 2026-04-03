import { createChildLogger } from "../logger.js";
import type { OnvifCameraInfo } from "../onvif/types.js";
import { type CameraDevice, createCameraDevice } from "./camera.js";

const log = createChildLogger("camera-registry");

export interface RegistryEvents {
  added: (camera: CameraDevice) => void;
  removed: (camera: CameraDevice) => void;
  updated: (camera: CameraDevice) => void;
}

export class CameraRegistry {
  private cameras = new Map<string, CameraDevice>();
  private listeners: Partial<{ [K in keyof RegistryEvents]: RegistryEvents[K][] }> = {};

  on<K extends keyof RegistryEvents>(event: K, listener: RegistryEvents[K]): void {
    if (!this.listeners[event]) {
      this.listeners[event] = [];
    }
    this.listeners[event]!.push(listener);
  }

  private emit<K extends keyof RegistryEvents>(event: K, ...args: Parameters<RegistryEvents[K]>): void {
    const handlers = this.listeners[event];
    if (handlers) {
      for (const handler of handlers) {
        (handler as (...args: any[]) => void)(...args);
      }
    }
  }

  addCamera(info: OnvifCameraInfo): CameraDevice {
    const existing = this.cameras.get(info.id);
    if (existing) {
      // Update existing camera info
      existing.info = info;
      existing.reachable = true;
      this.emit("updated", existing);
      log.debug({ cameraId: info.id }, "Updated existing camera");
      return existing;
    }

    const camera = createCameraDevice(info);
    this.cameras.set(info.id, camera);
    this.emit("added", camera);
    log.info({ cameraId: info.id, name: info.name }, "Camera added to registry");
    return camera;
  }

  removeCamera(cameraId: string): void {
    const camera = this.cameras.get(cameraId);
    if (camera) {
      camera.reachable = false;
      this.cameras.delete(cameraId);
      this.emit("removed", camera);
      log.info({ cameraId, name: camera.info.name }, "Camera removed from registry");
    }
  }

  markUnreachable(cameraId: string): void {
    const camera = this.cameras.get(cameraId);
    if (camera && camera.reachable) {
      camera.reachable = false;
      this.emit("updated", camera);
      log.warn({ cameraId }, "Camera marked unreachable");
    }
  }

  getCamera(cameraId: string): CameraDevice | undefined {
    return this.cameras.get(cameraId);
  }

  getCameraByStreamName(streamName: string): CameraDevice | undefined {
    for (const camera of this.cameras.values()) {
      if (camera.streamName === streamName) return camera;
    }
    return undefined;
  }

  getAllCameras(): CameraDevice[] {
    return Array.from(this.cameras.values());
  }

  get size(): number {
    return this.cameras.size;
  }
}
