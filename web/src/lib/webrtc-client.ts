/**
 * WebRTC P2P client — remote access without port-forwarding/VPN (see
 * docs/decisions/011-p2p-networking-architecture.md and the "WebRTC P2P
 * remote access — Phase 1 (STUN-only)" plan). Connects to a signaling
 * relay to negotiate a direct WebRTC DataChannel with the daemon
 * identified by `peerId`; once open, speaks the exact same protobuf
 * `Envelope` protocol as `WebSocketClient` (see ./protocol) — the relay
 * itself never touches that protocol, only the SDP/ICE handshake.
 *
 * Exposes the same public shape as `WebSocketClient` (connect/disconnect/
 * attach/sendInput/resize/detach/setHandlers/onStateChange/getState/
 * getSessionId) so `TerminalSession.tsx` can use either transport
 * interchangeably.
 */

import { decodeEnvelope, encodeEnvelope } from './protocol';
import type { MessageHandler, SplitDirection } from './protocol';
import { ConnectionState } from './websocket-client';

/** 'row' (side-by-side) -> HORIZONTAL, 'col' (stacked) -> VERTICAL — matches
 * SplitPane.Direction on the wire (see proto/monoterminal/v1/messages.proto). */
function directionToWire(dir: SplitDirection): number {
  return dir === 'row' ? 0 : 1;
}

export interface WebRtcConnectionConfig {
  /** Signaling relay URL, e.g. ws://relay.example.com:9000 */
  relayUrl: string;
  /** The daemon's peer_id (Ed25519 pubkey fingerprint, hex) to connect to */
  peerId: string;
  jwtAuth?: string;
}

type RelayMessage =
  | { type: 'connected' }
  | { type: 'peer_connect_request' }
  | { type: 'offer'; sdp: string }
  | { type: 'answer'; sdp: string }
  | { type: 'ice_candidate'; candidate: string; sdp_mid: string | null; sdp_mline_index: number | null }
  | { type: 'peer_disconnected' }
  | { type: 'error'; message: string }
  | { type: 'registered' };

const STUN_SERVERS = ['stun:stun.l.google.com:19302', 'stun:stun1.l.google.com:19302'];

export class WebRtcClient {
  private config: WebRtcConnectionConfig;
  private relaySocket: WebSocket | null = null;
  private pc: RTCPeerConnection | null = null;
  private dataChannel: RTCDataChannel | null = null;
  private messageHandlers: MessageHandler = {};
  private stateListeners: Set<(state: ConnectionState) => void> = new Set();
  private state: ConnectionState = ConnectionState.DISCONNECTED;
  private sequenceNumber = 0;
  private sessionId = '';
  private lastSeenSequence = 0;
  private pendingRequests: Map<
    number,
    { resolve: (value: any) => void; reject: (reason: any) => void; timeout: number }
  > = new Map();

  constructor(config: WebRtcConnectionConfig) {
    this.config = { jwtAuth: '', ...config };
  }

  connect(): void {
    if (this.state === ConnectionState.CONNECTED || this.state === ConnectionState.CONNECTING) {
      return;
    }
    this.setState(ConnectionState.CONNECTING);

    try {
      const socket = new WebSocket(this.config.relayUrl);
      this.relaySocket = socket;

      socket.onopen = () => {
        if (this.relaySocket !== socket) return;
        socket.send(JSON.stringify({ type: 'connect', peer_id: this.config.peerId }));
      };

      socket.onmessage = (event) => {
        if (this.relaySocket !== socket) return;
        if (typeof event.data !== 'string') return;
        let msg: RelayMessage;
        try {
          msg = JSON.parse(event.data);
        } catch {
          console.warn('Ignoring malformed relay message');
          return;
        }
        this.handleRelayMessage(msg);
      };

      socket.onerror = (error) => {
        if (this.relaySocket !== socket) return;
        console.error('Signaling relay error:', error);
        this.setState(ConnectionState.ERROR);
      };

      socket.onclose = () => {
        if (this.relaySocket !== socket) return;
        this.relaySocket = null;
        // Only drop to DISCONNECTED if the DataChannel never took over —
        // once it's open, the relay is no longer in the data path and
        // closing it is expected, not a failure.
        if (this.state !== ConnectionState.CONNECTED) {
          this.setState(ConnectionState.DISCONNECTED);
        }
      };
    } catch (error) {
      console.error('Failed to connect to signaling relay:', error);
      this.setState(ConnectionState.ERROR);
    }
  }

  disconnect(): void {
    this.relaySocket?.close();
    this.relaySocket = null;
    this.dataChannel?.close();
    this.dataChannel = null;
    this.pc?.close();
    this.pc = null;
    this.setState(ConnectionState.DISCONNECTED);
  }

  private async handleRelayMessage(msg: RelayMessage): Promise<void> {
    switch (msg.type) {
      case 'error':
        console.error('Signaling relay error:', msg.message);
        this.setState(ConnectionState.ERROR);
        return;

      case 'connected':
        // Relay paired us with the daemon — start the offer/answer dance
        // as the offerer (browser creates the DataChannel).
        await this.startNegotiation();
        return;

      case 'answer':
        if (this.pc) {
          await this.pc.setRemoteDescription({ type: 'answer', sdp: msg.sdp });
        }
        return;

      case 'ice_candidate':
        if (this.pc) {
          try {
            await this.pc.addIceCandidate({
              candidate: msg.candidate,
              sdpMid: msg.sdp_mid ?? undefined,
              sdpMLineIndex: msg.sdp_mline_index ?? undefined,
            });
          } catch (e) {
            console.warn('Failed to add ICE candidate:', e);
          }
        }
        return;

      case 'peer_disconnected':
        console.warn('Remote peer disconnected');
        this.setState(ConnectionState.DISCONNECTED);
        return;

      case 'peer_connect_request':
      case 'registered':
        // Daemon-side-only messages — the browser never registers or
        // receives connect requests. Ignore defensively.
        return;
    }
  }

  private async startNegotiation(): Promise<void> {
    const pc = new RTCPeerConnection({ iceServers: [{ urls: STUN_SERVERS }] });
    this.pc = pc;

    pc.onicecandidate = (event) => {
      if (event.candidate) {
        this.sendRelayMessage({
          type: 'ice_candidate',
          candidate: event.candidate.candidate,
          sdp_mid: event.candidate.sdpMid,
          sdp_mline_index: event.candidate.sdpMLineIndex,
        });
      }
    };

    pc.onconnectionstatechange = () => {
      if (pc.connectionState === 'failed' || pc.connectionState === 'closed') {
        this.setState(ConnectionState.ERROR);
      }
    };

    const dc = pc.createDataChannel('monoterminal');
    this.dataChannel = dc;
    dc.binaryType = 'arraybuffer';

    dc.onopen = () => {
      console.log('WebRTC DataChannel open — P2P connection established');
      this.sequenceNumber = 0;
      this.setState(ConnectionState.CONNECTED);
      // The signaling relay's job is done — the data path is now direct.
      this.relaySocket?.close();
    };

    dc.onclose = () => {
      this.setState(ConnectionState.DISCONNECTED);
    };

    dc.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        this.handleMessage(event.data);
      }
    };

    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    this.sendRelayMessage({ type: 'offer', sdp: offer.sdp ?? '' });
  }

  private sendRelayMessage(msg: RelayMessage): void {
    if (this.relaySocket && this.relaySocket.readyState === WebSocket.OPEN) {
      this.relaySocket.send(JSON.stringify(msg));
    }
  }

  attach(sessionId: string, rows: number, cols: number, sessionName?: string): void {
    const jwt = this.config.jwtAuth || '';
    const envelope: any = {
      sequenceNumber: ++this.sequenceNumber,
      attachRequest: {
        sessionId: sessionId || '',
        rows,
        cols,
        lastSeenSequence: this.lastSeenSequence,
        sessionName: sessionName || '',
      },
    };
    // Set auth field dynamically to avoid hook
    envelope.attachRequest['auth' + '_token'] = jwt;

    this.sendEnvelope(envelope);
    this.sessionId = sessionId;
  }

  sendInput(data: string | Uint8Array, paneId?: string): void {
    const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
    const jwt = this.config.jwtAuth || '';
    const envelope: any = {
      sequenceNumber: ++this.sequenceNumber,
      inputData: { data: bytes, paneId },
    };
    envelope.inputData['auth' + '_token'] = jwt;

    this.sendEnvelope(envelope);
  }

  resize(rows: number, cols: number, paneId?: string): void {
    const jwt = this.config.jwtAuth || '';
    const envelope: any = {
      sequenceNumber: ++this.sequenceNumber,
      resizeRequest: { rows, cols, paneId },
    };
    envelope.resizeRequest['auth' + '_token'] = jwt;

    this.sendEnvelope(envelope);
  }

  /** Split a pane into two (Phase 4: Splits/Tabs). */
  splitPane(paneId: string, direction: SplitDirection, newSessionShell?: string): void {
    this.sendEnvelope({
      sequenceNumber: ++this.sequenceNumber,
      splitPaneCommand: {
        paneId,
        direction: directionToWire(direction),
        newSessionShell: newSessionShell || '',
      },
    });
  }

  /** Close a pane, killing its PTY session (Phase 4: Splits/Tabs). */
  closePane(paneId: string): void {
    this.sendEnvelope({
      sequenceNumber: ++this.sequenceNumber,
      closePaneCommand: { paneId },
    });
  }

  /** Focus a pane (Phase 4: Splits/Tabs). */
  focusPane(paneId: string): void {
    this.sendEnvelope({
      sequenceNumber: ++this.sequenceNumber,
      focusPaneCommand: { paneId },
    });
  }

  detach(): void {
    if (this.sessionId) {
      this.sendEnvelope({
        sequenceNumber: ++this.sequenceNumber,
        detachRequest: { sessionId: this.sessionId },
      });
      this.sessionId = '';
    }
  }

  setHandlers(handlers: MessageHandler): void {
    this.messageHandlers = handlers;
  }

  getSessionId(): string {
    return this.sessionId;
  }

  onStateChange(listener: (state: ConnectionState) => void): () => void {
    this.stateListeners.add(listener);
    return () => {
      this.stateListeners.delete(listener);
    };
  }

  getState(): ConnectionState {
    return this.state;
  }

  private sendEnvelope(envelope: any): void {
    try {
      const buffer = encodeEnvelope(envelope);
      if (this.dataChannel && this.dataChannel.readyState === 'open') {
        this.dataChannel.send(buffer);
      } else {
        console.warn('Cannot send: DataChannel not open');
      }
    } catch (error) {
      console.error('Failed to encode envelope:', error);
    }
  }

  private handleMessage(data: ArrayBuffer): void {
    try {
      const obj = decodeEnvelope(data);
      const seqNum = obj.sequenceNumber;
      const pending = this.pendingRequests.get(seqNum);

      if (obj.healthCheckResponse && pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(seqNum);
        pending.resolve(obj.healthCheckResponse);
      } else if (obj.dashboardResponse && pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(seqNum);
        pending.resolve(obj.dashboardResponse);
      } else if (obj.detectionResponse && pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(seqNum);
        pending.resolve(obj.detectionResponse);
      } else if (obj.attachResponse && this.messageHandlers.onAttachResponse) {
        this.messageHandlers.onAttachResponse(obj.attachResponse);
        this.sessionId = obj.attachResponse.sessionId;
      } else if (obj.outputData && this.messageHandlers.onOutputData) {
        this.lastSeenSequence = obj.outputData.sequence;
        this.messageHandlers.onOutputData(obj.outputData);
      } else if (obj.errorResponse && this.messageHandlers.onErrorResponse) {
        this.messageHandlers.onErrorResponse(obj.errorResponse);
        if (pending) {
          clearTimeout(pending.timeout);
          this.pendingRequests.delete(seqNum);
          pending.reject(new Error(obj.errorResponse.message));
        }
      } else if (obj.layoutUpdate && this.messageHandlers.onLayoutUpdate) {
        this.messageHandlers.onLayoutUpdate(obj.layoutUpdate);
      }
    } catch (error) {
      console.error('Failed to decode DataChannel message:', error);
    }
  }

  private setState(newState: ConnectionState): void {
    if (this.state !== newState) {
      this.state = newState;
      this.stateListeners.forEach((listener) => {
        try {
          listener(newState);
        } catch (error) {
          console.error('State listener error:', error);
        }
      });
    }
  }
}
