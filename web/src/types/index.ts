/**
 * MONOTERMINAL Web Client Type Definitions
 *
 * This module re-exports all TypeScript types corresponding to the
 * proto/monoterminal/v1/messages.proto Protocol Buffers schema.
 *
 * Organization:
 * - protocol.ts: Core wire protocol types (Envelope, Attach, Input/Output, Error)
 * - health.ts: Health check and upgrade types (SRS §2.4.3)
 * - dashboard.ts: Dashboard, detection, and monitoring types (SRS §2.4.1, §2.4.2)
 *
 * Usage:
 *   import { ErrorCode, HealthCheckRequest, MonitoringData } from '@/types';
 */

// ============================================================================
// Core Protocol Types
// ============================================================================

export {
  CompressionType,
  ErrorCode,
  AttachRequest,
  AttachResponse,
  InputData,
  OutputData,
  ResizeRequest,
  DetachRequest,
  ErrorResponse,
  SessionMetadata,
  Line,
  Envelope,
  getErrorMessage,
  isRecoverableError,
} from './protocol';

// ============================================================================
// Health Check & Upgrade Types
// ============================================================================

export {
  HealthStatus,
  IssueSeverity,
  HealthIssue,
  HealthCheckRequest,
  HealthCheckResponse,
  UpgradeRequest,
  UpgradeResponse,
  computeHealthStatus,
  formatTimestamp,
  getStatusEmoji,
  getStatusText,
} from './health';

// ============================================================================
// Dashboard & Monitoring Types
// ============================================================================

export {
  // Request/Response types
  DashboardRequest,
  DashboardResponse,
  DetectionRequest,
  DetectionResponse,

  // Backend JSON schema types
  DashboardData,
  OrgStatus,
  AgentInfo,
  RunInfo,
  MemoryStats,

  // Protobuf streaming types
  RunSummary,
  MonitoringData,

  // UI state types
  DashboardTab,
  DashboardState,

  // Helper functions
  parseDashboardResponse,
  parseDashboardData,
  formatRunDuration,
  formatRelativeTime,
  formatDbSize,
  formatUptime,
  getRunStatusClass,
  getRunStatusText,
  getAgentStatusClass,
} from './dashboard';
