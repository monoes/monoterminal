import { describe, expect, it } from 'vitest';
import { toRelayWsUrl } from './accounts-client';

describe('toRelayWsUrl', () => {
  it('converts https to wss', () => {
    expect(toRelayWsUrl('https://accounts.example.com')).toBe('wss://accounts.example.com');
  });

  it('converts http to ws', () => {
    expect(toRelayWsUrl('http://localhost:9000')).toBe('ws://localhost:9000');
  });

  it('strips trailing slashes before converting', () => {
    expect(toRelayWsUrl('https://accounts.example.com/')).toBe('wss://accounts.example.com');
  });
});
