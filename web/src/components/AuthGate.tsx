import { useState } from 'react';
import type { FormEvent, ReactNode } from 'react';
import { getStoredAuth, login, signup } from '../lib/accounts-client';
import './AuthGate.css';

interface AuthGateProps {
  children: ReactNode;
}

type AuthMode = 'login' | 'signup';

const DEFAULT_BASE_URL_PLACEHOLDER = 'https://accounts.example.com';

export function AuthGate({ children }: AuthGateProps) {
  const [loggedIn, setLoggedIn] = useState(() => getStoredAuth() !== null);
  const [skipped, setSkipped] = useState(false);
  const [mode, setMode] = useState<AuthMode>('login');
  const [baseUrl, setBaseUrl] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  if (loggedIn || skipped) {
    return <>{children}</>;
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);

    const trimmedBaseUrl = baseUrl.trim() || DEFAULT_BASE_URL_PLACEHOLDER;
    const trimmedEmail = email.trim();

    if (!trimmedEmail || !password) {
      setError('Email and password are required.');
      return;
    }
    if (mode === 'signup' && password !== confirmPassword) {
      setError('Passwords do not match.');
      return;
    }

    setSubmitting(true);
    try {
      if (mode === 'signup') {
        await signup(trimmedBaseUrl, trimmedEmail, password);
      } else {
        await login(trimmedBaseUrl, trimmedEmail, password);
      }
      setLoggedIn(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Something went wrong.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="auth-gate">
      <div className="auth-card">
        <h1 className="auth-title">MONOTERMINAL</h1>
        <p className="auth-subtitle">
          Sign {mode === 'login' ? 'in' : 'up'} to link and access your computers from anywhere.
        </p>

        <div className="auth-mode-toggle" role="radiogroup" aria-label="Auth mode">
          <button
            type="button"
            role="radio"
            aria-checked={mode === 'login'}
            className={mode === 'login' ? 'active' : ''}
            onClick={() => {
              setMode('login');
              setError(null);
            }}
          >
            Log in
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={mode === 'signup'}
            className={mode === 'signup' ? 'active' : ''}
            onClick={() => {
              setMode('signup');
              setError(null);
            }}
          >
            Sign up
          </button>
        </div>

        <form className="auth-form" onSubmit={handleSubmit}>
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

          <label className="auth-field">
            <span>Email</span>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              autoComplete="email"
              required
            />
          </label>

          <label className="auth-field">
            <span>Password</span>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
              required
            />
          </label>

          {mode === 'signup' && (
            <label className="auth-field">
              <span>Confirm password</span>
              <input
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                autoComplete="new-password"
                required
              />
            </label>
          )}

          {error && <div className="auth-error">{error}</div>}

          <button type="submit" className="auth-submit-btn" disabled={submitting}>
            {submitting ? 'Please wait...' : mode === 'login' ? 'Log in' : 'Sign up'}
          </button>
        </form>

        <button type="button" className="auth-skip-btn" onClick={() => setSkipped(true)}>
          Continue without an account
        </button>
      </div>
    </div>
  );
}
