/**
 * Monomind Dashboard Types
 * Corresponds to proto/monoterminal/v1/messages.proto dashboard messages
 * SRS §2.4.2
 */

import { ErrorCode } from './protocol';

// ============================================================================
// Dashboard Request/Response (SRS §2.4.2)
// ============================================================================

export interface DashboardRequest {
  command: string; // "status", "agents", "memory", etc.
  params: Record<string, string>;
}

export interface DashboardResponse {
  jsonData: string; // JSON response from monomind CLI
  error: ErrorCode;
}

// ============================================================================
// Dashboard Data Structures (Backend JSON Schema)
// These match the Rust dashboard.rs implementation
// ============================================================================

/**
 * Top-level dashboard data structure
 * Matches Rust DashboardData struct from monomind-bridge/src/dashboard.rs
 */
export interface DashboardData {
  org_status: OrgStatus;
  agents: AgentInfo[];
  runs: RunInfo[];
  memory_stats: MemoryStats;
  timestamp: number; // epoch milliseconds
}

/**
 * Organization runtime status
 * Matches Rust OrgStatus struct
 */
export interface OrgStatus {
  running: boolean;
  name?: string;
  run_id?: string;
  active_agents: number;
  pending_tasks: number;
  status_message: string;
}

/**
 * Individual agent information
 * Matches Rust AgentInfo struct
 */
export interface AgentInfo {
  id: string;
  agent_type: string;
  status: string; // "running", "idle", "stopped"
  tasks_completed: number;
  uptime_secs: number;
}

/**
 * Run history entry
 * Matches Rust RunInfo struct
 */
export interface RunInfo {
  id: string;
  org_name: string;
  started_at: string; // ISO 8601 timestamp
  ended_at?: string; // ISO 8601 timestamp, undefined if still running
  outcome: string; // "success", "failed", "running"
  tokens: number;
}

/**
 * Memory and knowledge graph statistics
 * Matches Rust MemoryStats struct
 */
export interface MemoryStats {
  total_entries: number;
  kg_nodes: number;
  kg_edges: number;
  db_size_bytes: number;
}

// ============================================================================
// Detection Request/Response (SRS §2.4.1)
// ============================================================================

export interface DetectionRequest {
  projectDir: string; // Directory to check (typically PTY cwd)
}

export interface DetectionResponse {
  found: boolean; // Whether .monomind/ exists
  monomindRoot: string; // Root directory containing .monomind/ (if found)
  suggestInstall: boolean; // Whether to show install suggestion
  dismissFileExists: boolean; // Whether user has dismissed the suggestion
  bannerText: string; // MOTD-style banner to display (if suggest_install)
}

// ============================================================================
// Monitoring Data Stream (SRS §2.4.2)
// ============================================================================

export interface RunSummary {
  runId: string;
  goal: string;
  startedAt: number; // Unix timestamp (seconds)
  completedAt: number; // 0 if still running
  status: 'running' | 'completed' | 'failed';
}

export interface MonitoringData {
  // Org Status
  orgName: string;
  activeAgents: number;
  runningTasks: number;

  // Knowledge Graph Stats
  kgNodes: number;
  kgRelationships: number;
  kgLastUpdated: number; // Unix timestamp (seconds)

  // Run History (last 5 runs)
  recentRuns: RunSummary[];
}

// ============================================================================
// Dashboard UI State Types
// ============================================================================

export type DashboardTab = 'overview' | 'agents' | 'knowledge-graph' | 'runs' | 'health';

export interface DashboardState {
  activeTab: DashboardTab;
  monitoring: MonitoringData | null;
  lastUpdate: number; // Unix timestamp (ms)
  autoRefresh: boolean;
  refreshInterval: number; // ms
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Parse dashboard JSON response safely (generic)
 */
export function parseDashboardResponse<T>(response: DashboardResponse): T | null {
  if (response.error !== ErrorCode.UNKNOWN) {
    console.error('Dashboard request failed with error:', response.error);
    return null;
  }

  try {
    return JSON.parse(response.jsonData) as T;
  } catch (error) {
    console.error('Failed to parse dashboard JSON:', error);
    return null;
  }
}

/**
 * Parse DashboardData from DashboardResponse
 * Typed variant of parseDashboardResponse for the main dashboard data structure
 */
export function parseDashboardData(response: DashboardResponse): DashboardData | null {
  return parseDashboardResponse<DashboardData>(response);
}

/**
 * Format database size in human-readable format
 */
export function formatDbSize(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  } else if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(2)} KB`;
  } else if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  } else {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }
}

/**
 * Format uptime in human-readable format
 */
export function formatUptime(seconds: number): string {
  if (seconds < 60) {
    return `${seconds}s`;
  } else if (seconds < 3600) {
    const minutes = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${minutes}m ${secs}s`;
  } else if (seconds < 86400) {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${minutes}m`;
  } else {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    return `${days}d ${hours}h`;
  }
}

/**
 * Get agent status badge color class
 */
export function getAgentStatusClass(status: string): string {
  switch (status.toLowerCase()) {
    case 'running':
      return 'status-running';
    case 'idle':
      return 'status-idle';
    case 'stopped':
      return 'status-stopped';
    default:
      return 'status-unknown';
  }
}

/**
 * Format run duration
 */
export function formatRunDuration(startedAt: number, completedAt: number): string {
  const start = startedAt * 1000; // Convert to ms
  const end = completedAt === 0 ? Date.now() : completedAt * 1000;
  const duration = Math.floor((end - start) / 1000); // seconds

  if (duration < 60) {
    return `${duration}s`;
  } else if (duration < 3600) {
    const minutes = Math.floor(duration / 60);
    const seconds = duration % 60;
    return `${minutes}m ${seconds}s`;
  } else {
    const hours = Math.floor(duration / 3600);
    const minutes = Math.floor((duration % 3600) / 60);
    return `${hours}h ${minutes}m`;
  }
}

/**
 * Format timestamp relative to now
 */
export function formatRelativeTime(timestamp: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - timestamp;

  if (diff < 60) {
    return 'just now';
  } else if (diff < 3600) {
    const minutes = Math.floor(diff / 60);
    return `${minutes}m ago`;
  } else if (diff < 86400) {
    const hours = Math.floor(diff / 3600);
    return `${hours}h ago`;
  } else {
    const days = Math.floor(diff / 86400);
    return `${days}d ago`;
  }
}

/**
 * Get status badge color class
 */
export function getRunStatusClass(status: RunSummary['status']): string {
  switch (status) {
    case 'running':
      return 'status-running';
    case 'completed':
      return 'status-completed';
    case 'failed':
      return 'status-failed';
    default:
      return 'status-unknown';
  }
}

/**
 * Get status display text
 */
export function getRunStatusText(status: RunSummary['status']): string {
  switch (status) {
    case 'running':
      return 'Running';
    case 'completed':
      return 'Completed';
    case 'failed':
      return 'Failed';
    default:
      return 'Unknown';
  }
}
