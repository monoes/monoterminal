/**
 * Clipboard Status Indicator (ADR-020 Security Control #3)
 * Persistent visual indicator when clipboard access is active
 */

import { useEffect, useState } from 'react';

export interface ClipboardStatusIndicatorProps {
  active: boolean;
  lastActivityMs: number; // Milliseconds since last activity
  onRevoke: () => void;
}

export function ClipboardStatusIndicator({
  active,
  lastActivityMs,
  onRevoke,
}: ClipboardStatusIndicatorProps) {
  const [timeAgo, setTimeAgo] = useState('');

  useEffect(() => {
    if (!active) return;

    const updateTimeAgo = () => {
      const seconds = Math.floor(lastActivityMs / 1000);
      const minutes = Math.floor(seconds / 60);
      const hours = Math.floor(minutes / 60);

      if (seconds < 60) {
        setTimeAgo('<1m ago');
      } else if (minutes < 60) {
        setTimeAgo(`${minutes}m ago`);
      } else {
        setTimeAgo(`${hours}h ${minutes % 60}m ago`);
      }
    };

    updateTimeAgo();
    const interval = setInterval(updateTimeAgo, 10000); // Update every 10s

    return () => clearInterval(interval);
  }, [active, lastActivityMs]);

  if (!active) return null;

  return (
    <div className="clipboard-status-indicator">
      <span className="indicator-icon">🔓</span>
      <span className="indicator-text">
        Clipboard Access: <strong>ACTIVE</strong> (last used {timeAgo})
      </span>
      <button className="indicator-revoke-btn" onClick={onRevoke} title="Revoke clipboard access">
        Revoke
      </button>

      <style>{`
        .clipboard-status-indicator {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 8px 12px;
          background: rgba(255, 193, 7, 0.15);
          border: 1px solid #ffc107;
          border-radius: 4px;
          font-size: 13px;
          color: #ffc107;
          font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
          margin-bottom: 8px;
        }

        .indicator-icon {
          font-size: 16px;
        }

        .indicator-text {
          flex: 1;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }

        .indicator-text strong {
          color: #ffffff;
          font-weight: 600;
        }

        .indicator-revoke-btn {
          padding: 4px 12px;
          background: rgba(220, 53, 69, 0.2);
          border: 1px solid #dc3545;
          border-radius: 3px;
          color: #dc3545;
          font-size: 12px;
          font-weight: 500;
          cursor: pointer;
          transition: all 0.2s;
        }

        .indicator-revoke-btn:hover {
          background: rgba(220, 53, 69, 0.3);
          border-color: #c82333;
          color: #c82333;
        }

        .indicator-revoke-btn:active {
          transform: scale(0.95);
        }

        /* Mobile responsive */
        @media (max-width: 600px) {
          .clipboard-status-indicator {
            font-size: 12px;
            padding: 6px 10px;
          }

          .indicator-text {
            font-size: 11px;
          }

          .indicator-revoke-btn {
            padding: 3px 8px;
            font-size: 11px;
          }
        }
      `}</style>
    </div>
  );
}
