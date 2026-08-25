/**
 * Clipboard Paste Button for iOS Safari (ADR-020)
 * iOS Safari requires user gesture for clipboard access
 */

import { useState } from 'react';

export interface ClipboardPasteButtonProps {
  requestId: string;
  sessionId: string;
  onPaste: (requestId: string, content: string | null) => void;
  onDeny: (requestId: string) => void;
}

export function ClipboardPasteButton({
  requestId,
  sessionId,
  onPaste,
  onDeny,
}: ClipboardPasteButtonProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleTapToPaste = async () => {
    setLoading(true);
    setError(null);

    try {
      // User tap = gesture → clipboard.readText() is allowed on iOS Safari
      const text = await navigator.clipboard.readText();
      onPaste(requestId, text);
    } catch (err) {
      console.error('[ClipboardPasteButton] Failed to read clipboard:', err);
      setError(err instanceof Error ? err.message : 'Clipboard access denied');
      setLoading(false);
    }
  };

  const handleDeny = () => {
    onDeny(requestId);
  };

  return (
    <div className="clipboard-paste-button-overlay">
      <div className="clipboard-paste-button-modal">
        <div className="clipboard-paste-header">
          <h3>📋 Clipboard Access Request</h3>
          <p className="session-info">
            Session <code>{sessionId.substring(0, 8)}...</code>
          </p>
        </div>

        <div className="clipboard-paste-body">
          <p>The terminal session wants to read your clipboard.</p>
          <p className="ios-note">
            <strong>iOS Safari:</strong> Tap the button below to share your clipboard.
          </p>

          {error && (
            <div className="clipboard-error">
              <span className="error-icon">⚠️</span>
              <span>{error}</span>
            </div>
          )}
        </div>

        <div className="clipboard-paste-actions">
          <button
            className="clipboard-paste-btn clipboard-paste-btn-deny"
            onClick={handleDeny}
            disabled={loading}
          >
            Deny
          </button>
          <button
            className="clipboard-paste-btn clipboard-paste-btn-paste"
            onClick={handleTapToPaste}
            disabled={loading}
            autoFocus
          >
            {loading ? '⏳ Reading...' : '📋 Tap to Paste'}
          </button>
        </div>

        <div className="clipboard-paste-footer">
          <small>
            Your tap gesture allows iOS Safari to access the clipboard. This is required by Apple's
            security model.
          </small>
        </div>
      </div>

      <style>{`
        .clipboard-paste-button-overlay {
          position: fixed;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background: rgba(0, 0, 0, 0.8);
          display: flex;
          align-items: center;
          justify-content: center;
          z-index: 9999;
          backdrop-filter: blur(4px);
        }

        .clipboard-paste-button-modal {
          background: #1e1e1e;
          border: 2px solid #3c3c3c;
          border-radius: 12px;
          padding: 24px;
          max-width: 400px;
          width: 90%;
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
          color: #d4d4d4;
        }

        .clipboard-paste-header h3 {
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

        .clipboard-paste-body {
          margin-bottom: 20px;
        }

        .clipboard-paste-body p {
          margin: 0 0 12px 0;
          line-height: 1.5;
        }

        .ios-note {
          background: rgba(52, 152, 219, 0.1);
          border: 1px solid #3498db;
          padding: 10px 12px;
          border-radius: 4px;
          font-size: 14px;
        }

        .ios-note strong {
          color: #3498db;
        }

        .clipboard-error {
          display: flex;
          align-items: center;
          gap: 8px;
          background: rgba(220, 53, 69, 0.1);
          border: 1px solid #dc3545;
          border-radius: 4px;
          padding: 10px 12px;
          margin-top: 12px;
          color: #dc3545;
          font-size: 14px;
        }

        .error-icon {
          font-size: 18px;
        }

        .clipboard-paste-actions {
          display: flex;
          gap: 12px;
          justify-content: flex-end;
        }

        .clipboard-paste-btn {
          padding: 12px 24px;
          border-radius: 8px;
          border: none;
          font-size: 15px;
          font-weight: 600;
          cursor: pointer;
          transition: all 0.2s;
          touch-action: manipulation; /* iOS optimization */
        }

        .clipboard-paste-btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .clipboard-paste-btn-deny {
          background: #6c757d;
          color: #ffffff;
        }

        .clipboard-paste-btn-deny:hover:not(:disabled) {
          background: #5a6268;
        }

        .clipboard-paste-btn-paste {
          background: #28a745;
          color: #ffffff;
          flex: 1;
        }

        .clipboard-paste-btn-paste:hover:not(:disabled) {
          background: #218838;
          transform: translateY(-1px);
          box-shadow: 0 4px 12px rgba(40, 167, 69, 0.3);
        }

        .clipboard-paste-btn-paste:active:not(:disabled) {
          transform: translateY(0);
        }

        .clipboard-paste-footer {
          margin-top: 16px;
          padding-top: 16px;
          border-top: 1px solid #3c3c3c;
        }

        .clipboard-paste-footer small {
          font-size: 12px;
          color: #999;
          line-height: 1.4;
        }

        /* Mobile optimizations */
        @media (max-width: 600px) {
          .clipboard-paste-button-modal {
            max-width: none;
            width: 95%;
            padding: 20px;
          }

          .clipboard-paste-btn {
            padding: 14px 20px;
            font-size: 16px; /* iOS minimum for no zoom */
          }
        }
      `}</style>
    </div>
  );
}
