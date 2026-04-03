export interface OnvifProfile {
  name: string;
  token: string;
  videoEncoderConfig?: {
    encoding: string; // H264, H265, JPEG, MPEG4
    resolution: { width: number; height: number };
    frameRateLimit: number;
    bitrateLimit: number;
    quality: number;
  };
  audioEncoderConfig?: {
    encoding: string; // G711, G726, AAC
    bitrate: number;
    sampleRate: number;
  };
  ptzConfig?: {
    nodeToken: string;
  };
}

export interface OnvifStreamUri {
  uri: string;
  protocol: string;
}

export interface OnvifCameraInfo {
  /** Unique identifier derived from ONVIF device info */
  id: string;
  /** Human-readable name */
  name: string;
  /** IP address or hostname */
  hostname: string;
  /** ONVIF service port */
  port: number;
  /** Device manufacturer */
  manufacturer: string;
  /** Device model */
  model: string;
  /** Firmware version */
  firmwareVersion: string;
  /** Serial number */
  serialNumber: string;
  /** Hardware ID */
  hardwareId: string;
  /** Available media profiles */
  profiles: OnvifProfile[];
  /** Primary RTSP stream URI */
  streamUri: string;
  /** Whether camera supports PTZ */
  hasPtz: boolean;
}
