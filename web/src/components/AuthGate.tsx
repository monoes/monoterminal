import { useState } from 'react';
import type { ReactNode } from 'react';
import { consumeAuthFromFragment, getStoredAuth, startLogin } from '../lib/accounts-client';
import './AuthGate.css';

interface AuthGateProps {
  children: ReactNode;
}

const CONFIGURED_BASE_URL = import.meta.env.VITE_ACCOUNTS_URL || '';

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

    const effectiveBaseUrl = baseUrl.trim() || CONFIGURED_BASE_URL;
    if (!effectiveBaseUrl) {
      setError('No accounts server configured. Enter one above.');
      return;
    }

    setSubmitting(true);
    try {
      await startLogin(effectiveBaseUrl);
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
              placeholder={CONFIGURED_BASE_URL || 'https://accounts.example.com'}
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              autoComplete="url"
            />
            <span className="auth-field-help">
              {CONFIGURED_BASE_URL
                ? 'Where your account is hosted. Leave blank to use the default.'
                : 'Where your account is hosted.'}
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
