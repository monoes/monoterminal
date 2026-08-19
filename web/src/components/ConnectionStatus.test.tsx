/**
 * ConnectionStatus Component Tests
 * Tests state indicator rendering and reconnect button visibility
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ConnectionStatus } from './ConnectionStatus';
import { ConnectionState } from '../lib/websocket-client';

describe('ConnectionStatus', () => {
  describe('State Indicators', () => {
    it('should display CONNECTED state correctly', () => {
      const { container } = render(
        <ConnectionStatus state={ConnectionState.CONNECTED} />
      );

      expect(screen.getByText('Connected')).toBeInTheDocument();
      expect(screen.getByText('●')).toBeInTheDocument(); // Connected icon
      expect(container.querySelector('.connected')).toBeInTheDocument();
    });

    it('should display CONNECTING state correctly', () => {
      const { container } = render(
        <ConnectionStatus state={ConnectionState.CONNECTING} />
      );

      expect(screen.getByText('Connecting...')).toBeInTheDocument();
      expect(screen.getByText('○')).toBeInTheDocument(); // Connecting icon
      expect(container.querySelector('.connecting')).toBeInTheDocument();
    });

    it('should display RECONNECTING state correctly', () => {
      const { container } = render(
        <ConnectionStatus state={ConnectionState.RECONNECTING} />
      );

      expect(screen.getByText('Reconnecting...')).toBeInTheDocument();
      expect(screen.getByText('◐')).toBeInTheDocument(); // Reconnecting icon
      expect(container.querySelector('.reconnecting')).toBeInTheDocument();
    });

    it('should display DISCONNECTED state correctly', () => {
      const { container } = render(
        <ConnectionStatus state={ConnectionState.DISCONNECTED} />
      );

      expect(screen.getByText('Disconnected')).toBeInTheDocument();
      expect(screen.getByText('○')).toBeInTheDocument(); // Disconnected icon
      expect(container.querySelector('.disconnected')).toBeInTheDocument();
    });

    it('should display ERROR state correctly', () => {
      const { container } = render(
        <ConnectionStatus state={ConnectionState.ERROR} />
      );

      expect(screen.getByText('Connection Error')).toBeInTheDocument();
      expect(screen.getByText('✗')).toBeInTheDocument(); // Error icon
      expect(container.querySelector('.error')).toBeInTheDocument();
    });
  });

  describe('Reconnect Button', () => {
    it('should show reconnect button when DISCONNECTED and onReconnect provided', () => {
      const onReconnect = vi.fn();
      render(
        <ConnectionStatus
          state={ConnectionState.DISCONNECTED}
          onReconnect={onReconnect}
        />
      );

      expect(screen.getByText('Reconnect')).toBeInTheDocument();
    });

    it('should show reconnect button when ERROR and onReconnect provided', () => {
      const onReconnect = vi.fn();
      render(
        <ConnectionStatus state={ConnectionState.ERROR} onReconnect={onReconnect} />
      );

      expect(screen.getByText('Reconnect')).toBeInTheDocument();
    });

    it('should NOT show reconnect button when CONNECTED', () => {
      const onReconnect = vi.fn();
      render(
        <ConnectionStatus state={ConnectionState.CONNECTED} onReconnect={onReconnect} />
      );

      expect(screen.queryByText('Reconnect')).not.toBeInTheDocument();
    });

    it('should NOT show reconnect button when CONNECTING', () => {
      const onReconnect = vi.fn();
      render(
        <ConnectionStatus state={ConnectionState.CONNECTING} onReconnect={onReconnect} />
      );

      expect(screen.queryByText('Reconnect')).not.toBeInTheDocument();
    });

    it('should NOT show reconnect button when RECONNECTING', () => {
      const onReconnect = vi.fn();
      render(
        <ConnectionStatus
          state={ConnectionState.RECONNECTING}
          onReconnect={onReconnect}
        />
      );

      expect(screen.queryByText('Reconnect')).not.toBeInTheDocument();
    });

    it('should NOT show reconnect button when onReconnect is not provided', () => {
      render(<ConnectionStatus state={ConnectionState.DISCONNECTED} />);

      expect(screen.queryByText('Reconnect')).not.toBeInTheDocument();
    });

    it('should call onReconnect when button is clicked', async () => {
      const user = userEvent.setup();
      const onReconnect = vi.fn();
      render(
        <ConnectionStatus
          state={ConnectionState.DISCONNECTED}
          onReconnect={onReconnect}
        />
      );

      await user.click(screen.getByText('Reconnect'));

      expect(onReconnect).toHaveBeenCalledTimes(1);
    });
  });

  describe('State Transitions', () => {
    it('should update display when state changes', () => {
      const { rerender } = render(
        <ConnectionStatus state={ConnectionState.CONNECTING} />
      );

      expect(screen.getByText('Connecting...')).toBeInTheDocument();

      rerender(<ConnectionStatus state={ConnectionState.CONNECTED} />);

      expect(screen.getByText('Connected')).toBeInTheDocument();
      expect(screen.queryByText('Connecting...')).not.toBeInTheDocument();
    });

    it('should show/hide reconnect button based on state change', () => {
      const onReconnect = vi.fn();
      const { rerender } = render(
        <ConnectionStatus
          state={ConnectionState.CONNECTED}
          onReconnect={onReconnect}
        />
      );

      expect(screen.queryByText('Reconnect')).not.toBeInTheDocument();

      rerender(
        <ConnectionStatus
          state={ConnectionState.DISCONNECTED}
          onReconnect={onReconnect}
        />
      );

      expect(screen.getByText('Reconnect')).toBeInTheDocument();
    });
  });

  describe('CSS Classes', () => {
    it('should apply correct wrapper class', () => {
      const { container } = render(
        <ConnectionStatus state={ConnectionState.CONNECTED} />
      );

      expect(container.querySelector('.connection-status')).toBeInTheDocument();
    });

    it('should apply status-icon and status-text classes', () => {
      const { container } = render(
        <ConnectionStatus state={ConnectionState.CONNECTED} />
      );

      expect(container.querySelector('.status-icon')).toBeInTheDocument();
      expect(container.querySelector('.status-text')).toBeInTheDocument();
    });

    it('should apply reconnect-btn class to button', () => {
      const onReconnect = vi.fn();
      const { container } = render(
        <ConnectionStatus
          state={ConnectionState.ERROR}
          onReconnect={onReconnect}
        />
      );

      expect(container.querySelector('.reconnect-btn')).toBeInTheDocument();
    });
  });
});
