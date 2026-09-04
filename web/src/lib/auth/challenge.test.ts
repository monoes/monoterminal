/**
 * Tests for challenge-response authentication.
 *
 * Note: tests touching real Ed25519 signing (generateKeypair/signChallenge/
 * verify) fail under jsdom due to a cross-realm SubtleCrypto/ArrayBuffer
 * instanceof mismatch (jsdom injects Node's WebCrypto into its own VM
 * realm) — confirmed to be a jsdom-only artifact, not a bug in this code;
 * the same code works against a real browser's WebCrypto. See keys.ts's
 * module doc comment.
 */

import { describe, it, expect } from 'vitest';
import { parseChallenge, isChallengeExpired, signChallenge } from './challenge';
import { generateKeypair, verify } from './keys';

function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

describe('Challenge-Response', () => {
  it('should parse valid challenge', () => {
    const nonce = new Uint8Array(32);
    crypto.getRandomValues(nonce);

    const challengeData = {
      nonce,
      expiresAt: nowSeconds() + 30, // 30s from now
    };

    const challenge = parseChallenge(challengeData);

    expect(challenge.nonce).toEqual(nonce);
    expect(challenge.expiresAt).toBe(challengeData.expiresAt);
  });

  it('should reject invalid challenge format', () => {
    expect(() => parseChallenge({} as never)).toThrow('Invalid challenge format');
    expect(() => parseChallenge({ nonce: 'abc' } as never)).toThrow('Invalid challenge format');
    expect(() => parseChallenge({ expiresAt: 123 } as never)).toThrow('Invalid challenge format');
  });

  it('should reject invalid nonce length', () => {
    const shortNonce = new Uint8Array(5);
    const challengeData = {
      nonce: shortNonce,
      expiresAt: nowSeconds() + 30,
    };

    expect(() => parseChallenge(challengeData)).toThrow('Invalid nonce length');
  });

  it('should detect expired challenge', () => {
    const nonce = new Uint8Array(32);
    crypto.getRandomValues(nonce);

    const challenge = {
      nonce,
      expiresAt: nowSeconds() - 1, // 1s in the past
    };

    expect(isChallengeExpired(challenge)).toBe(true);
  });

  it('should detect non-expired challenge', () => {
    const nonce = new Uint8Array(32);
    crypto.getRandomValues(nonce);

    const challenge = {
      nonce,
      expiresAt: nowSeconds() + 30, // 30s in the future
    };

    expect(isChallengeExpired(challenge)).toBe(false);
  });

  it('should sign challenge correctly', async () => {
    const keypair = await generateKeypair();
    const nonce = new Uint8Array(32);
    crypto.getRandomValues(nonce);

    const challenge = {
      nonce,
      expiresAt: nowSeconds() + 30,
    };

    const response = await signChallenge(challenge, keypair.privateKey, keypair.publicKey);

    expect(response.signature).toBeInstanceOf(Uint8Array);
    expect(response.signature.length).toBe(64);
    expect(response.publicKey).toEqual(keypair.publicKey);

    // Verify signature is valid
    const isValid = await verify(response.signature, challenge.nonce, response.publicKey);
    expect(isValid).toBe(true);
  });

  it('should reject signing expired challenge', async () => {
    const keypair = await generateKeypair();
    const nonce = new Uint8Array(32);
    crypto.getRandomValues(nonce);

    const challenge = {
      nonce,
      expiresAt: nowSeconds() - 1, // Expired
    };

    await expect(
      signChallenge(challenge, keypair.privateKey, keypair.publicKey)
    ).rejects.toThrow('Challenge has expired');
  });
});
