/**
 * Clipboard Permission Modal (ADR-020)
 * Security controls: Modal UI security (#6), Session remember (#3)
 */

import { useState, useEffect } from 'react';

export interface ClipboardPermissionModalProps {
  sessionId: string;
  frequencyWarning: { shouldWarn: boolean; requestCount: number };
  onAllow: (remember: boolean) => void;
  onDeny: () => void;
  visible: boolean;
}

export function ClipboardPermissionModal({
  sessionId,
  frequencyWarning,
  onAllow,
  onDeny,
  visible,
}: ClipboardPermissionModalProps) {
  const [rememberSession, setRememberSession] = useState(false);
  const [focusedButton, setFocusedButton] = useState<'deny' | 'allow'>('deny');

  // Security control #6: Default focus on "Deny" button
  useEffect(() => {
    if (visible) {
      setFocusedButton('deny');
      setRememberSession(false); // Reset checkbox on each show
    }
  }, [visible]);

  if (!visible) return null;

  // Security control #6: Different visual treatment for frequent requests
  const modalClass = frequencyWarning.shouldWarn
    ? 'clipboard-modal clipboard-modal-warning'
    : 'clipboard-modal';

  return (
    <div className="clipboard-modal-overlay">
      <div className={modalClass}>
        {/* Security control #6: Frequency warning */}
        {frequencyWarning.shouldWarn && (
          <div className="clipboard-warning-header">
            <span className="warning-icon">⚠️</span>
            <span className="warning-text">
              This session is requesting clipboard access frequently ({frequencyWarning.requestCount}{' '}
              times in last 5 minutes)
            </span>
          </div>
        )}

        {/* Security control #6: Session identifier */}
        <div className="clipboard-modal-header">
          <h3>Clipboard Access Request</h3>
          <p className="session-info">
            Session <code>{sessionId.substring(0, 8)}...</code> wants to read your clipboard
          </p>
        </div>

        <div className="clipboard-modal-body">
          <p>
            A remote program is requesting access to your clipboard contents. This may be a
            legitimate tool (tmux, vim) or a malicious script.
          </p>

          <div className="clipboard-info-box">
            <strong>Security:</strong> Your clipboard may contain passwords, API keys, or sensitive
            data. Only approve if you trust this session.
          </div>

          {/* Security control #3: Session-scoped remember (opt-in) */}
          <label className="clipboard-remember-checkbox">
            <input
              type="checkbox"
              checked={rememberSession}
              onChange={(e) => setRememberSession(e.target.checked)}
            />
            <span>
              Remember for this session (until disconnect or 30 min inactivity)
            </span>
          </label>
        </div>

        <div className="clipboard-modal-actions">
          {/* Security control #6: Deny button is default focus */}
          <button
            className={`clipboard-btn clipboard-btn-deny ${
              focusedButton === 'deny' ? 'focused' : ''
            }`}
            onClick={onDeny}
            autoFocus
            onFocus={() => setFocusedButton('deny')}
          >
            Deny
          </button>
          <button
            className={`clipboard-btn clipboard-btn-allow ${
              focusedButton === 'allow' ? 'focused' : ''
            }`}
            onClick={() => onAllow(rememberSession)}
            onFocus={() => setFocusedButton('allow')}
          >
            Allow
          </button>
        </div>

        <div className="clipboard-modal-footer">
          <small>
            This authorization applies only to server-initiated clipboard reads. Your regular pastes
            (Ctrl+V) are unaffected.
          </small>
        </div>
      </div>

      <style>{`
        .clipboard-modal-overlay {
          position: fixed;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background: rgba(0, 0, 0, 0.7);
          display: flex;
          align-items: center;
          justify-content: center;
          z-index: 9999;
          backdrop-filter: blur(4px);
        }

        .clipboard-modal {
          background: #1e1e1e;
          border: 2px solid #3c3c3c;
          border-radius: 8px;
          padding: 24px;
          max-width: 500px;
          width: 90%;
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
          color: #d4d4d4;
        }

        /* Security control #6: Warning visual treatment */
        .clipboard-modal-warning {
          border: 2px solid #f0ad4e;
          box-shadow: 0 0 24px rgba(240, 173, 78, 0.3);
        }

        .clipboard-warning-header {
          background: rgba(240, 173, 78, 0.15);
          border: 1px solid #f0ad4e;
          border-radius: 4px;
          padding: 12px;
          margin-bottom: 16px;
          display: flex;
          align-items: center;
          gap: 8px;
        }

        .warning-icon {
          font-size: 20px;
        }

        .warning-text {
          font-size: 14px;
          color: #f0ad4e;
          font-weight: 500;
        }

        .clipboard-modal-header h3 {
          margin: 0 0 8px 0;
          font-size: 20px;
          color: #ffffff;
        }

        .session-info {
          margin: 0 0 16px 0;
          font-size: 14px;
          color: #999;
        }

        .session-info code {
          background: #2a2a2a;
          padding: 2px 6px;
          border-radius: 3px;
          font-family: 'Consolas', 'Courier New', monospace;
          color: #4ec9b0;
        }

        .clipboard-modal-body {
          margin-bottom: 20px;
        }

        .clipboard-modal-body p {
          margin: 0 0 12px 0;
          line-height: 1.5;
        }

        .clipboard-info-box {
          background: rgba(220, 53, 69, 0.1);
          border: 1px solid #dc3545;
          padding: 12px;
          margin: 16px 0;
          font-size: 14px;
          border-radius: 4px;
        }

        .clipboard-info-box strong {
          color: #dc3545;
        }

        /* Security control #3: Remember checkbox */
        .clipboard-remember-checkbox {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 12px;
          background: rgba(76, 175, 80, 0.1);
          border: 1px solid #4caf50;
          border-radius: 4px;
          cursor: pointer;
          margin-top: 16px;
        }

        .clipboard-remember-checkbox input[type='checkbox'] {
          width: 18px;
          height: 18px;
          cursor: pointer;
        }

        .clipboard-remember-checkbox span {
          font-size: 14px;
          color: #4caf50;
        }

        .clipboard-modal-actions {
          display: flex;
          gap: 12px;
          justify-content: flex-end;
        }

        .clipboard-btn {
          padding: 10px 24px;
          border-radius: 4px;
          border: none;
          font-size: 14px;
          font-weight: 500;
          cursor: pointer;
          transition: all 0.2s;
        }

        /* Security control #6: Deny button styled as primary action */
        .clipboard-btn-deny {
          background: #6c757d;
          color: #ffffff;
        }

        .clipboard-btn-deny:hover,
        .clipboard-btn-deny.focused {
          background: #5a6268;
          box-shadow: 0 0 0 3px rgba(108, 117, 125, 0.3);
        }

        .clipboard-btn-allow {
          background: #28a745;
          color: #ffffff;
        }

        .clipboard-btn-allow:hover,
        .clipboard-btn-allow.focused {
          background: #218838;
          box-shadow: 0 0 0 3px rgba(40, 167, 69, 0.3);
        }

        .clipboard-modal-footer {
          margin-top: 16px;
          padding-top: 16px;
          border-top: 1px solid #3c3c3c;
        }

        .clipboard-modal-footer small {
          font-size: 12px;
          color: #999;
          line-height: 1.4;
        }
      `}</style>
    </div>
  );
}
