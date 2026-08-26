import { useState } from 'react';
import type { ReactNode } from 'react';
import { consumeAuthFromFragment, getStoredAuth, startLogin } from '../lib/accounts-client';
import './AuthGate.css';

interface AuthGateProps {
  children: ReactNode;
}

const DEFAULT_BASE_URL_PLACEHOLDER = 'https://accounts.example.com';

export function AuthGate({ children }: AuthGateProps) {
  const [loggedIn, setLoggedIn] = useState(() => consumeAuthFromFragment() || getStoredAuth() !== null);
  const [skipped, setSkipped] = useState(false);
  const [baseUrl, setBaseUrl] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  if (loggedIn || skipped) {
    return <>{children}</>;
  }

  async function handleSignIn() {
    setError(null);
    setSubmitting(true);
    try {
      await startLogin(baseUrl.trim() || DEFAULT_BASE_URL_PLACEHOLDER);
      // startLogin navigates away on success; nothing further runs here.
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Something went wrong.');
      setSubmitting(false);
    }
  }

  return (
    <div className="auth-gate">
      <div className="auth-card">
        <h1 className="auth-title">MONOTERMINAL</h1>
        <p className="auth-subtitle">Sign in with your monoes.me account to link and access your computers from anywhere.</p>

        <div className="auth-form">
          <label className="auth-field">
            <span>Accounts server URL</span>
            <input
              type="text"
              placeholder={DEFAULT_BASE_URL_PLACEHOLDER}
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              autoComplete="url"
            />
            <span className="auth-field-help">
              Where your account is hosted. Leave blank to use the default.
            </span>
          </label>

          {error && <div className="auth-error">{error}</div>}

          <button
            type="button"
            className="auth-submit-btn"
            onClick={handleSignIn}
            disabled={submitting}
          >
            {submitting ? 'Redirecting…' : 'Sign in with monoes.me'}
          </button>
        </div>

        <button type="button" className="auth-skip-btn" onClick={() => setSkipped(true)}>
          Continue without an account
        </button>
      </div>
    </div>
  );
}
