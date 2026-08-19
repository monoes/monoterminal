/**
 * Tests for challenge-response authentication
 */

import { describe, it, expect } from 'vitest';
import {
  parseChallenge,
  isChallengeExpired,
  signChallenge,
  serializeChallengeResponse,
} from './challenge';
import { generateKeypair, verify } from './keys';

describe('Challenge-Response', () => {
  it('should parse valid challenge', () => {
    const nonce = new Uint8Array(32);
    crypto.getRandomValues(nonce);
    const base64Nonce = btoa(String.fromCharCode(...nonce));

    const challengeData = {
      nonce: base64Nonce,
      expiresAt: Date.now() + 30000, // 30s from now
    };

    const challenge = parseChallenge(challengeData);

    expect(challenge.nonce).toEqual(nonce);
    expect(challenge.expiresAt).toBe(challengeData.expiresAt);
  });

  it('should reject invalid challenge format', () => {
    expect(() => parseChallenge({})).toThrow('Invalid challenge format');
    expect(() => parseChallenge({ nonce: 'abc' })).toThrow('Invalid challenge format');
    expect(() => parseChallenge({ expiresAt: 123 })).toThrow('Invalid challenge format');
  });

  it('should reject invalid nonce length', () => {
    const shortNonce = btoa('short');
    const challengeData = {
      nonce: shortNonce,
      expiresAt: Date.now() + 30000,
    };

    expect(() => parseChallenge(challengeData)).toThrow('Invalid nonce length');
  });

  it('should detect expired challenge', () => {
    const nonce = new Uint8Array(32);
    crypto.getRandomValues(nonce);

    const challenge = {
      nonce,
      expiresAt: Date.now() - 1000, // 1s in the past
    };

    expect(isChallengeExpired(challenge)).toBe(true);
  });

  it('should detect non-expired challenge', () => {
    const nonce = new Uint8Array(32);
    crypto.getRandomValues(nonce);

    const challenge = {
      nonce,
      expiresAt: Date.now() + 30000, // 30s in the future
    };

    expect(isChallengeExpired(challenge)).toBe(false);
  });

  it('should sign challenge correctly', async () => {
    const keypair = await generateKeypair();
    const nonce = new Uint8Array(32);
    crypto.getRandomValues(nonce);

    const challenge = {
      nonce,
      expiresAt: Date.now() + 30000,
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
      expiresAt: Date.now() - 1000, // Expired
    };

    await expect(
      signChallenge(challenge, keypair.privateKey, keypair.publicKey)
    ).rejects.toThrow('Challenge has expired');
  });

  it('should serialize challenge response to base64', async () => {
    const keypair = await generateKeypair();
    const nonce = new Uint8Array(32);
    crypto.getRandomValues(nonce);

    const challenge = {
      nonce,
      expiresAt: Date.now() + 30000,
    };

    const response = await signChallenge(challenge, keypair.privateKey, keypair.publicKey);
    const serialized = serializeChallengeResponse(response);

    expect(typeof serialized.signature).toBe('string');
    expect(typeof serialized.publicKey).toBe('string');

    // Verify we can decode it back
    const decodedSig = Uint8Array.from(atob(serialized.signature), c => c.charCodeAt(0));
    const decodedPubkey = Uint8Array.from(atob(serialized.publicKey), c => c.charCodeAt(0));

    expect(decodedSig).toEqual(response.signature);
    expect(decodedPubkey).toEqual(response.publicKey);
  });
});
