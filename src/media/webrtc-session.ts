import type WebSocket from "ws";
import { createChildLogger } from "../logger.js";
import { Go2RtcApi } from "./go2rtc-api.js";

const log = createChildLogger("webrtc-session");

let nextSessionId = 1;

export class WebRtcSession {
  readonly id: number;
  private ws: WebSocket | null = null;
  private closed = false;

  constructor(
    readonly streamName: string,
    private readonly api: Go2RtcApi,
  ) {
    this.id = nextSessionId++;
  }

  /** Exchange SDP offer/answer and set up ICE candidate forwarding */
  async negotiate(sdpOffer: string): Promise<{
    sdpAnswer: string;
    iceCandidates: string[];
  }> {
    log.debug({ sessionId: this.id, streamName: this.streamName }, "Starting WebRTC negotiation");

    // go2rtc typically embeds ICE candidates in the SDP answer,
    // so we may not need trickle ICE at all
    const sdpAnswer = await this.api.exchangeSdp(this.streamName, sdpOffer);

    // Extract ICE candidates from the SDP answer
    const iceCandidates = this.extractIceCandidates(sdpAnswer);

    log.info(
      { sessionId: this.id, candidates: iceCandidates.length },
      "WebRTC negotiation complete",
    );

    return { sdpAnswer, iceCandidates };
  }

  /** Send ICE candidates from the controller to go2rtc */
  sendIceCandidates(candidates: string[]): void {
    if (!this.ws) {
      // Open WebSocket for ICE exchange if not already open
      this.ws = this.api.openIceWebSocket(this.streamName, (candidate) => {
        log.debug({ sessionId: this.id, candidate }, "Received ICE candidate from go2rtc");
      });
    }

    for (const candidate of candidates) {
      this.api.sendIceCandidate(this.ws, candidate);
    }
  }

  /** Close the session */
  close(): void {
    if (this.closed) return;
    this.closed = true;

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    log.info({ sessionId: this.id }, "WebRTC session closed");
  }

  private extractIceCandidates(sdp: string): string[] {
    const candidates: string[] = [];
    for (const line of sdp.split("\n")) {
      if (line.startsWith("a=candidate:")) {
        candidates.push(line.trim());
      }
    }
    return candidates;
  }
}
