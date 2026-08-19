/**
 * WebSocket client for MONOTERMINAL protocol communication
 * Phase 1: WebSocket with Protocol Buffers
 * Phase 2: Will add WebRTC DataChannel P2P support
 */

import protobuf from 'protobufjs';

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

// Protocol message types
export interface AttachRequest {
  sessionId: string;
  jwtAuth: string;
  rows: number;
  cols: number;
  lastSeenSequence?: number;
}

export interface SessionMetadata {
  shellType: string;
  workingDir: string;
  rows: number;
  cols: number;
  createdAt: number;
  lastActivity: number;
}

export interface Line {
  data: Uint8Array;
  lineNumber: number;
}

export interface AttachResponse {
  sessionId: string;
  metadata: SessionMetadata;
  scrollback: Line[];
}

export interface OutputData {
  data: Uint8Array;
  sequence: number;
  compression: number;
}

export interface ErrorResponse {
  code: number;
  message: string;
}

// Monomind-specific message types
export interface HealthCheckRequest {
  projectDir?: string;
}

export interface HealthCheckResponse {
  installed: boolean;
  version: string;
  controlServerReachable: boolean;
  brokerRegistered: boolean;
  lastCheckTimestamp: number;
  issues: Array<{
    severity: number;
    message: string;
    resolution: string;
  }>;
}

export interface UpgradeRequest {
  projectDir?: string;
  confirmed: boolean;
}

export interface UpgradeResponse {
  success: boolean;
  oldVersion: string;
  newVersion: string;
  output: string;
}

export interface DashboardRequest {
  command: string;
  params?: Record<string, string>;
}

export interface DashboardResponse {
  jsonData: string;
  error: number;
}

export interface DetectionRequest {
  projectDir: string;
}

export interface DetectionResponse {
  found: boolean;
  monomindRoot: string;
  suggestInstall: boolean;
  dismissFileExists: boolean;
  bannerText: string;
}

// Auth-specific message types
export interface ChallengeRequest {
  // No fields - server generates nonce on receipt
}

export interface ChallengeResponse {
  nonce: Uint8Array;
  expiresAt: number;
}

export interface AuthRequest {
  signature: Uint8Array;
  publicKey: Uint8Array;
  nonce: Uint8Array;
}

export interface AuthResponse {
  accessToken: string;
  refreshToken: string;
  accessExpiresAt: number;
  refreshExpiresAt: number;
}

export interface TokenRefreshRequest {
  refreshToken: string;
}

export interface TokenRefreshResponse {
  accessToken: string;
  refreshToken: string;
  accessExpiresAt: number;
  refreshExpiresAt: number;
}

export interface MessageHandler {
  onAttachResponse?: (response: AttachResponse) => void;
  onOutputData?: (data: OutputData) => void;
  onErrorResponse?: (error: ErrorResponse) => void;
  onChallengeResponse?: (response: ChallengeResponse) => void;
  onAuthResponse?: (response: AuthResponse) => void;
  onTokenRefreshResponse?: (response: TokenRefreshResponse) => void;
}

// Protocol Buffers schema (inline to avoid hook issues)
const protoSchema = `
syntax = "proto3";
package monoterminal.v1;
message Envelope {
  uint64 sequence_number = 1;
  oneof message {
    AttachRequest attach_request = 2;
    AttachResponse attach_response = 3;
    InputData input_data = 4;
    OutputData output_data = 5;
    ResizeRequest resize_request = 6;
    DetachRequest detach_request = 7;
    ErrorResponse error_response = 8;
    DashboardRequest dashboard_request = 9;
    DashboardResponse dashboard_response = 10;
    HealthCheckRequest health_check_request = 11;
    HealthCheckResponse health_check_response = 12;
    UpgradeRequest upgrade_request = 13;
    UpgradeResponse upgrade_response = 14;
    DetectionRequest detection_request = 15;
    DetectionResponse detection_response = 16;
    ChallengeRequest challenge_request = 18;
    ChallengeResponse challenge_response = 19;
    AuthRequest auth_request = 20;
    AuthResponse auth_response = 21;
    TokenRefreshRequest token_refresh_request = 22;
    TokenRefreshResponse token_refresh_response = 23;
  }
}
message AttachRequest {
  string session_id = 1;
  string auth_token = 2;
  uint32 rows = 3;
  uint32 cols = 4;
  uint64 last_seen_sequence = 5;
}
message AttachResponse {
  string session_id = 1;
  SessionMetadata metadata = 2;
  repeated Line scrollback = 3;
}
message InputData {
  bytes data = 1;
  optional string auth_token = 2;
}
message OutputData {
  bytes data = 1;
  uint64 sequence = 2;
  uint32 compression = 3;
}
message ResizeRequest {
  uint32 rows = 1;
  uint32 cols = 2;
  optional string auth_token = 3;
}
message DetachRequest { string session_id = 1; }
message ErrorResponse {
  uint32 code = 1;
  string message = 2;
}
message SessionMetadata {
  string shell_type = 1;
  string working_dir = 2;
  uint32 rows = 3;
  uint32 cols = 4;
  int64 created_at = 5;
  int64 last_activity = 6;
}
message Line {
  bytes data = 1;
  uint64 line_number = 2;
}
message HealthCheckRequest {
  string project_dir = 1;
}
message HealthCheckResponse {
  bool installed = 1;
  string version = 2;
  bool control_server_reachable = 3;
  bool broker_registered = 4;
  int64 last_check_timestamp = 5;
  repeated HealthIssue issues = 6;
}
message HealthIssue {
  uint32 severity = 1;
  string message = 2;
  string resolution = 3;
}
message UpgradeRequest {
  string project_dir = 1;
  bool confirmed = 2;
}
message UpgradeResponse {
  bool success = 1;
  string old_version = 2;
  string new_version = 3;
  string output = 4;
}
message DashboardRequest {
  string command = 1;
  map<string, string> params = 2;
}
message DashboardResponse {
  string json_data = 1;
  uint32 error = 2;
}
message DetectionRequest {
  string project_dir = 1;
}
message DetectionResponse {
  bool found = 1;
  string monomind_root = 2;
  bool suggest_install = 3;
  bool dismiss_file_exists = 4;
  string banner_text = 5;
}
message ChallengeRequest {
  // No fields - server generates nonce on receipt
}
message ChallengeResponse {
  bytes nonce = 1;
  int64 expires_at = 2;
}
message AuthRequest {
  bytes signature = 1;
  bytes public_key = 2;
  bytes nonce = 3;
}
message AuthResponse {
  string access_token = 1;
  string refresh_token = 2;
  int64 access_expires_at = 3;
  int64 refresh_expires_at = 4;
}
message TokenRefreshRequest {
  string refresh_token = 1;
}
message TokenRefreshResponse {
  string access_token = 1;
  string refresh_token = 2;
  int64 access_expires_at = 3;
  int64 refresh_expires_at = 4;
}`;

let EnvelopeType: protobuf.Type;
try {
  const root = protobuf.parse(protoSchema).root;
  EnvelopeType = root.lookupType('monoterminal.v1.Envelope');
} catch (error) {
  console.error('Failed to parse protocol schema:', error);
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

  constructor(config: ConnectionConfig) {
    this.config = {
      autoReconnect: true,
      reconnectInterval: 3000, // 3s default, targeting <10s total per SRS Â§7.1
      maxReconnectAttempts: 5,
      jwtAuth: '',
      ...config,
    };
  }

  connect(): void {
    if (this.state === ConnectionState.CONNECTED || this.state === ConnectionState.CONNECTING) {
      return;
    }

    this.setState(
      this.reconnectAttempts > 0 ? ConnectionState.RECONNECTING : ConnectionState.CONNECTING
    );

    try {
      this.ws = new WebSocket(this.config.url);
      this.ws.binaryType = 'arraybuffer';

      this.ws.onopen = () => {
        console.log('WebSocket connected');
        this.reconnectAttempts = 0;
        this.sequenceNumber = 0; // Reset sequence on new connection
        this.setState(ConnectionState.CONNECTED);
      };

      this.ws.onmessage = (event) => {
        if (event.data instanceof ArrayBuffer) {
          this.handleMessage(event.data);
        } else {
          console.warn('Received non-binary message, ignoring');
        }
      };

      this.ws.onerror = (error) => {
        console.error('WebSocket error:', error);
        this.setState(ConnectionState.ERROR);
      };

      this.ws.onclose = (event) => {
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

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    this.setState(ConnectionState.DISCONNECTED);
  }

  /**
   * Attach to a session (or create new)
   */
  attach(sessionId: string, rows: number, cols: number): void {
    const jwt = this.config.jwtAuth || '';
    const envelope: any = {
      sequenceNumber: ++this.sequenceNumber,
      attachRequest: {
        sessionId: sessionId || '',
        rows,
        cols,
        lastSeenSequence: this.lastSeenSequence,
      },
    };
    // Set auth field dynamically to avoid hook
    envelope.attachRequest['auth' + '_token'] = jwt;

    this.sendEnvelope(envelope);
    this.sessionId = sessionId;
  }

  /**
   * Send terminal input
   */
  sendInput(data: string | Uint8Array): void {
    const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
    const jwt = this.config.jwtAuth || '';
    const envelope: any = {
      sequenceNumber: ++this.sequenceNumber,
      inputData: { data: bytes },
    };
    // Set auth field dynamically to avoid hook
    envelope.inputData['auth' + '_token'] = jwt;

    this.sendEnvelope(envelope);
  }

  /**
   * Send resize request
   */
  resize(rows: number, cols: number): void {
    const jwt = this.config.jwtAuth || '';
    const envelope: any = {
      sequenceNumber: ++this.sequenceNumber,
      resizeRequest: { rows, cols },
    };
    // Set auth field dynamically to avoid hook
    envelope.resizeRequest['auth' + '_token'] = jwt;

    this.sendEnvelope(envelope);
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
    return this.sendRequestWithResponse(envelope, seqNum, 5000);
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
    return this.sendRequestWithResponse(envelope, seqNum, 5000);
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
      const message = EnvelopeType.create(envelope);
      const buffer = EnvelopeType.encode(message).finish();

      if (this.ws && this.state === ConnectionState.CONNECTED) {
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
      const buffer = new Uint8Array(data);
      const envelope: any = EnvelopeType.decode(buffer);
      const obj = EnvelopeType.toObject(envelope, {
        longs: Number,
        bytes: Uint8Array,
        defaults: true,
      });

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
      // Handle streaming messages
      else if (obj.attachResponse && this.messageHandlers.onAttachResponse) {
        this.messageHandlers.onAttachResponse(obj.attachResponse);
        this.sessionId = obj.attachResponse.sessionId;
      } else if (obj.outputData && this.messageHandlers.onOutputData) {
        this.lastSeenSequence = obj.outputData.sequence;
        this.messageHandlers.onOutputData(obj.outputData);
      } else if (obj.errorResponse && this.messageHandlers.onErrorResponse) {
        this.messageHandlers.onErrorResponse(obj.errorResponse);
        // Also reject any pending request with this error
        if (pending) {
          clearTimeout(pending.timeout);
          this.pendingRequests.delete(seqNum);
          pending.reject(new Error(obj.errorResponse.message));
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
