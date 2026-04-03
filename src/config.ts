import { config } from "dotenv";

config();

export const Config = {
  onvif: {
    username: process.env.ONVIF_USERNAME || "admin",
    password: process.env.ONVIF_PASSWORD || "admin",
    discoveryInterval: parseInt(process.env.ONVIF_DISCOVERY_INTERVAL || "60000", 10),
    /** Comma-separated list of static cameras: "host:port,host:port,..." */
    staticCameras: process.env.ONVIF_STATIC_CAMERAS || "",
  },

  go2rtc: {
    /** "local" = spawn go2rtc as subprocess, "external" = connect to existing go2rtc (e.g. Docker) */
    mode: (process.env.GO2RTC_MODE || "external") as "local" | "external",
    /** Binary path, only used when mode=local */
    path: process.env.GO2RTC_PATH || "./bin/go2rtc",
    /** Hostname or IP where go2rtc API is reachable */
    host: process.env.GO2RTC_HOST || "localhost",
    apiPort: parseInt(process.env.GO2RTC_API_PORT || "1984", 10),
    webrtcPort: parseInt(process.env.GO2RTC_WEBRTC_PORT || "8555", 10),
  },

  matter: {
    port: parseInt(process.env.MATTER_PORT || "5540", 10),
    passcode: parseInt(process.env.MATTER_PASSCODE || "20202021", 10),
    discriminator: parseInt(process.env.MATTER_DISCRIMINATOR || "3840", 10),
    storagePath: process.env.MATTER_STORAGE_PATH || ".matter-storage",
  },

  logLevel: (process.env.LOG_LEVEL || "info") as "trace" | "debug" | "info" | "warn" | "error" | "fatal",
} as const;
