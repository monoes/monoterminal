import { useState } from 'react';
import './MonomindSuggestion.css';

interface MonomindSuggestionProps {
  bannerText: string;
  monomindRoot: string;
  onDismiss: () => void;
  onOpenDashboard: () => void;
}

/**
 * Monomind detection suggestion banner
 * Per SRS §2.4.1: Show when .monomind/ is detected in working directory
 * Dismissible and persisted via .monoterminal-dismiss file
 */
export function MonomindSuggestion({
  bannerText,
  monomindRoot,
  onDismiss,
  onOpenDashboard,
}: MonomindSuggestionProps) {
  const [isVisible, setIsVisible] = useState(true);

  const handleDismiss = () => {
    setIsVisible(false);
    onDismiss();
  };

  if (!isVisible) return null;

  return (
    <div className="monomind-suggestion" data-testid="monomind-suggestion">
      <div className="suggestion-content">
        <div className="suggestion-icon" data-testid="suggestion-icon">
          🧠
        </div>
        <div className="suggestion-text" data-testid="suggestion-text">
          <p className="suggestion-message" data-testid="suggestion-message">
            {bannerText}
          </p>
          <p className="suggestion-path" data-testid="suggestion-path">
            <strong>Project:</strong> {monomindRoot}
          </p>
        </div>
        <div className="suggestion-actions">
          <button
            className="suggestion-btn primary"
            onClick={onOpenDashboard}
            data-testid="suggestion-open-dashboard"
          >
            Open Dashboard
          </button>
          <button
            className="suggestion-btn secondary"
            onClick={handleDismiss}
            data-testid="monomind-suggestion-dismiss"
          >
            Dismiss
          </button>
        </div>
      </div>
    </div>
  );
}
