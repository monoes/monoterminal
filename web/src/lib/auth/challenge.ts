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
import type { ChallengeResponse as WireChallengeResponse } from '../protocol';

export interface Challenge {
  nonce: Uint8Array;       // 32-byte random challenge
  expiresAt: number;       // Unix timestamp (seconds) — matches the JWT's
                           // own exp/iat convention (see auth/jwt.rs's Claims)
}

export interface ChallengeResponse {
  signature: Uint8Array;   // 64-byte Ed25519 signature
  publicKey: Uint8Array;   // 32-byte Ed25519 public key
}

/**
 * Parse a challenge from the server's ChallengeResponse envelope message.
 * `nonce`/`expiresAt` already arrive as real bytes/a number — protobufjs
 * decodes `bytes` fields straight into a Uint8Array, so there's no base64
 * step here (there used to be one, against a wire shape the server never
 * actually sent).
 */
export function parseChallenge(data: WireChallengeResponse): Challenge {
  if (!(data.nonce instanceof Uint8Array) || typeof data.expiresAt !== 'number') {
    throw new Error('Invalid challenge format');
  }
  if (data.nonce.length !== 32) {
    throw new Error(`Invalid nonce length: expected 32 bytes, got ${data.nonce.length}`);
  }

  return {
    nonce: data.nonce,
    expiresAt: data.expiresAt,
  };
}

/**
 * Check if challenge has expired. `expiresAt` is Unix seconds.
 */
export function isChallengeExpired(challenge: Challenge): boolean {
  return Math.floor(Date.now() / 1000) > challenge.expiresAt;
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
