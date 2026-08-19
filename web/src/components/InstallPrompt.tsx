import { useState, useEffect } from 'react';
import './InstallPrompt.css';

interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>;
}

/**
 * PWA Install Prompt Component
 * Per SRS §2.2: Triggers after 2 visits + 5 min engagement
 *
 * Engagement tracking:
 * - Visit count stored in localStorage
 * - Session time tracked via visibility API
 * - Install prompt shown when both thresholds met
 */
export function InstallPrompt() {
  const [deferredPrompt, setDeferredPrompt] = useState<BeforeInstallPromptEvent | null>(null);
  const [showPrompt, setShowPrompt] = useState(false);
  const [canInstall, setCanInstall] = useState(false);

  useEffect(() => {
    // Check if already installed
    if (window.matchMedia('(display-mode: standalone)').matches) {
      return; // Already installed, don't show prompt
    }

    // Track visit count
    const visitCount = parseInt(localStorage.getItem('monoterminal-visit-count') || '0', 10);
    localStorage.setItem('monoterminal-visit-count', String(visitCount + 1));

    // Track engagement time (in milliseconds)
    let engagementStart = Date.now();
    let totalEngagement = parseInt(localStorage.getItem('monoterminal-engagement-time') || '0', 10);

    // Update engagement time periodically while page is visible
    const updateEngagement = () => {
      if (!document.hidden) {
        const now = Date.now();
        const sessionTime = now - engagementStart;
        totalEngagement += sessionTime;
        localStorage.setItem('monoterminal-engagement-time', String(totalEngagement));
        engagementStart = now;
      }
    };

    const engagementInterval = setInterval(updateEngagement, 30000); // Update every 30s

    // Handle visibility change
    const handleVisibilityChange = () => {
      if (document.hidden) {
        updateEngagement();
      } else {
        engagementStart = Date.now();
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);

    // Capture beforeinstallprompt event
    const handleBeforeInstallPrompt = (e: Event) => {
      e.preventDefault();
      setDeferredPrompt(e as BeforeInstallPromptEvent);
      setCanInstall(true);

      // Check if thresholds met: 2+ visits AND 5+ minutes engagement
      const REQUIRED_VISITS = 2;
      const REQUIRED_ENGAGEMENT_MS = 5 * 60 * 1000; // 5 minutes

      if (visitCount >= REQUIRED_VISITS && totalEngagement >= REQUIRED_ENGAGEMENT_MS) {
        // Check if user previously dismissed
        const dismissed = localStorage.getItem('monoterminal-install-dismissed');
        if (!dismissed) {
          setShowPrompt(true);
        }
      }
    };

    window.addEventListener('beforeinstallprompt', handleBeforeInstallPrompt);

    return () => {
      clearInterval(engagementInterval);
      updateEngagement(); // Final update
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      window.removeEventListener('beforeinstallprompt', handleBeforeInstallPrompt);
    };
  }, []);

  const handleInstall = async () => {
    if (!deferredPrompt) return;

    // Show the install prompt
    await deferredPrompt.prompt();

    // Wait for user response
    const { outcome } = await deferredPrompt.userChoice;

    console.log(`User ${outcome} the install prompt`);

    // Clear the deferred prompt
    setDeferredPrompt(null);
    setShowPrompt(false);
    setCanInstall(false);

    if (outcome === 'accepted') {
      // Clear engagement tracking - they installed!
      localStorage.removeItem('monoterminal-visit-count');
      localStorage.removeItem('monoterminal-engagement-time');
      localStorage.removeItem('monoterminal-install-dismissed');
    }
  };

  const handleDismiss = () => {
    setShowPrompt(false);
    // Mark as dismissed (won't show again unless localStorage is cleared)
    localStorage.setItem('monoterminal-install-dismissed', 'true');
  };

  const handleLater = () => {
    setShowPrompt(false);
    // Don't mark as permanently dismissed - will show again on next visit
  };

  if (!showPrompt) return null;

  return (
    <div className="install-prompt-overlay">
      <div className="install-prompt">
        <div className="install-prompt-header">
          <h3>Install MONOTERMINAL</h3>
          <button className="close-btn" onClick={handleDismiss} aria-label="Dismiss">
            ×
          </button>
        </div>
        <div className="install-prompt-content">
          <p>
            Add MONOTERMINAL to your home screen for a native app-like experience:
          </p>
          <ul>
            <li>Launch directly from your desktop or home screen</li>
            <li>Native window controls and full-screen mode</li>
            <li>Offline app shell for fast startup</li>
            <li>Optimized for desktop and mobile devices</li>
          </ul>
          <div className="platform-note">
            {/* iOS-specific note per SRS §2.2, §9.3 */}
            {/iPhone|iPad|iPod/i.test(navigator.userAgent) && (
              <p className="ios-note">
                <strong>iOS Note:</strong> Backgrounded sessions will disconnect after ~30s.
                Fast reconnect + scrollback sync handles this automatically.
              </p>
            )}
          </div>
        </div>
        <div className="install-prompt-actions">
          <button className="btn-secondary" onClick={handleLater}>
            Maybe Later
          </button>
          <button className="btn-primary" onClick={handleInstall}>
            Install Now
          </button>
        </div>
      </div>
    </div>
  );
}
