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
  id: string;
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

/** Inverse of toRelayWsUrl — recovers the accounts HTTP base URL from the
 * relay WebSocket URL a WebRtcClient was configured with, so TURN
 * credentials can be fetched without requiring a logged-in session (the
 * endpoint is unauthenticated, keyed by peer_id only — see turn.rs). */
export function toAccountsHttpUrl(relayWsUrl: string): string {
  const trimmed = trimBaseUrl(relayWsUrl);
  if (trimmed.startsWith('wss://')) return 'https://' + trimmed.slice('wss://'.length);
  if (trimmed.startsWith('ws://')) return 'http://' + trimmed.slice('ws://'.length);
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

/**
 * Starts the browser OAuth redirect flow against monoes.me: asks the relay
 * for an authorize URL (it holds the client secret and PKCE state), then
 * navigates the browser there. Call this from a click handler — it's a full
 * page navigation, not a fetch you await.
 */
export async function startLogin(baseUrl: string): Promise<void> {
  const returnTo = window.location.origin + window.location.pathname;
  const res = await fetch(
    `${trimBaseUrl(baseUrl)}/api/oauth/start?return_to=${encodeURIComponent(returnTo)}`
  );
  const data = await parseJsonOrThrow<{ authorize_url: string }>(res);
  localStorage.setItem(STORAGE_KEY_BASE_URL, trimBaseUrl(baseUrl));
  window.location.assign(data.authorize_url);
}

/**
 * After `/api/oauth/callback` redirects back here, the relay session JWT and
 * email are in the URL fragment (never a query param — see
 * crates/signaling-relay/src/oauth.rs for why). Consumes and strips it.
 * Returns true if a session was picked up.
 */
export function consumeAuthFromFragment(): boolean {
  if (!window.location.hash) return false;

  const params = new URLSearchParams(window.location.hash.slice(1));
  const token = params.get('monoterminal_token');
  const email = params.get('email');
  if (!token || !email) return false;

  const baseUrl = localStorage.getItem(STORAGE_KEY_BASE_URL);
  if (!baseUrl) return false;

  persistAuth(baseUrl, token, email);

  params.delete('monoterminal_token');
  params.delete('email');
  const remaining = params.toString();
  const cleanUrl =
    window.location.pathname + window.location.search + (remaining ? `#${remaining}` : '');
  window.history.replaceState(null, '', cleanUrl);
  return true;
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

export interface TurnCredentials {
  urls: string[];
  username: string;
  credential: string;
  expires_at: number;
}

/** Unauthenticated — mirrors the relay's own trust model for this endpoint
 * (see crates/signaling-relay/src/turn.rs): keyed by peer_id only, rate
 * limited server-side, no Bearer token required. */
export async function getTurnCredentials(
  baseUrl: string,
  peerId: string
): Promise<TurnCredentials> {
  const res = await fetch(
    `${trimBaseUrl(baseUrl)}/api/turn-credentials?peer_id=${encodeURIComponent(peerId)}`
  );
  return parseJsonOrThrow<TurnCredentials>(res);
}

export async function unlinkComputer(id: number): Promise<{ ok: true }> {
  const auth = requireAuth();
  const res = await fetch(`${trimBaseUrl(auth.baseUrl)}/api/computers/${id}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${auth.token}` },
  });
  return parseJsonOrThrow<{ ok: true }>(res);
}

/** Named `*Remote` to avoid colliding with `WorkspaceContext`'s local
 * `renameComputer` — `Sidebar.tsx` imports both (matches
 * `renameWorkspaceRemote`'s naming). Propagates a computer rename to every
 * other device synced to the same account — the daemon resolves a session
 * by "<computer>/<workspace>" name, so without this, two devices can
 * silently attach to two different sessions once one of them renames a
 * computer locally. */
export async function renameComputerRemote(id: number, name: string): Promise<{ ok: true }> {
  const auth = requireAuth();
  const res = await fetch(`${trimBaseUrl(auth.baseUrl)}/api/computers/${id}`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${auth.token}`,
    },
    body: JSON.stringify({ name }),
  });
  return parseJsonOrThrow<{ ok: true }>(res);
}

export interface ServerWorkspace {
  id: number;
  name: string;
  created_at: number;
}

export async function listWorkspaces(computerId: number): Promise<{ workspaces: ServerWorkspace[] }> {
  const auth = requireAuth();
  const res = await fetch(`${trimBaseUrl(auth.baseUrl)}/api/computers/${computerId}/workspaces`, {
    headers: { Authorization: `Bearer ${auth.token}` },
  });
  return parseJsonOrThrow<{ workspaces: ServerWorkspace[] }>(res);
}

/** Idempotent server-side (unique per computer+name) — safe to call whenever
 * a workspace is created locally, without worrying about a 409 race against
 * another device creating the same name. */
export async function createWorkspace(
  computerId: number,
  name: string
): Promise<{ workspace: ServerWorkspace }> {
  const auth = requireAuth();
  const res = await fetch(`${trimBaseUrl(auth.baseUrl)}/api/computers/${computerId}/workspaces`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${auth.token}`,
    },
    body: JSON.stringify({ name }),
  });
  return parseJsonOrThrow<{ workspace: ServerWorkspace }>(res);
}

export async function renameWorkspaceRemote(workspaceId: number, name: string): Promise<{ ok: true }> {
  const auth = requireAuth();
  const res = await fetch(`${trimBaseUrl(auth.baseUrl)}/api/workspaces/${workspaceId}`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${auth.token}`,
    },
    body: JSON.stringify({ name }),
  });
  return parseJsonOrThrow<{ ok: true }>(res);
}

export async function deleteWorkspaceRemote(workspaceId: number): Promise<{ ok: true }> {
  const auth = requireAuth();
  const res = await fetch(`${trimBaseUrl(auth.baseUrl)}/api/workspaces/${workspaceId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${auth.token}` },
  });
  return parseJsonOrThrow<{ ok: true }>(res);
}
