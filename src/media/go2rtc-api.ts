import WebSocket from "ws";
import { createChildLogger } from "../logger.js";
import { Config } from "../config.js";

const log = createChildLogger("go2rtc-api");

export class Go2RtcApi {
  private readonly baseUrl: string;
  private readonly wsHost: string;
  private readonly wsPort: number;

  constructor(
    host: string = Config.go2rtc.host,
    port: number = Config.go2rtc.apiPort,
  ) {
    this.baseUrl = `http://${host}:${port}`;
    this.wsHost = host;
    this.wsPort = port;
  }

  /** Add an RTSP stream source to go2rtc (v1.9+ API) */
  async addStream(name: string, rtspUrl: string): Promise<void> {
    const url = `${this.baseUrl}/api/streams?name=${encodeURIComponent(name)}`;
    const res = await fetch(url, {
      method: "PUT",
      body: rtspUrl,
    });
    if (!res.ok) {
      const body = await res.text().catch(() => "");
      throw new Error(`Failed to add stream ${name}: ${res.status} ${body}`);
    }
    log.info({ name, rtspUrl }, "Added stream to go2rtc");
  }

  /** Remove a stream from go2rtc */
  async removeStream(name: string): Promise<void> {
    const url = `${this.baseUrl}/api/streams?name=${encodeURIComponent(name)}`;
    const res = await fetch(url, { method: "DELETE" });
    if (!res.ok && res.status !== 404) {
      throw new Error(`Failed to remove stream ${name}: ${res.status} ${res.statusText}`);
    }
    log.info({ name }, "Removed stream from go2rtc");
  }

  /** Exchange WebRTC SDP offer for answer via go2rtc */
  async exchangeSdp(streamName: string, sdpOffer: string): Promise<string> {
    const url = `${this.baseUrl}/api/webrtc?src=${encodeURIComponent(streamName)}`;
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/sdp" },
      body: sdpOffer,
    });

    if (!res.ok) {
      throw new Error(`SDP exchange failed for ${streamName}: ${res.status} ${res.statusText}`);
    }

    const sdpAnswer = await res.text();
    log.debug({ streamName, answerLength: sdpAnswer.length }, "SDP exchange complete");
    return sdpAnswer;
  }

  /** Open a WebSocket for ICE candidate exchange */
  openIceWebSocket(
    streamName: string,
    onCandidate: (candidate: string) => void,
  ): WebSocket {
    const wsUrl = `ws://${this.wsHost}:${this.wsPort}/api/ws?src=${encodeURIComponent(streamName)}`;
    const ws = new WebSocket(wsUrl);

    ws.on("message", (data: Buffer) => {
      try {
        const msg = JSON.parse(data.toString());
        if (msg.type === "webrtc/candidate" && msg.value) {
          onCandidate(msg.value);
        }
      } catch (err) {
        log.warn({ err }, "Failed to parse go2rtc WebSocket message");
      }
    });

    ws.on("error", (err) => {
      log.error({ err, streamName }, "go2rtc WebSocket error");
    });

    return ws;
  }

  /** Send an ICE candidate to go2rtc via WebSocket */
  sendIceCandidate(ws: WebSocket, candidate: string): void {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "webrtc/candidate", value: candidate }));
    }
  }

  /** List all current streams */
  async listStreams(): Promise<Record<string, unknown>> {
    const res = await fetch(`${this.baseUrl}/api/streams`);
    if (!res.ok) {
      throw new Error(`Failed to list streams: ${res.status}`);
    }
    return res.json() as Promise<Record<string, unknown>>;
  }
}
