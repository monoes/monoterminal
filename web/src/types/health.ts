/**
 * Monomind Health Check Types
 * Corresponds to proto/monoterminal/v1/messages.proto health messages
 * SRS §2.4.3
 */

export type HealthStatus = 'unknown' | 'healthy' | 'warning' | 'error';

export enum IssueSeverity {
  INFO = 0,
  WARNING = 1,
  ERROR = 2,
}

export interface HealthIssue {
  severity: IssueSeverity;
  message: string;
  resolution: string;
}

export interface HealthCheckResponse {
  installed: boolean;
  version: string;
  controlServerReachable: boolean;
  brokerRegistered: boolean;
  lastCheckTimestamp: number; // Unix timestamp in seconds
  issues: HealthIssue[];
}

export interface HealthCheckRequest {
  projectDir?: string; // Defaults to session cwd if empty
}

export interface UpgradeRequest {
  projectDir?: string;
  confirmed: boolean; // MUST be true for upgrade to proceed
}

export interface UpgradeResponse {
  success: boolean;
  oldVersion: string;
  newVersion: string;
  output: string; // Full command output
}

/**
 * Compute overall health status from HealthCheckResponse
 */
export function computeHealthStatus(response: HealthCheckResponse): HealthStatus {
  if (!response.installed) {
    return 'error';
  }

  const hasErrors = response.issues.some(issue => issue.severity === IssueSeverity.ERROR);
  const hasWarnings = response.issues.some(issue => issue.severity === IssueSeverity.WARNING);

  if (hasErrors || !response.controlServerReachable || !response.brokerRegistered) {
    return 'error';
  }

  if (hasWarnings) {
    return 'warning';
  }

  return 'healthy';
}

/**
 * Format timestamp for display
 */
export function formatTimestamp(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleString();
}

/**
 * Get status indicator emoji
 */
export function getStatusEmoji(status: HealthStatus): string {
  switch (status) {
    case 'healthy':
      return '✓';
    case 'warning':
      return '⚠';
    case 'error':
      return '✗';
    case 'unknown':
      return '?';
  }
}

/**
 * Get status text description
 */
export function getStatusText(status: HealthStatus): string {
  switch (status) {
    case 'healthy':
      return 'All systems operational';
    case 'warning':
      return 'Some issues detected';
    case 'error':
      return 'Critical issues found';
    case 'unknown':
      return 'Status unknown';
  }
}
