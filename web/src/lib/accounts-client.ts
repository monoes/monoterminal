/**
 * Thin REST client for the SaaS accounts/pairing service (see
 * crates/signaling-relay's /api/* routes). Independent of the
 * daemon-facing WebSocketClient — this talks to the accounts base URL the
 * user configures, not a specific computer.
 */

const STORAGE_KEY_JWT = 'monoterminal.auth.token';
const STORAGE_KEY_BASE_URL = 'monoterminal.auth.baseUrl';
const STORAGE_KEY_EMAIL = 'monoterminal.auth.email';

export interface AuthUser {
  id: number;
  email: string;
}

export interface StoredAuth {
  token: string;
  baseUrl: string;
  email: string;
}

export interface LinkedComputer {
  id: number;
  name: string;
  peer_id: string;
  online: boolean;
  linked_at: number;
  last_seen_at: number | null;
}

interface ErrorBody {
  error: string;
}

function trimBaseUrl(baseUrl: string): string {
  return baseUrl.replace(/\/+$/, '');
}

async function parseJsonOrThrow<T>(res: Response): Promise<T> {
  let body: unknown;
  try {
    body = await res.json();
  } catch {
    throw new Error(`Request failed (${res.status})`);
  }
  if (!res.ok) {
    const message = (body as ErrorBody)?.error || `Request failed (${res.status})`;
    throw new Error(message);
  }
  return body as T;
}

/** Converts an accounts base URL to the WebSocket URL used for P2P relay signaling. */
export function toRelayWsUrl(baseUrl: string): string {
  const trimmed = trimBaseUrl(baseUrl);
  if (trimmed.startsWith('https://')) return 'wss://' + trimmed.slice('https://'.length);
  if (trimmed.startsWith('http://')) return 'ws://' + trimmed.slice('http://'.length);
  return trimmed;
}

export function getStoredAuth(): StoredAuth | null {
  const jwt = localStorage.getItem(STORAGE_KEY_JWT);
  const baseUrl = localStorage.getItem(STORAGE_KEY_BASE_URL);
  const email = localStorage.getItem(STORAGE_KEY_EMAIL);
  if (!jwt || !baseUrl || !email) return null;
  return { token: jwt, baseUrl, email };
}

function persistAuth(baseUrl: string, jwt: string, email: string): void {
  localStorage.setItem(STORAGE_KEY_JWT, jwt);
  localStorage.setItem(STORAGE_KEY_BASE_URL, trimBaseUrl(baseUrl));
  localStorage.setItem(STORAGE_KEY_EMAIL, email);
}

export function logout(): void {
  localStorage.removeItem(STORAGE_KEY_JWT);
  localStorage.removeItem(STORAGE_KEY_BASE_URL);
  localStorage.removeItem(STORAGE_KEY_EMAIL);
}

export async function signup(
  baseUrl: string,
  email: string,
  password: string
): Promise<{ token: string; user: AuthUser }> {
  const res = await fetch(`${trimBaseUrl(baseUrl)}/api/signup`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  });
  const data = await parseJsonOrThrow<{ token: string; user: AuthUser }>(res);
  persistAuth(baseUrl, data.token, data.user.email);
  return data;
}

export async function login(
  baseUrl: string,
  email: string,
  password: string
): Promise<{ token: string; user: AuthUser }> {
  const res = await fetch(`${trimBaseUrl(baseUrl)}/api/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  });
  const data = await parseJsonOrThrow<{ token: string; user: AuthUser }>(res);
  persistAuth(baseUrl, data.token, data.user.email);
  return data;
}

function requireAuth(): StoredAuth {
  const auth = getStoredAuth();
  if (!auth) throw new Error('Not logged in');
  return auth;
}

export async function linkComputer(
  code: string,
  name?: string
): Promise<{ computer: LinkedComputer }> {
  const auth = requireAuth();
  const res = await fetch(`${trimBaseUrl(auth.baseUrl)}/api/link`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${auth.token}`,
    },
    body: JSON.stringify(name ? { code, name } : { code }),
  });
  return parseJsonOrThrow<{ computer: LinkedComputer }>(res);
}

export async function listComputers(): Promise<{ computers: LinkedComputer[] }> {
  const auth = requireAuth();
  const res = await fetch(`${trimBaseUrl(auth.baseUrl)}/api/computers`, {
    headers: { Authorization: `Bearer ${auth.token}` },
  });
  return parseJsonOrThrow<{ computers: LinkedComputer[] }>(res);
}

export async function unlinkComputer(id: number): Promise<{ ok: true }> {
  const auth = requireAuth();
  const res = await fetch(`${trimBaseUrl(auth.baseUrl)}/api/computers/${id}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${auth.token}` },
  });
  return parseJsonOrThrow<{ ok: true }>(res);
}
