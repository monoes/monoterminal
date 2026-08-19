/**
 * Core Protocol Types
 * Corresponds to proto/monoterminal/v1/messages.proto
 * SRS §3.1.1
 */

// ============================================================================
// Enums
// ============================================================================

export enum CompressionType {
  NONE = 0,
  ZSTD = 1,
}

export enum ErrorCode {
  UNKNOWN = 0,
  SESSION_NOT_FOUND = 1,
  AUTH_FAILED = 2,
  PERMISSION_DENIED = 3,
  RATE_LIMIT_EXCEEDED = 4,
  INVALID_REQUEST = 5,
  SERVER_ERROR = 6,
}

// ============================================================================
// Core Message Types
// ============================================================================

export interface AttachRequest {
  sessionId: string; // UUID or empty for new session
  authToken: string; // JWT
  rows: number; // Terminal dimensions
  cols: number;
  lastSeenSequence: number; // For late-joiner sync (0 = full scrollback)
}

export interface AttachResponse {
  sessionId: string;
  metadata: SessionMetadata;
  scrollback: Line[]; // Last 10k lines (Phase 1: in-memory only)
}

export interface InputData {
  data: Uint8Array; // Raw keyboard input (UTF-8)
  authToken: string; // JWT Bearer token (EdDSA signed, 15min TTL)
}

export interface OutputData {
  data: Uint8Array; // PTY output chunk
  sequence: number; // For ordering/dedup
  compression: CompressionType;
}

export interface ResizeRequest {
  rows: number;
  cols: number;
  authToken: string; // JWT Bearer token (EdDSA signed, 15min TTL)
}

export interface DetachRequest {
  sessionId: string; // Session to detach from
}

export interface ErrorResponse {
  code: ErrorCode;
  message: string;
}

// ============================================================================
// Supporting Types
// ============================================================================

export interface SessionMetadata {
  shellType: string; // "cmd.exe", "powershell.exe", etc.
  workingDir: string; // Current working directory
  rows: number; // Current dimensions
  cols: number;
  createdAt: number; // Unix timestamp (seconds)
  lastActivity: number; // Unix timestamp (seconds)
}

export interface Line {
  data: Uint8Array; // Line content (UTF-8, with ANSI codes)
  lineNumber: number; // Sequential line number
}

// ============================================================================
// Envelope (top-level message wrapper)
// ============================================================================

export interface Envelope {
  sequenceNumber: number;
  message:
    | { type: 'attachRequest'; value: AttachRequest }
    | { type: 'attachResponse'; value: AttachResponse }
    | { type: 'inputData'; value: InputData }
    | { type: 'outputData'; value: OutputData }
    | { type: 'resizeRequest'; value: ResizeRequest }
    | { type: 'detachRequest'; value: DetachRequest }
    | { type: 'errorResponse'; value: ErrorResponse }
    | { type: 'dashboardRequest'; value: import('./dashboard').DashboardRequest }
    | { type: 'dashboardResponse'; value: import('./dashboard').DashboardResponse }
    | { type: 'healthCheckRequest'; value: import('./health').HealthCheckRequest }
    | { type: 'healthCheckResponse'; value: import('./health').HealthCheckResponse }
    | { type: 'upgradeRequest'; value: import('./health').UpgradeRequest }
    | { type: 'upgradeResponse'; value: import('./health').UpgradeResponse }
    | { type: 'detectionRequest'; value: import('./dashboard').DetectionRequest }
    | { type: 'detectionResponse'; value: import('./dashboard').DetectionResponse }
    | { type: 'monitoringData'; value: import('./dashboard').MonitoringData };
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Get error message from error code
 */
export function getErrorMessage(code: ErrorCode): string {
  switch (code) {
    case ErrorCode.UNKNOWN:
      return 'Unknown error occurred';
    case ErrorCode.SESSION_NOT_FOUND:
      return 'Session not found';
    case ErrorCode.AUTH_FAILED:
      return 'Authentication failed';
    case ErrorCode.PERMISSION_DENIED:
      return 'Permission denied';
    case ErrorCode.RATE_LIMIT_EXCEEDED:
      return 'Rate limit exceeded';
    case ErrorCode.INVALID_REQUEST:
      return 'Invalid request';
    case ErrorCode.SERVER_ERROR:
      return 'Server error';
    default:
      return `Error code: ${code}`;
  }
}

/**
 * Check if error is recoverable
 */
export function isRecoverableError(code: ErrorCode): boolean {
  switch (code) {
    case ErrorCode.RATE_LIMIT_EXCEEDED:
    case ErrorCode.SERVER_ERROR:
      return true; // Can retry
    case ErrorCode.SESSION_NOT_FOUND:
    case ErrorCode.AUTH_FAILED:
    case ErrorCode.PERMISSION_DENIED:
    case ErrorCode.INVALID_REQUEST:
      return false; // Terminal error
    default:
      return false;
  }
}
