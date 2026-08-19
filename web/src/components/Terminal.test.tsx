/**
 * Terminal Component Tests
 * Tests xterm.js initialization, resize handling, data callbacks, cleanup
 *
 * Note: Full xterm.js addon testing requires canvas support.
 * These tests verify the component structure and callback handling.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '@testing-library/react';
import { Terminal } from './Terminal';

// Mock xterm.js and addons
const mockTerm = {
  open: vi.fn(),
  loadAddon: vi.fn(),
  dispose: vi.fn(),
  onData: vi.fn((callback) => {
    mockTerm._dataCallback = callback;
  }),
  onResize: vi.fn((callback) => {
    mockTerm._resizeCallback = callback;
  }),
  write: vi.fn(),
  writeln: vi.fn(),
  clear: vi.fn(),
  _dataCallback: null as ((data: string) => void) | null,
  _resizeCallback: null as ((size: { cols: number; rows: number }) => void) | null,
};

const mockFitAddon = {
  fit: vi.fn(),
};

const mockWebglAddon = {
  onContextLoss: vi.fn(),
};

vi.mock('@xterm/xterm', () => ({
  Terminal: vi.fn(() => mockTerm),
}));

vi.mock('@xterm/addon-fit', () => ({
  FitAddon: vi.fn(() => mockFitAddon),
}));

vi.mock('@xterm/addon-webgl', () => ({
  WebglAddon: vi.fn(() => mockWebglAddon),
}));

vi.mock('@xterm/addon-web-links', () => ({
  WebLinksAddon: vi.fn(() => ({})),
}));

describe('Terminal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockTerm._dataCallback = null;
    mockTerm._resizeCallback = null;
  });

  it('should render terminal container', () => {
    const { container } = render(<Terminal />);
    const terminalDiv = container.firstChild as HTMLElement;

    expect(terminalDiv).toBeTruthy();
    expect(terminalDiv.style.width).toBe('100%');
    expect(terminalDiv.style.height).toBe('100%');
  });

  it('should call onData callback when terminal receives input', () => {
    const onData = vi.fn();
    render(<Terminal onData={onData} />);

    expect(mockTerm.onData).toHaveBeenCalledWith(onData);
  });

  it('should call onResize callback when terminal is resized', () => {
    const onResize = vi.fn();
    render(<Terminal onResize={onResize} />);

    expect(mockTerm.onResize).toHaveBeenCalled();

    // Simulate resize event
    if (mockTerm._resizeCallback) {
      mockTerm._resizeCallback({ cols: 80, rows: 24 });
      expect(onResize).toHaveBeenCalledWith(80, 24);
    }
  });

  it('should initialize terminal and fit addon', () => {
    render(<Terminal />);

    expect(mockTerm.open).toHaveBeenCalled();
    expect(mockTerm.loadAddon).toHaveBeenCalled();
    expect(mockFitAddon.fit).toHaveBeenCalled();
  });

  it('should dispose terminal on unmount', () => {
    const { unmount } = render(<Terminal />);

    expect(mockTerm.dispose).not.toHaveBeenCalled();

    unmount();

    expect(mockTerm.dispose).toHaveBeenCalled();
  });

  it('should expose public API on window object', () => {
    render(<Terminal />);

    expect((window as any).terminal).toBeDefined();
    expect((window as any).terminal.write).toBeDefined();
    expect((window as any).terminal.writeln).toBeDefined();
    expect((window as any).terminal.clear).toBeDefined();
  });

  it('should handle window resize events', () => {
    render(<Terminal />);

    const initialFitCount = mockFitAddon.fit.mock.calls.length;

    window.dispatchEvent(new Event('resize'));

    expect(mockFitAddon.fit.mock.calls.length).toBeGreaterThan(initialFitCount);
  });

  it('should handle orientation change with delay', () => {
    vi.useFakeTimers();

    render(<Terminal />);

    const initialFitCount = mockFitAddon.fit.mock.calls.length;

    window.dispatchEvent(new Event('orientationchange'));

    // Should not call fit immediately
    expect(mockFitAddon.fit.mock.calls.length).toBe(initialFitCount);

    // Should call fit after 100ms delay
    vi.advanceTimersByTime(100);
    expect(mockFitAddon.fit.mock.calls.length).toBeGreaterThan(initialFitCount);

    vi.useRealTimers();
  });
});
