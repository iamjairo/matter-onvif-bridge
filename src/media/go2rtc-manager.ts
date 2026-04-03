import { spawn, type ChildProcess } from "node:child_process";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { createChildLogger } from "../logger.js";
import { Config } from "../config.js";

const log = createChildLogger("go2rtc-manager");

export class Go2RtcManager {
  private process: ChildProcess | null = null;
  private restarting = false;
  private configPath: string;
  private readonly mode: "local" | "external";

  constructor(
    private readonly binaryPath: string = Config.go2rtc.path,
    private readonly host: string = Config.go2rtc.host,
    private readonly apiPort: number = Config.go2rtc.apiPort,
    private readonly webrtcPort: number = Config.go2rtc.webrtcPort,
  ) {
    this.configPath = resolve("go2rtc.yaml");
    this.mode = Config.go2rtc.mode;
  }

  async start(): Promise<void> {
    if (this.mode === "external") {
      log.info({ host: this.host, apiPort: this.apiPort }, "Connecting to external go2rtc (Docker)");
      await this.waitForReady();
      log.info("External go2rtc is ready");
      return;
    }

    await this.writeConfig();
    this.spawnProcess();
    await this.waitForReady();
    log.info({ apiPort: this.apiPort }, "Local go2rtc started");
  }

  async stop(): Promise<void> {
    if (this.mode === "external") {
      // Nothing to stop — Docker Compose manages the container
      return;
    }

    this.restarting = false;
    if (this.process) {
      log.info("Stopping go2rtc");
      this.process.kill("SIGTERM");
      this.process = null;
    }
  }

  private spawnProcess(): void {
    const binPath = resolve(this.binaryPath);
    log.debug({ binary: binPath, config: this.configPath }, "Spawning go2rtc");

    this.process = spawn(binPath, ["-config", this.configPath], {
      stdio: ["ignore", "pipe", "pipe"],
    });

    this.process.stdout?.on("data", (data: Buffer) => {
      const line = data.toString().trim();
      if (line) log.debug({ source: "go2rtc" }, line);
    });

    this.process.stderr?.on("data", (data: Buffer) => {
      const line = data.toString().trim();
      if (line) log.warn({ source: "go2rtc" }, line);
    });

    this.process.on("exit", (code, signal) => {
      log.warn({ code, signal }, "go2rtc exited");
      if (this.restarting) return;
      this.restarting = true;
      setTimeout(() => {
        this.restarting = false;
        log.info("Restarting go2rtc");
        this.spawnProcess();
      }, 3000);
    });

    this.process.on("error", (err) => {
      log.error({ err }, "go2rtc process error");
    });
  }

  private async writeConfig(): Promise<void> {
    const yaml = [
      `api:`,
      `  listen: ":${this.apiPort}"`,
      `webrtc:`,
      `  listen: ":${this.webrtcPort}"`,
      `streams: {}`,
    ].join("\n");

    await writeFile(this.configPath, yaml, "utf-8");
    log.debug({ path: this.configPath }, "Wrote go2rtc config");
  }

  private async waitForReady(timeout = 15000): Promise<void> {
    const url = `http://${this.host}:${this.apiPort}/api`;
    const start = Date.now();
    while (Date.now() - start < timeout) {
      try {
        const res = await fetch(url);
        if (res.ok) return;
      } catch {
        // Not ready yet
      }
      await new Promise((r) => setTimeout(r, 500));
    }
    throw new Error(`go2rtc at ${url} not ready within ${timeout}ms`);
  }
}
