/**
 * InstallPrompt Component Tests
 * Tests engagement heuristics, beforeinstallprompt event handling, and user interactions
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { InstallPrompt } from './InstallPrompt';

describe('InstallPrompt', () => {
  beforeEach(() => {
    localStorage.clear();

    // Mock matchMedia
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockImplementation(query => ({
        matches: false,
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });

    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    localStorage.clear();
  });

  describe('Engagement Tracking', () => {
    it('should increment visit count in localStorage', () => {
      render(<InstallPrompt />);

      expect(localStorage.getItem('monoterminal-visit-count')).toBe('1');
    });

    it('should track engagement time', async () => {
      render(<InstallPrompt />);

      await act(async () => {
        vi.advanceTimersByTime(30000);
      });

      const engagementTime = parseInt(
        localStorage.getItem('monoterminal-engagement-time') || '0',
        10
      );
      expect(engagementTime).toBeGreaterThan(0);
    });

    it('should update engagement on cleanup', () => {
      const { unmount } = render(<InstallPrompt />);

      act(() => {
        vi.advanceTimersByTime(15000);
      });

      unmount();

      const engagement = parseInt(
        localStorage.getItem('monoterminal-engagement-time') || '0',
        10
      );
      expect(engagement).toBeGreaterThan(0);
    });
  });

  describe('beforeinstallprompt Event Handling', () => {
    it('should capture beforeinstallprompt event', async () => {
      render(<InstallPrompt />);

      const mockDeferredPrompt = {
        prompt: vi.fn().mockResolvedValue(undefined),
        userChoice: Promise.resolve({ outcome: 'accepted' }),
        preventDefault: vi.fn(),
      };

      await act(async () => {
        const event = new Event('beforeinstallprompt') as any;
        Object.assign(event, mockDeferredPrompt);
        window.dispatchEvent(event);
      });

      expect(mockDeferredPrompt.preventDefault).toHaveBeenCalled();
    });

    it('should NOT show prompt if visit threshold not met', async () => {
      localStorage.setItem('monoterminal-visit-count', '1');
      localStorage.setItem('monoterminal-engagement-time', String(6 * 60 * 1000));

      render(<InstallPrompt />);

      const mockDeferredPrompt = {
        prompt: vi.fn().mockResolvedValue(undefined),
        userChoice: Promise.resolve({ outcome: 'accepted' }),
        preventDefault: vi.fn(),
      };

      await act(async () => {
        const event = new Event('beforeinstallprompt') as any;
        Object.assign(event, mockDeferredPrompt);
        window.dispatchEvent(event);
      });

      expect(screen.queryByText('Install MONOTERMINAL')).not.toBeInTheDocument();
    });

    it('should NOT show prompt if engagement threshold not met', async () => {
      localStorage.setItem('monoterminal-visit-count', '3');
      localStorage.setItem('monoterminal-engagement-time', String(2 * 60 * 1000));

      render(<InstallPrompt />);

      const mockDeferredPrompt = {
        prompt: vi.fn().mockResolvedValue(undefined),
        userChoice: Promise.resolve({ outcome: 'accepted' }),
        preventDefault: vi.fn(),
      };

      await act(async () => {
        const event = new Event('beforeinstallprompt') as any;
        Object.assign(event, mockDeferredPrompt);
        window.dispatchEvent(event);
      });

      expect(screen.queryByText('Install MONOTERMINAL')).not.toBeInTheDocument();
    });

    it('should show prompt when both thresholds are met', async () => {
      localStorage.setItem('monoterminal-visit-count', '2');
      localStorage.setItem('monoterminal-engagement-time', String(5 * 60 * 1000));

      render(<InstallPrompt />);

      const mockDeferredPrompt = {
        prompt: vi.fn().mockResolvedValue(undefined),
        userChoice: Promise.resolve({ outcome: 'accepted' }),
        preventDefault: vi.fn(),
      };

      await act(async () => {
        const event = new Event('beforeinstallprompt') as any;
        Object.assign(event, mockDeferredPrompt);
        window.dispatchEvent(event);
      });

      await waitFor(() => {
        expect(screen.getByText('Install MONOTERMINAL')).toBeInTheDocument();
      });
    });

    it('should NOT show prompt if previously dismissed', async () => {
      localStorage.setItem('monoterminal-visit-count', '3');
      localStorage.setItem('monoterminal-engagement-time', String(10 * 60 * 1000));
      localStorage.setItem('monoterminal-install-dismissed', 'true');

      render(<InstallPrompt />);

      const mockDeferredPrompt = {
        prompt: vi.fn().mockResolvedValue(undefined),
        userChoice: Promise.resolve({ outcome: 'accepted' }),
        preventDefault: vi.fn(),
      };

      await act(async () => {
        const event = new Event('beforeinstallprompt') as any;
        Object.assign(event, mockDeferredPrompt);
        window.dispatchEvent(event);
      });

      expect(screen.queryByText('Install MONOTERMINAL')).not.toBeInTheDocument();
    });
  });

  describe('User Interactions', () => {
    it('should handle install button click', async () => {
      const user = userEvent.setup({ delay: null });
      localStorage.setItem('monoterminal-visit-count', '3');
      localStorage.setItem('monoterminal-engagement-time', String(10 * 60 * 1000));

      render(<InstallPrompt />);

      const mockDeferredPrompt = {
        prompt: vi.fn().mockResolvedValue(undefined),
        userChoice: Promise.resolve({ outcome: 'accepted' }),
        preventDefault: vi.fn(),
      };

      await act(async () => {
        const event = new Event('beforeinstallprompt') as any;
        Object.assign(event, mockDeferredPrompt);
        window.dispatchEvent(event);
      });

      await waitFor(() => {
        expect(screen.getByText('Install Now')).toBeInTheDocument();
      });

      await user.click(screen.getByText('Install Now'));

      expect(mockDeferredPrompt.prompt).toHaveBeenCalled();
    });

    it('should hide prompt on "Maybe Later" click', async () => {
      const user = userEvent.setup({ delay: null });
      localStorage.setItem('monoterminal-visit-count', '3');
      localStorage.setItem('monoterminal-engagement-time', String(10 * 60 * 1000));

      render(<InstallPrompt />);

      const mockDeferredPrompt = {
        prompt: vi.fn().mockResolvedValue(undefined),
        userChoice: Promise.resolve({ outcome: 'dismissed' }),
        preventDefault: vi.fn(),
      };

      await act(async () => {
        const event = new Event('beforeinstallprompt') as any;
        Object.assign(event, mockDeferredPrompt);
        window.dispatchEvent(event);
      });

      await waitFor(() => {
        expect(screen.getByText('Maybe Later')).toBeInTheDocument();
      });

      await user.click(screen.getByText('Maybe Later'));

      await waitFor(() => {
        expect(screen.queryByText('Install MONOTERMINAL')).not.toBeInTheDocument();
      });

      expect(localStorage.getItem('monoterminal-install-dismissed')).toBeNull();
    });

    it('should permanently dismiss on close button click', async () => {
      const user = userEvent.setup({ delay: null });
      localStorage.setItem('monoterminal-visit-count', '3');
      localStorage.setItem('monoterminal-engagement-time', String(10 * 60 * 1000));

      render(<InstallPrompt />);

      const mockDeferredPrompt = {
        prompt: vi.fn().mockResolvedValue(undefined),
        userChoice: Promise.resolve({ outcome: 'dismissed' }),
        preventDefault: vi.fn(),
      };

      await act(async () => {
        const event = new Event('beforeinstallprompt') as any;
        Object.assign(event, mockDeferredPrompt);
        window.dispatchEvent(event);
      });

      await waitFor(() => {
        expect(screen.getByLabelText('Dismiss')).toBeInTheDocument();
      });

      await user.click(screen.getByLabelText('Dismiss'));

      await waitFor(() => {
        expect(screen.queryByText('Install MONOTERMINAL')).not.toBeInTheDocument();
      });

      expect(localStorage.getItem('monoterminal-install-dismissed')).toBe('true');
    });
  });

  describe('Rendering', () => {
    it('should render null when prompt is not shown', () => {
      const { container } = render(<InstallPrompt />);

      expect(container.firstChild).toBeNull();
    });

    it('should display feature benefits when shown', async () => {
      localStorage.setItem('monoterminal-visit-count', '3');
      localStorage.setItem('monoterminal-engagement-time', String(10 * 60 * 1000));

      render(<InstallPrompt />);

      const mockDeferredPrompt = {
        prompt: vi.fn().mockResolvedValue(undefined),
        userChoice: Promise.resolve({ outcome: 'accepted' }),
        preventDefault: vi.fn(),
      };

      await act(async () => {
        const event = new Event('beforeinstallprompt') as any;
        Object.assign(event, mockDeferredPrompt);
        window.dispatchEvent(event);
      });

      await waitFor(() => {
        expect(screen.getByText(/Launch directly from your desktop/)).toBeInTheDocument();
        expect(screen.getByText(/Native window controls/)).toBeInTheDocument();
        expect(screen.getByText(/Offline app shell/)).toBeInTheDocument();
      });
    });
  });
});
