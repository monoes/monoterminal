/**
 * MONOTERMINAL wire protocol — Protocol Buffers schema, message shapes, and
 * Envelope encode/decode helpers, shared by every transport that speaks it
 * (WebSocketClient today, WebRtcClient for P2P remote access). Extracted
 * out of websocket-client.ts so the two transports can't drift into two
 * separate copies of the same schema.
 *
 * Canonical source of truth: proto/monoterminal/v1/messages.proto. This is
 * a hand-maintained mirror (protobufjs can't load a .proto file bundled
 * into a browser build) — keep them in sync when the schema changes.
 */

import protobuf from 'protobufjs';

// ---- Wire message shapes -------------------------------------------------

export interface AttachRequest {
  sessionId: string;
  jwtAuth: string;
  rows: number;
  cols: number;
  lastSeenSequence?: number;
  sessionName?: string;
  /** Set when sessionName just changed (a workspace/computer rename) for a
   * session this client was already attached to — see the .proto field's
   * doc comment. */
  previousSessionName?: string;
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
  /** Which pane produced this (Phase 4: Splits/Tabs) — absent for a plain,
   * non-paned session. */
  paneId?: string;
}

// ---- Phase 4: Splits/Tabs (ADR-018) -------------------------------------

export type SplitDirection = 'row' | 'col';

export interface TerminalPaneNode {
  terminal: {
    sessionId: string;
    focused: boolean;
    paneId: string;
  };
}

export interface SplitPaneNode {
  split: {
    direction: number; // 0 = row (horizontal/side-by-side), 1 = col (vertical/stacked)
    children: PaneLayoutNode[];
    ratios: number[];
  };
}

export type PaneLayoutNode = TerminalPaneNode | SplitPaneNode;

export interface LayoutUpdate {
  root?: PaneLayoutNode;
  focusedPaneId: string;
}

export interface SplitPaneCommand {
  paneId: string;
  direction: number;
  newSessionShell?: string;
}

export interface ClosePaneCommand {
  paneId: string;
}

export interface FocusPaneCommand {
  paneId: string;
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

// Clipboard message types (ADR-020)
export interface ClipboardGetRequest {
  requestId: string;
  mimeTypes: string[];
}

export interface ClipboardGetResponse {
  requestId: string;
  content: string;
  mimeType: string;
  authorized: boolean;
  error: string;
}

export interface ClipboardSetRequest {
  content: string;
  binaryContent: Uint8Array;
  mimeType: string;
  timestamp: number;
}

export interface ClipboardOSC52 {
  content: string;
  selection: string;
}

export interface MessageHandler {
  onAttachResponse?: (response: AttachResponse) => void;
  onOutputData?: (data: OutputData) => void;
  onErrorResponse?: (error: ErrorResponse) => void;
  onChallengeResponse?: (response: ChallengeResponse) => void;
  onAuthResponse?: (response: AuthResponse) => void;
  onTokenRefreshResponse?: (response: TokenRefreshResponse) => void;
  onClipboardGetRequest?: (request: ClipboardGetRequest) => void;
  onClipboardOSC52?: (osc52: ClipboardOSC52) => void;
  onLayoutUpdate?: (update: LayoutUpdate) => void;
}

// ---- Protocol Buffers schema (inline — see module doc above) -----------

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
    SplitPaneCommand split_pane_command = 32;
    ClosePaneCommand close_pane_command = 33;
    FocusPaneCommand focus_pane_command = 34;
    LayoutUpdate layout_update = 35;
    ClipboardGetRequest clipboard_get_request = 36;
    ClipboardGetResponse clipboard_get_response = 37;
    ClipboardSetRequest clipboard_set_request = 38;
    ClipboardOSC52 clipboard_osc52 = 39;
  }
}
message AttachRequest {
  string session_id = 1;
  string auth_token = 2;
  uint32 rows = 3;
  uint32 cols = 4;
  uint64 last_seen_sequence = 5;
  string session_name = 6;
  string previous_session_name = 7;
}
message AttachResponse {
  string session_id = 1;
  SessionMetadata metadata = 2;
  repeated Line scrollback = 3;
}
message InputData {
  bytes data = 1;
  optional string auth_token = 2;
  optional string pane_id = 3;
}
message OutputData {
  bytes data = 1;
  uint64 sequence = 2;
  uint32 compression = 3;
  optional string pane_id = 4;
}
message ResizeRequest {
  uint32 rows = 1;
  uint32 cols = 2;
  optional string auth_token = 3;
  optional string pane_id = 4;
}
message PaneLayout {
  oneof pane {
    TerminalPane terminal = 1;
    SplitPane split = 2;
  }
}
message TerminalPane {
  string session_id = 1;
  bool focused = 2;
  string pane_id = 3;
}
message SplitPane {
  uint32 direction = 1;
  repeated PaneLayout children = 2;
  repeated float ratios = 3;
}
message SplitPaneCommand {
  string pane_id = 1;
  uint32 direction = 2;
  string new_session_shell = 3;
}
message ClosePaneCommand {
  string pane_id = 1;
}
message FocusPaneCommand {
  string pane_id = 1;
}
message LayoutUpdate {
  PaneLayout root = 1;
  string focused_pane_id = 2;
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
}
message ClipboardGetRequest {
  string request_id = 1;
  repeated string mime_types = 2;
}
message ClipboardGetResponse {
  string request_id = 1;
  string content = 2;
  string mime_type = 3;
  bool authorized = 4;
  string error = 5;
}
message ClipboardSetRequest {
  string content = 1;
  bytes binary_content = 2;
  string mime_type = 3;
  uint64 timestamp = 4;
}
message ClipboardOSC52 {
  string content = 1;
  string selection = 2;
}`;

let EnvelopeType: protobuf.Type;
try {
  const root = protobuf.parse(protoSchema).root;
  EnvelopeType = root.lookupType('monoterminal.v1.Envelope');
} catch (error) {
  console.error('Failed to parse protocol schema:', error);
}

/** Encode a plain-object Envelope (protobufjs conventions) to bytes. */
export function encodeEnvelope(envelope: Record<string, unknown>): Uint8Array {
  const message = EnvelopeType.create(envelope);
  return EnvelopeType.encode(message).finish();
}

/** Decode bytes into a plain-object Envelope (protobufjs `toObject` shape). */
export function decodeEnvelope(data: ArrayBuffer | Uint8Array): any {
  const buffer = data instanceof Uint8Array ? data : new Uint8Array(data);
  const envelope = EnvelopeType.decode(buffer);
  return EnvelopeType.toObject(envelope, {
    longs: Number,
    bytes: Uint8Array,
    defaults: true,
  });
}
