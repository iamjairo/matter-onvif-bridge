/**
 * WebRTC Transport Provider Server Behavior
 *
 * Implements the server-side behavior for WebRTC-based streaming.
 * This is the critical path: it bridges Matter ProvideOffer commands
 * to go2rtc's WebRTC SDP exchange API.
 */
import { WebRtcTransportProviderBehavior, WebRtcTransportProvider } from "./cluster.js";
import { Go2RtcApi } from "../../../media/go2rtc-api.js";
import { WebRtcSession } from "../../../media/webrtc-session.js";
import { createChildLogger } from "../../../logger.js";

const log = createChildLogger("webrtc-provider-server");

/** Map of active WebRTC sessions by session ID */
const activeSessions = new Map<number, WebRtcSession>();

class WebRtcTransportProviderServerBase extends WebRtcTransportProviderBehavior {
  declare state: any;

  override initialize() {
    // State initialized from endpoint configuration
  }

  /**
   * Handle ProvideOffer command — the critical streaming path.
   *
   * 1. Receive SDP offer from the Matter controller
   * 2. Forward to go2rtc via POST /api/webrtc
   * 3. go2rtc returns SDP answer with ICE candidates embedded
   * 4. Return SDP answer to the controller
   */
  async provideOffer(request: any): Promise<any> {
    const streamName = this.getStreamName();
    const api = this.getGo2RtcApi();

    log.info({ streamName, streamUsage: request.streamUsage }, "Received ProvideOffer");

    const session = new WebRtcSession(streamName, api);
    activeSessions.set(session.id, session);

    try {
      const { sdpAnswer } = await session.negotiate(request.sdp);

      const sessionEntry = {
        id: session.id,
        peerNodeId: 0,
        peerFabricIndex: 0,
        streamUsage: request.streamUsage,
        videoStreamId: request.videoStreamId ?? undefined,
        audioStreamId: request.audioStreamId ?? undefined,
      };

      const sessions = [...((this.state as any).currentSessions || []), sessionEntry];
      (this.state as any).currentSessions = sessions;

      log.info({ sessionId: session.id, streamName }, "WebRTC session established");

      return {
        webRtcSessionId: session.id,
        sdp: sdpAnswer,
        videoStreamId: request.videoStreamId ?? undefined,
        audioStreamId: request.audioStreamId ?? undefined,
      };
    } catch (err) {
      session.close();
      activeSessions.delete(session.id);
      log.error({ err, streamName }, "ProvideOffer failed");
      throw err;
    }
  }

  solicitOffer(_request: any): any {
    throw new Error("SolicitOffer not implemented");
  }

  provideAnswer(_request: any): void {
    throw new Error("ProvideAnswer not implemented (SolicitOffer flow)");
  }

  provideIceCandidates(request: any): void {
    const session = activeSessions.get(request.webRtcSessionId);
    if (!session) {
      log.warn({ sessionId: request.webRtcSessionId }, "ICE candidates for unknown session");
      return;
    }
    session.sendIceCandidates(request.iceCandidates);
    log.debug(
      { sessionId: request.webRtcSessionId, count: request.iceCandidates.length },
      "Forwarded ICE candidates to go2rtc",
    );
  }

  endSession(request: any): void {
    const session = activeSessions.get(request.webRtcSessionId);
    if (session) {
      session.close();
      activeSessions.delete(request.webRtcSessionId);
    }

    const sessions = ((this.state as any).currentSessions || []).filter(
      (s: any) => s.id !== request.webRtcSessionId,
    );
    (this.state as any).currentSessions = sessions;

    log.info({ sessionId: request.webRtcSessionId, reason: request.reason }, "WebRTC session ended");
  }

  private getStreamName(): string {
    const camera = (this.endpoint as any).__cameraDevice;
    if (camera?.streamName) return camera.streamName;
    const id = this.endpoint.id;
    return typeof id === "string" ? id.replace("camera-", "") : `camera-${id}`;
  }

  private getGo2RtcApi(): Go2RtcApi {
    const api = (this.endpoint as any).__go2rtcApi;
    if (api) return api;
    return new Go2RtcApi();
  }
}

// Fix featureMap (same issue as CameraAvStreamManagement)
const OrigWrtpState = WebRtcTransportProviderServerBase.State;
const FixedWrtpState = class extends OrigWrtpState {
  constructor() {
    super();
    (this as any).featureMap = {};
  }
};
Object.defineProperty(WebRtcTransportProviderServerBase, "State", { value: FixedWrtpState });

// Map command handlers to numeric keys for ValidatedElements
const commands = WebRtcTransportProvider.commands;
const proto = WebRtcTransportProviderServerBase.prototype as any;
const methodMap: Record<string, string> = {
  ProvideOffer: "provideOffer",
  SolicitOffer: "solicitOffer",
  ProvideAnswer: "provideAnswer",
  ProvideIceCandidates: "provideIceCandidates",
  EndSession: "endSession",
};
for (const key of Object.keys(commands)) {
  const cmd = commands[key];
  if (!cmd) continue;
  const method = methodMap[cmd.name];
  if (method && proto[method]) {
    proto[key] = proto[method];
  } else if (cmd.name.endsWith("Response")) {
    proto[key] = function () {};
  }
}

export const WebRtcTransportProviderServer = WebRtcTransportProviderServerBase;
