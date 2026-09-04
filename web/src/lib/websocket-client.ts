/**
 * WebSocket client for MONOTERMINAL protocol communication.
 * Wire format (schema, message shapes, Envelope encode/decode) lives in
 * ./protocol — shared with webrtc-client.ts so the two transports can't
 * drift into two separate copies of the same protocol.
 */

import { decodeEnvelope, encodeEnvelope } from './protocol';
import { getAuthService } from './auth/service-singleton';
import type {
  AttachResponse,
  AuthRequest,
  AuthResponse,
  ChallengeResponse,
  ClipboardGetRequest,
  ClipboardGetResponse,
  ClipboardOSC52,
  ClipboardSetRequest,
  DashboardRequest,
  DashboardResponse,
  DetectionRequest,
  DetectionResponse,
  ErrorResponse,
  HealthCheckRequest,
  HealthCheckResponse,
  MessageHandler,
  OutputData,
  SplitDirection,
  TokenRefreshResponse,
  UpgradeRequest,
  UpgradeResponse,
} from './protocol';

export type {
  AttachRequest,
  AttachResponse,
  AuthRequest,
  AuthResponse,
  ChallengeRequest,
  ChallengeResponse,
  ClipboardGetRequest,
  ClipboardGetResponse,
  ClipboardOSC52,
  ClipboardSetRequest,
  ClosePaneCommand,
  DashboardRequest,
  DashboardResponse,
  DetectionRequest,
  DetectionResponse,
  ErrorResponse,
  FocusPaneCommand,
  HealthCheckRequest,
  HealthCheckResponse,
  Line,
  LayoutUpdate,
  MessageHandler,
  OutputData,
  PaneLayoutNode,
  SessionMetadata,
  SplitDirection,
  SplitPaneCommand,
  SplitPaneNode,
  TerminalPaneNode,
  TokenRefreshRequest,
  TokenRefreshResponse,
  UpgradeRequest,
  UpgradeResponse,
} from './protocol';

/** 'row' (side-by-side) -> HORIZONTAL, 'col' (stacked) -> VERTICAL — matches
 * SplitPane.Direction on the wire (see proto/monoterminal/v1/messages.proto). */
function directionToWire(dir: SplitDirection): number {
  return dir === 'row' ? 0 : 1;
}

/** Unix seconds — matches AuthResponse's *_expires_at fields and the JWT's
 * own exp/iat convention (see crates/master/src/auth/jwt.rs's Claims). */
function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

export enum ConnectionState {
  DISCONNECTED = 'disconnected',
  CONNECTING = 'connecting',
  CONNECTED = 'connected',
  RECONNECTING = 'reconnecting',
  ERROR = 'error',
}

export interface ConnectionConfig {
  url: string;
  autoReconnect?: boolean;
  reconnectInterval?: number; // ms
  maxReconnectAttempts?: number;
  jwtAuth?: string; // JWT for authentication
}

export class WebSocketClient {
  private ws: WebSocket | null = null;
  private config: Required<ConnectionConfig>;
  private messageHandlers: MessageHandler = {};
  private stateListeners: Set<(state: ConnectionState) => void> = new Set();
  private reconnectAttempts = 0;
  private reconnectTimer: number | null = null;
  private state: ConnectionState = ConnectionState.DISCONNECTED;
  private sequenceNumber = 0;
  private sessionId = '';
  private lastSeenSequence = 0;
  private pendingRequests: Map<
    number,
    { resolve: (value: any) => void; reject: (reason: any) => void; timeout: number }
  > = new Map();
  // `disconnect()` forces `config.autoReconnect` off so a torn-down client
  // doesn't keep trying to reconnect — but `connect()` needs to restore
  // whatever the caller originally asked for, or a single disconnect()
  // permanently disables auto-reconnect for the rest of this client's life.
  // React 18 StrictMode (dev only) double-invokes mount effects, which
  // calls connect() -> disconnect() -> connect() on the same client at
  // startup; without restoring this here, every dev-mode session silently
  // loses auto-reconnect from the very first render, so any later drop
  // (server restart, sleep/wake) leaves the terminal stuck until a manual
  // page refresh creates a fresh client.
  private readonly configuredAutoReconnect: boolean;

  // True while the post-open, pre-CONNECTED Ed25519 challenge/auth
  // handshake (see `authenticate()`) is in flight. Lets `sendEnvelope`
  // permit the handshake's own requests through before the public state
  // reaches CONNECTED, without weakening the gate for anything else.
  private authenticating = false;
  // Proactive JWT refresh (Phase 6): re-armed at the end of every
  // successful `authenticate()`, cleared on disconnect/close/supersession
  // so a stale timer can never fire against a dead socket.
  private refreshTimer: number | null = null;
  // Holds the current refresh credential in memory only (never persisted),
  // matching AuthService's own JWT-storage policy.
  private refreshCredential: string | null = null;

  constructor(config: ConnectionConfig) {
    this.config = {
      autoReconnect: true,
      reconnectInterval: 3000, // 3s default, targeting <10s total per SRS Â§7.1
      maxReconnectAttempts: 5,
      jwtAuth: '',
      ...config,
    };
    this.configuredAutoReconnect = this.config.autoReconnect;
  }

  connect(): void {
    if (this.state === ConnectionState.CONNECTED || this.state === ConnectionState.CONNECTING) {
      return;
    }

    this.config.autoReconnect = this.configuredAutoReconnect;
    this.setState(
      this.reconnectAttempts > 0 ? ConnectionState.RECONNECTING : ConnectionState.CONNECTING
    );

    try {
      // Capture this specific socket instance so every handler below can
      // check it's still the live one before touching shared state. React
      // 18 StrictMode double-invokes mount effects in dev, which calls
      // connect() then disconnect() then connect() again in quick
      // succession — the FIRST socket's close event used to fire after the
      // SECOND (real) socket had already opened and been assigned to
      // this.ws, and unconditionally did `this.ws = null`, wiping out the
      // live socket reference. attach()'s sendEnvelope() then saw a null/
      // stale this.ws and silently dropped the attach request forever,
      // leaving the UI showing "Connected" with no session ever attached.
      const socket = new WebSocket(this.config.url);
      this.ws = socket;
      socket.binaryType = 'arraybuffer';

      socket.onopen = () => {
        if (this.ws !== socket) return; // superseded by a newer connect()
        console.log('WebSocket connected, authenticating...');
        this.reconnectAttempts = 0;
        this.sequenceNumber = 0; // Reset sequence on new connection
        void this.authenticate(socket);
      };

      socket.onmessage = (event) => {
        if (this.ws !== socket) return;
        if (event.data instanceof ArrayBuffer) {
          this.handleMessage(event.data);
        } else {
          console.warn('Received non-binary message, ignoring');
        }
      };

      socket.onerror = (error) => {
        if (this.ws !== socket) return;
        console.error('WebSocket error:', error);
        this.setState(ConnectionState.ERROR);
      };

      socket.onclose = (event) => {
        if (this.ws !== socket) return; // stale socket — already superseded
        console.log('WebSocket closed:', event.code, event.reason);
        this.ws = null;

        if (
          this.config.autoReconnect &&
          this.reconnectAttempts < this.config.maxReconnectAttempts
        ) {
          this.scheduleReconnect();
        } else {
          this.setState(ConnectionState.DISCONNECTED);
        }
      };
    } catch (error) {
      console.error('Failed to create WebSocket:', error);
      this.setState(ConnectionState.ERROR);
    }
  }

  disconnect(): void {
    this.config.autoReconnect = false;
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.clearRefreshTimer();
    this.authenticating = false;

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    this.setState(ConnectionState.DISCONNECTED);
  }

  private clearRefreshTimer(): void {
    if (this.refreshTimer !== null) {
      clearTimeout(this.refreshTimer);
      this.refreshTimer = null;
    }
  }

  /**
   * Ed25519 challenge-response handshake (SRS §3.2.2), run once per socket
   * right after it opens and before the public CONNECTED transition —
   * `HybridTransport` treats CONNECTED as "this route is usable, call
   * attach()", so firing it before a real JWT exists would race the
   * attach's own auth_token against this handshake still completing.
   *
   * Every step re-checks `this.ws === socket`, same as `onopen`/`onmessage`/
   * `onerror`/`onclose` above — this method awaits across multiple network
   * round trips, each of which is a new window where a StrictMode
   * double-invoke or a fresh connect() could have superseded this socket.
   */
  private async authenticate(socket: WebSocket): Promise<void> {
    this.authenticating = true;
    try {
      const authService = await getAuthService();
      if (this.ws !== socket) return;

      // Reuse a still-valid JWT across a reconnect/failover instead of
      // re-running the full round trip — keeps the identity stable and
      // reconnects fast.
      const existing = authService.getJWT();
      if (existing) {
        this.config.jwtAuth = existing;
        this.authenticating = false;
        this.setState(ConnectionState.CONNECTED);
        this.scheduleRefresh(authService.getJWTTimeRemaining() ?? 0);
        return;
      }

      const challenge = await this.sendChallengeRequest();
      if (this.ws !== socket) return;

      const signed = await authService.signChallenge(challenge);
      if (this.ws !== socket) return;

      const authResult = await this.sendAuthRequest({
        signature: signed.signature,
        publicKey: signed.publicKey,
        nonce: challenge.nonce,
      });
      if (this.ws !== socket) return;

      const access = authResult.accessToken;
      const refresh = authResult.refreshToken;
      authService.setJWT(access, authResult.accessExpiresAt - nowSeconds());
      this.storeRefreshCredential(refresh);
      this.config.jwtAuth = access;

      console.log(`Authenticated as ${authResult.userId}`);
      this.authenticating = false;
      this.setState(ConnectionState.CONNECTED);
      this.scheduleRefresh(authResult.accessExpiresAt - nowSeconds());
    } catch (error) {
      if (this.ws !== socket) return; // superseded mid-handshake — not this socket's problem anymore
      console.error('Authentication failed:', error);
      this.authenticating = false;
      this.setState(ConnectionState.ERROR);
      socket.close();
    }
  }

  private storeRefreshCredential(value: string): void {
    this.refreshCredential = value;
  }

  /** Proactively refreshes the JWT ~2 minutes before it expires, so a
   * terminal session that stays on one healthy socket far longer than the
   * 15-minute access-token lifetime doesn't start failing auth mid-use.
   * Falls back to a full `authenticate()` re-run if the refresh itself
   * fails (e.g. the stored credential was also rejected). */
  private scheduleRefresh(accessTtlSeconds: number): void {
    this.clearRefreshTimer();
    const delayMs = Math.max((accessTtlSeconds - 120) * 1000, 5000);
    const socket = this.ws;

    this.refreshTimer = window.setTimeout(async () => {
      if (this.ws !== socket || !this.refreshCredential) return;
      try {
        const result = await this.refreshJWT(this.refreshCredential);
        if (this.ws !== socket) return;
        const authService = await getAuthService();
        const access = result.accessToken;
        const refresh = result.refreshToken; // rotated — the old value is now single-use-burned
        authService.setJWT(access, result.accessExpiresAt - nowSeconds());
        this.storeRefreshCredential(refresh);
        this.config.jwtAuth = access;
        this.scheduleRefresh(result.accessExpiresAt - nowSeconds());
      } catch (error) {
        if (this.ws !== socket) return;
        console.warn('JWT refresh failed, re-authenticating:', error);
        // Clear the cached (still-technically-unexpired) access token first —
        // otherwise authenticate()'s fast path sees it as still valid and
        // just re-arms another refresh with the same already-proven-bad
        // refresh token, looping every 5s until the access token's own
        // expiry instead of getting fresh credentials now.
        const authService = await getAuthService();
        authService.clearJWT();
        void this.authenticate(socket);
      }
    }, delayMs);
  }

  /**
   * Attach to a session (or create new). When `sessionId` is empty and
   * `sessionName` is given, the server finds-or-creates a session keyed by
   * that stable logical name — so the same terminal opened from another
   * browser/device (which derives the same name) converges on the same
   * live session instead of spawning an independent one.
   */
  attach(
    sessionId: string,
    rows: number,
    cols: number,
    sessionName?: string,
    previousSessionName?: string
  ): void {
    const jwt = this.config.jwtAuth || '';
    const envelope: any = {
      sequenceNumber: ++this.sequenceNumber,
      attachRequest: {
        sessionId: sessionId || '',
        // protobufjs converts the wire field `auth_token` to camelCase
        // `authToken` for JS access (default `keepCase: false`) — the
        // previous `['auth' + '_token']` trick set a property named
        // `auth_token` that the encoder never reads, so this field was
        // silently always empty on the wire.
        authToken: jwt,
        rows,
        cols,
        lastSeenSequence: this.lastSeenSequence,
        sessionName: sessionName || '',
        previousSessionName: previousSessionName || '',
      },
    };

    this.sendEnvelope(envelope);
    this.sessionId = sessionId;
  }

  /**
   * Send terminal input. `paneId` targets a specific pane (Phase 4:
   * Splits/Tabs) — omitted for a plain, non-paned session.
   */
  sendInput(data: string | Uint8Array, paneId?: string): void {
    const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
    const jwt = this.config.jwtAuth || '';
    const envelope: any = {
      sequenceNumber: ++this.sequenceNumber,
      inputData: { data: bytes, paneId, authToken: jwt },
    };

    this.sendEnvelope(envelope);
  }

  /**
   * Send resize request. `paneId` resizes a specific pane's own PTY (Phase
   * 4: Splits/Tabs) — omitted for a plain, non-paned session.
   */
  resize(rows: number, cols: number, paneId?: string): void {
    const jwt = this.config.jwtAuth || '';
    const envelope: any = {
      sequenceNumber: ++this.sequenceNumber,
      resizeRequest: { rows, cols, paneId, authToken: jwt },
    };

    this.sendEnvelope(envelope);
  }

  /**
   * Split a pane into two (Phase 4: Splits/Tabs). The new pane's session id
   * arrives via the next LayoutUpdate — see MessageHandler.onLayoutUpdate.
   */
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

  /** Focus a pane, changing which one receives keyboard input by default
   * (Phase 4: Splits/Tabs). */
  focusPane(paneId: string): void {
    this.sendEnvelope({
      sequenceNumber: ++this.sequenceNumber,
      focusPaneCommand: { paneId },
    });
  }

  /**
   * Detach from session
   */
  detach(): void {
    if (this.sessionId) {
      const envelope = {
        sequenceNumber: ++this.sequenceNumber,
        detachRequest: { sessionId: this.sessionId },
      };

      this.sendEnvelope(envelope);
      this.sessionId = '';
    }
  }

  /**
   * Set message handlers
   */
  setHandlers(handlers: MessageHandler): void {
    this.messageHandlers = handlers;
  }

  /**
   * Get current session ID
   */
  getSessionId(): string {
    return this.sessionId;
  }

  /**
   * Send health check request
   */
  async sendHealthCheckRequest(req: HealthCheckRequest): Promise<HealthCheckResponse> {
    const seqNum = ++this.sequenceNumber;
    const envelope = {
      sequenceNumber: seqNum,
      healthCheckRequest: {
        projectDir: req.projectDir || '',
      },
    };

    return this.sendRequestWithResponse(envelope, seqNum, 10000); // 10s timeout
  }

  /**
   * Send upgrade request
   */
  async sendUpgradeRequest(req: UpgradeRequest): Promise<UpgradeResponse> {
    const seqNum = ++this.sequenceNumber;
    const envelope = {
      sequenceNumber: seqNum,
      upgradeRequest: {
        projectDir: req.projectDir || '',
        confirmed: req.confirmed,
      },
    };

    return this.sendRequestWithResponse(envelope, seqNum, 60000); // 60s timeout for upgrade
  }

  /**
   * Send dashboard data request
   */
  async sendDashboardRequest(req: DashboardRequest): Promise<DashboardResponse> {
    const seqNum = ++this.sequenceNumber;
    const envelope = {
      sequenceNumber: seqNum,
      dashboardRequest: {
        command: req.command,
        params: req.params || {},
      },
    };

    return this.sendRequestWithResponse(envelope, seqNum, 10000); // 10s timeout
  }

  /**
   * Send detection request
   */
  async sendDetectionRequest(req: DetectionRequest): Promise<DetectionResponse> {
    const seqNum = ++this.sequenceNumber;
    const envelope = {
      sequenceNumber: seqNum,
      detectionRequest: {
        projectDir: req.projectDir,
      },
    };

    return this.sendRequestWithResponse(envelope, seqNum, 5000); // 5s timeout
  }

  /**
   * Request authentication challenge from server
   */
  async sendChallengeRequest(): Promise<ChallengeResponse> {
    const seqNum = ++this.sequenceNumber;
    const envelope = {
      sequenceNumber: seqNum,
      challengeRequest: {},
    };
    // Tighter than the usual 5s request timeout: this happens inside
    // connect()'s own budget (see HybridTransport's WS_CONNECT_TIMEOUT_MS),
    // so a wedged daemon should fail fast enough to still be caught by that
    // outer deadline rather than being cut off mid-request by it.
    return this.sendRequestWithResponse(envelope, seqNum, 3000);
  }

  /**
   * Submit signed challenge for authentication
   */
  async sendAuthRequest(req: AuthRequest): Promise<AuthResponse> {
    const seqNum = ++this.sequenceNumber;
    const envelope = {
      sequenceNumber: seqNum,
      authRequest: {
        signature: req.signature,
        publicKey: req.publicKey,
        nonce: req.nonce,
      },
    };
    return this.sendRequestWithResponse(envelope, seqNum, 3000);
  }


  /**
   * Refresh JWT access credentials
   */
  async refreshJWT(refresh: string): Promise<TokenRefreshResponse> {
    const seqNum = ++this.sequenceNumber;
    const request = { refreshToken: refresh };
    const envelope = {
      sequenceNumber: seqNum,
      tokenRefreshRequest: request,
    };
    return this.sendRequestWithResponse(envelope, seqNum, 5000);
  }

  /**
   * Send clipboard response (ADR-020)
   */
  sendClipboardResponse(response: ClipboardGetResponse): void {
    const envelope = {
      sequenceNumber: ++this.sequenceNumber,
      clipboardGetResponse: {
        requestId: response.requestId,
        content: response.content,
        mimeType: response.mimeType,
        authorized: response.authorized,
        error: response.error || '',
      },
    };
    this.sendEnvelope(envelope);
  }

  /**
   * Send clipboard set request (client initiates clipboard write)
   */
  sendClipboardSetRequest(request: ClipboardSetRequest): void {
    const envelope = {
      sequenceNumber: ++this.sequenceNumber,
      clipboardSetRequest: {
        content: request.content,
        binaryContent: request.binaryContent,
        mimeType: request.mimeType,
        timestamp: request.timestamp,
      },
    };
    this.sendEnvelope(envelope);
  }
  private sendRequestWithResponse<T>(envelope: any, seqNum: number, timeoutMs: number): Promise<T> {
    return new Promise((resolve, reject) => {
      const timeout = window.setTimeout(() => {
        this.pendingRequests.delete(seqNum);
        reject(new Error('Request timeout'));
      }, timeoutMs);

      this.pendingRequests.set(seqNum, { resolve, reject, timeout });

      try {
        this.sendEnvelope(envelope);
      } catch (error) {
        clearTimeout(timeout);
        this.pendingRequests.delete(seqNum);
        reject(error);
      }
    });
  }

  private sendEnvelope(envelope: any): void {
    try {
      const buffer = encodeEnvelope(envelope);

      // Also permitted while `authenticating`: the challenge/auth handshake
      // itself has to send envelopes before the public state reaches
      // CONNECTED (see `authenticate()`) — gating strictly on
      // ConnectionState.CONNECTED here would deadlock the handshake against
      // its own send gate.
      if (
        this.ws &&
        this.ws.readyState === WebSocket.OPEN &&
        (this.state === ConnectionState.CONNECTED || this.authenticating)
      ) {
        this.ws.send(buffer);
      } else {
        console.warn('Cannot send: WebSocket not connected');
      }
    } catch (error) {
      console.error('Failed to encode envelope:', error);
    }
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

  private handleMessage(data: ArrayBuffer): void {
    try {
      const obj = decodeEnvelope(data);

      const seqNum = obj.sequenceNumber;
      const pending = this.pendingRequests.get(seqNum);

      // Handle request-response messages
      if (obj.healthCheckResponse && pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(seqNum);
        pending.resolve(obj.healthCheckResponse);
      } else if (obj.upgradeResponse && pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(seqNum);
        pending.resolve(obj.upgradeResponse);
      } else if (obj.dashboardResponse && pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(seqNum);
        pending.resolve(obj.dashboardResponse);
      } else if (obj.detectionResponse && pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(seqNum);
        pending.resolve(obj.detectionResponse);
      } else if (obj.challengeResponse && pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(seqNum);
        pending.resolve(obj.challengeResponse);
      } else if (obj.authResponse && pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(seqNum);
        pending.resolve(obj.authResponse);
      } else if (obj.tokenRefreshResponse && pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(seqNum);
        pending.resolve(obj.tokenRefreshResponse);
      }
      // Handle clipboard messages (ADR-020)
      else if (obj.clipboardGetRequest && this.messageHandlers.onClipboardGetRequest) {
        this.messageHandlers.onClipboardGetRequest(obj.clipboardGetRequest);
      } else if (obj.clipboardOsc52 && this.messageHandlers.onClipboardOSC52) {
        this.messageHandlers.onClipboardOSC52(obj.clipboardOsc52);
      } else if (obj.layoutUpdate && this.messageHandlers.onLayoutUpdate) {
        this.messageHandlers.onLayoutUpdate(obj.layoutUpdate);
      }
      // Handle streaming messages
      else if (obj.attachResponse && this.messageHandlers.onAttachResponse) {
        this.messageHandlers.onAttachResponse(obj.attachResponse);
        this.sessionId = obj.attachResponse.sessionId;
      } else if (obj.outputData && this.messageHandlers.onOutputData) {
        this.lastSeenSequence = obj.outputData.sequence;
        this.messageHandlers.onOutputData(obj.outputData);
      } else if (obj.errorResponse) {
        // Reject the matching pending request unconditionally — this used
        // to only happen when an onErrorResponse handler was also
        // registered, so a caller with no handler (e.g. local-daemon.ts's
        // probe client, or any request made before handlers are wired up)
        // never got its promise rejected at all and just hung until the
        // request's own timeout fired.
        if (pending) {
          clearTimeout(pending.timeout);
          this.pendingRequests.delete(seqNum);
          pending.reject(new Error(obj.errorResponse.message));
        }
        if (this.messageHandlers.onErrorResponse) {
          this.messageHandlers.onErrorResponse(obj.errorResponse);
        }
      }
    } catch (error) {
      console.error('Failed to decode message:', error);
    }
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer !== null) {
      return;
    }

    this.reconnectAttempts++;
    const delay = this.config.reconnectInterval * Math.min(this.reconnectAttempts, 3); // Exponential backoff, capped

    console.log(
      `Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts}/${this.config.maxReconnectAttempts})`
    );

    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
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
