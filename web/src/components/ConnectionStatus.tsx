import { ConnectionState } from '../lib/websocket-client';
import './ConnectionStatus.css';

interface ConnectionStatusProps {
  state: ConnectionState;
  onReconnect?: () => void;
}

export function ConnectionStatus({ state, onReconnect }: ConnectionStatusProps) {
  const getStatusDisplay = () => {
    switch (state) {
      case ConnectionState.CONNECTED:
        return { text: 'Connected', className: 'connected', icon: '●' };
      case ConnectionState.CONNECTING:
        return { text: 'Connecting...', className: 'connecting', icon: '○' };
      case ConnectionState.RECONNECTING:
        return { text: 'Reconnecting...', className: 'reconnecting', icon: '◐' };
      case ConnectionState.DISCONNECTED:
        return { text: 'Disconnected', className: 'disconnected', icon: '○' };
      case ConnectionState.ERROR:
        return { text: 'Connection Error', className: 'error', icon: '✗' };
      default:
        return { text: 'Unknown', className: 'unknown', icon: '?' };
    }
  };

  const status = getStatusDisplay();

  return (
    <div className={`connection-status ${status.className}`} data-testid="connection-status">
      <span className="status-icon">{status.icon}</span>
      <span className="status-text" data-testid="connection-status-text">{status.text}</span>
      {(state === ConnectionState.DISCONNECTED || state === ConnectionState.ERROR) &&
        onReconnect && (
          <button className="reconnect-btn" onClick={onReconnect} data-testid="reconnect-button">
            Reconnect
          </button>
        )}
    </div>
  );
}
