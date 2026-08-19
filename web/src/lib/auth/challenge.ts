/**
 * Challenge-response authentication client
 * SRS §3.2.2: Ed25519 challenge-response flow
 *
 * Flow:
 * 1. Request challenge from server
 * 2. Server sends 32-byte random nonce
 * 3. Client signs nonce with Ed25519 private key
 * 4. Client sends signature + public key to server
 * 5. Server verifies signature and issues JWT
 */

import { sign } from './keys';

export interface Challenge {
  nonce: Uint8Array;       // 32-byte random challenge
  expiresAt: number;       // Unix timestamp (ms)
}

export interface ChallengeResponse {
  signature: Uint8Array;   // 64-byte Ed25519 signature
  publicKey: Uint8Array;   // 32-byte Ed25519 public key
}

/**
 * Parse challenge from server
 * Expects JSON: { nonce: string (base64), expiresAt: number }
 */
export function parseChallenge(data: any): Challenge {
  if (!data.nonce || typeof data.expiresAt !== 'number') {
    throw new Error('Invalid challenge format');
  }

  // Decode base64 nonce
  const nonce = Uint8Array.from(atob(data.nonce), c => c.charCodeAt(0));

  if (nonce.length !== 32) {
    throw new Error(`Invalid nonce length: expected 32 bytes, got ${nonce.length}`);
  }

  return {
    nonce,
    expiresAt: data.expiresAt,
  };
}

/**
 * Check if challenge has expired
 */
export function isChallengeExpired(challenge: Challenge): boolean {
  return Date.now() > challenge.expiresAt;
}

/**
 * Sign a challenge with Ed25519 private key
 *
 * @param challenge - The challenge received from server
 * @param privateKey - The Ed25519 private key (32 bytes)
 * @param publicKey - The Ed25519 public key (32 bytes)
 * @returns Challenge response with signature and public key
 */
export async function signChallenge(
  challenge: Challenge,
  privateKey: Uint8Array,
  publicKey: Uint8Array
): Promise<ChallengeResponse> {
  // Check expiration
  if (isChallengeExpired(challenge)) {
    throw new Error('Challenge has expired');
  }

  // Sign the challenge nonce
  const signature = await sign(challenge.nonce, privateKey);

  if (signature.length !== 64) {
    throw new Error(`Invalid signature length: expected 64 bytes, got ${signature.length}`);
  }

  return {
    signature,
    publicKey,
  };
}

/**
 * Serialize challenge response for transmission
 * Returns JSON-serializable object with base64-encoded binary data
 */
export function serializeChallengeResponse(response: ChallengeResponse): {
  signature: string;
  publicKey: string;
} {
  return {
    signature: btoa(String.fromCharCode(...response.signature)),
    publicKey: btoa(String.fromCharCode(...response.publicKey)),
  };
}
