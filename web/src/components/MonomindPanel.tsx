import { useState, useEffect } from 'react';
import './MonomindPanel.css';
import {
  HealthStatus,
  HealthCheckResponse,
  UpgradeResponse,
  IssueSeverity,
  computeHealthStatus,
  formatTimestamp,
  getStatusEmoji,
  getStatusText,
} from '../types/health';
import { WebSocketClient } from '../lib/websocket-client';

interface MonomindPanelProps {
  sessionId?: string;
  isVisible: boolean;
  onClose: () => void;
  wsClient?: WebSocketClient;
}

/**
 * Embedded Monomind dashboard panel
 * Per SRS §2.4.2: Lives inside the same web client, same JWT auth
 * Shows org/agent status, health checks, and upgrade controls
 *
 * Implementation status:
 * - ✅ UI layout and styling (task-7)
 * - ✅ WebSocket integration (task-8)
 * - ⏳ Full dashboard data (task-12)
 */
export function MonomindPanel({ sessionId, isVisible, onClose, wsClient }: MonomindPanelProps) {
  const [healthStatus, setHealthStatus] = useState<HealthStatus>('unknown');
  const [healthData, setHealthData] = useState<HealthCheckResponse | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [isUpgrading, setIsUpgrading] = useState(false);

  useEffect(() => {
    if (isVisible && sessionId) {
      // Auto-check health when panel opens
      checkHealth();
    }
  }, [isVisible, sessionId]);

  const checkHealth = async () => {
    if (!wsClient) {
      console.warn('WebSocket client not available');
      return;
    }

    setIsChecking(true);

    try {
      const response = await wsClient.sendHealthCheckRequest({ projectDir: '' });
      setHealthData(response);
      setHealthStatus(computeHealthStatus(response));
    } catch (error) {
      console.error('Health check failed:', error);
      setHealthStatus('error');
      setHealthData(null);
    } finally {
      setIsChecking(false);
    }
  };

  const handleUpgrade = async () => {
    if (!wsClient) {
      console.warn('WebSocket client not available');
      return;
    }

    // Per SRS §2.4.3: Require explicit user confirmation
    const confirmed = window.confirm(
      'Upgrade monomind to latest version?\n\n' +
      'This will:\n' +
      '- Run: npx monomind@latest upgrade\n' +
      '- May restart the CLI process\n' +
      '- Take 10-30 seconds\n\n' +
      'Continue?'
    );

    if (!confirmed) {
      return;
    }

    setIsUpgrading(true);

    try {
      const response = await wsClient.sendUpgradeRequest({
        projectDir: '',
        confirmed: true,
      });

      if (response.success) {
        alert(`Upgraded ${response.oldVersion} → ${response.newVersion}`);
        await checkHealth(); // Refresh health after upgrade
      } else {
        alert(`Upgrade failed:\n${response.output}`);
      }
    } catch (error) {
      console.error('Upgrade failed:', error);
      alert(`Upgrade error: ${error}`);
    } finally {
      setIsUpgrading(false);
    }
  };

  if (!isVisible) return null;

  return (
    <div className="monomind-panel" data-testid="dashboard-panel">
      <div className="panel-header" data-testid="panel-header">
        <h2>Monomind Dashboard</h2>
        <button
          className="close-btn"
          onClick={onClose}
          aria-label="Close panel"
          data-testid="close-button"
        >
          ×
        </button>
      </div>

      <div className="panel-content" data-testid="panel-content">
        {/* Health Status Section */}
        <section className="panel-section" data-testid="health-section">
          <h3>Health Status</h3>
          <div className="health-status" data-testid="health-result">
            <span
              className={`status-indicator status-${healthStatus}`}
              data-testid="health-status-indicator"
            >
              {getStatusEmoji(healthStatus)}
            </span>
            <span className="status-text" data-testid="health-status-text">
              {getStatusText(healthStatus)}
            </span>
          </div>

          {healthData && (
            <>
              <div className="health-details" data-testid="health-details">
                <p data-testid="health-version">
                  <strong>Version:</strong> {healthData.version || 'Unknown'}
                </p>
                <p data-testid="health-control-server">
                  <strong>Control Server:</strong>{' '}
                  {healthData.controlServerReachable ? '✓ Reachable' : '✗ Unreachable'}
                </p>
                <p data-testid="health-broker">
                  <strong>Broker:</strong>{' '}
                  {healthData.brokerRegistered ? '✓ Registered' : '✗ Not registered'}
                </p>
                <p className="last-check" data-testid="health-last-check">
                  Last checked: {formatTimestamp(healthData.lastCheckTimestamp)}
                </p>
              </div>

              {healthData.issues.length > 0 && (
                <div className="health-issues" data-testid="health-issues">
                  <h4>Issues Detected:</h4>
                  <ul>
                    {healthData.issues.map((issue, idx) => (
                      <li
                        key={idx}
                        className={`issue-${IssueSeverity[issue.severity].toLowerCase()}`}
                        data-testid={`health-issue-${idx}`}
                      >
                        <strong>
                          {IssueSeverity[issue.severity]}:
                        </strong>{' '}
                        {issue.message}
                        {issue.resolution && (
                          <p className="issue-resolution" data-testid={`health-issue-resolution-${idx}`}>
                            → {issue.resolution}
                          </p>
                        )}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </>
          )}

          <button
            className="action-btn"
            onClick={checkHealth}
            disabled={isChecking}
            data-testid="run-health-check"
          >
            {isChecking ? 'Checking...' : 'Run Health Check'}
          </button>
        </section>

        {/* Org Status Section */}
        <section className="panel-section" data-testid="org-section">
          <h3>Organization Status</h3>
          <div className="placeholder-content" data-testid="org-status">
            <p data-testid="org-name">
              <strong>Organization:</strong> {sessionId || 'monoterminal-dev'}
            </p>
            <p data-testid="run-status">
              <strong>Status:</strong> running
            </p>
            <p data-testid="agent-count">
              <strong>Active Agents:</strong> 0 agents
            </p>
            <div data-testid="agents-list">
              {/* Placeholder for agents list - will be populated when backend integration complete */}
              <p className="note">No active agents</p>
            </div>
          </div>
        </section>

        {/* Upgrade Section */}
        <section className="panel-section" data-testid="upgrade-section">
          <h3>Updates</h3>
          <div className="placeholder-content" data-testid="upgrade-info">
            <p data-testid="upgrade-current-version">
              <strong>Current Version:</strong>{' '}
              {healthData?.version || 'Unknown'}
            </p>
            <p className="note" data-testid="upgrade-command-note">
              Upgrade will run: <code>npx monomind@latest upgrade</code>
            </p>
          </div>
          <button
            className="action-btn secondary"
            onClick={handleUpgrade}
            disabled={isUpgrading || !healthData?.installed}
            title={!healthData?.installed ? 'Monomind not installed' : 'Upgrade to latest version'}
            data-testid="upgrade-button"
          >
            {isUpgrading ? 'Upgrading...' : 'Upgrade to Latest'}
          </button>
        </section>

        {/* Knowledge Graph Stats */}
        <section className="panel-section" data-testid="kg-section">
          <h3>Knowledge Graph</h3>
          <div className="placeholder-content" data-testid="kg-stats">
            <p data-testid="kg-nodes">Nodes: 0</p>
            <p data-testid="kg-relationships">Relationships: 0</p>
            <p data-testid="kg-last-updated">Last Updated: Never</p>
          </div>
        </section>

        <div className="panel-footer">
          <p className="note">
            <strong>Note:</strong> This dashboard will be fully functional when the monomind-bridge
            API integration is complete.
          </p>
        </div>
      </div>
    </div>
  );
}
