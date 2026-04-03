import pino from "pino";
import { Config } from "./config.js";

export const logger = pino({
  level: Config.logLevel,
  transport: {
    target: "pino/file",
    options: { destination: 1 }, // stdout
  },
});

export function createChildLogger(name: string) {
  return logger.child({ module: name });
}
