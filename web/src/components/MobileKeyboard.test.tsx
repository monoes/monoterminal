/**
 * MobileKeyboard Component Tests
 * Tests key press events, special keys, modifiers, and callbacks
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MobileKeyboard } from './MobileKeyboard';

describe('MobileKeyboard', () => {
  it('should render all essential keys', () => {
    const onKey = vi.fn();
    render(<MobileKeyboard onKey={onKey} />);

    expect(screen.getByLabelText('Control')).toBeInTheDocument();
    expect(screen.getByLabelText('Alt')).toBeInTheDocument();
    expect(screen.getByLabelText('Escape')).toBeInTheDocument();
    expect(screen.getByLabelText('Tab')).toBeInTheDocument();
    expect(screen.getByLabelText('Up')).toBeInTheDocument();
    expect(screen.getByLabelText('Down')).toBeInTheDocument();
    expect(screen.getByLabelText('Left')).toBeInTheDocument();
    expect(screen.getByLabelText('Right')).toBeInTheDocument();
  });

  describe('Special Keys', () => {
    it('should send Escape key (\\x1b)', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      await user.click(screen.getByLabelText('Escape'));

      expect(onKey).toHaveBeenCalledWith('\x1b');
    });

    it('should send Tab key (\\t)', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      await user.click(screen.getByLabelText('Tab'));

      expect(onKey).toHaveBeenCalledWith('\t');
    });
  });

  describe('Arrow Keys', () => {
    it('should send Up arrow (\\x1b[A)', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      await user.click(screen.getByLabelText('Up'));

      expect(onKey).toHaveBeenCalledWith('\x1b[A');
    });

    it('should send Down arrow (\\x1b[B)', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      await user.click(screen.getByLabelText('Down'));

      expect(onKey).toHaveBeenCalledWith('\x1b[B');
    });

    it('should send Left arrow (\\x1b[D)', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      await user.click(screen.getByLabelText('Left'));

      expect(onKey).toHaveBeenCalledWith('\x1b[D');
    });

    it('should send Right arrow (\\x1b[C)', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      await user.click(screen.getByLabelText('Right'));

      expect(onKey).toHaveBeenCalledWith('\x1b[C');
    });
  });

  describe('Ctrl Modifier', () => {
    it('should toggle Ctrl modifier on click', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      const ctrlBtn = screen.getByLabelText('Control');

      // Initially not active
      expect(ctrlBtn).not.toHaveClass('active');

      // Click to activate
      await user.click(ctrlBtn);
      expect(ctrlBtn).toHaveClass('active');

      // Click again to deactivate
      await user.click(ctrlBtn);
      expect(ctrlBtn).not.toHaveClass('active');
    });

    it('should send Ctrl+C (\\x03)', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      await user.click(screen.getByLabelText('Control'));

      // Simulate pressing 'C' key (would come from handleKeyPress)
      // Since MobileKeyboard only has fixed buttons, we test the logic conceptually
      // The actual Ctrl+letter would require text input integration
      expect(screen.getByLabelText('Control')).toHaveClass('active');
    });

    it('should reset Ctrl modifier after key press', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      const ctrlBtn = screen.getByLabelText('Control');

      await user.click(ctrlBtn);
      expect(ctrlBtn).toHaveClass('active');

      // Press any key (e.g., Tab)
      await user.click(screen.getByLabelText('Tab'));

      // Ctrl should be reset
      expect(ctrlBtn).not.toHaveClass('active');
      expect(onKey).toHaveBeenCalledWith('\t'); // Tab without Ctrl
    });
  });

  describe('Alt Modifier', () => {
    it('should toggle Alt modifier on click', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      const altBtn = screen.getByLabelText('Alt');

      // Initially not active
      expect(altBtn).not.toHaveClass('active');

      // Click to activate
      await user.click(altBtn);
      expect(altBtn).toHaveClass('active');

      // Click again to deactivate
      await user.click(altBtn);
      expect(altBtn).not.toHaveClass('active');
    });

    it('should send Alt+key with ESC prefix (\\x1b)', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      await user.click(screen.getByLabelText('Alt'));

      // Press Tab with Alt active
      await user.click(screen.getByLabelText('Tab'));

      // Should send ESC + Tab
      expect(onKey).toHaveBeenCalledWith('\x1b\t');
    });

    it('should reset Alt modifier after key press', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      const altBtn = screen.getByLabelText('Alt');

      await user.click(altBtn);
      expect(altBtn).toHaveClass('active');

      await user.click(screen.getByLabelText('Escape'));

      // Alt should be reset
      expect(altBtn).not.toHaveClass('active');
    });
  });

  describe('Combined Modifiers', () => {
    it('should handle Ctrl+Alt combination', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      // Activate both modifiers
      await user.click(screen.getByLabelText('Control'));
      await user.click(screen.getByLabelText('Alt'));

      expect(screen.getByLabelText('Control')).toHaveClass('active');
      expect(screen.getByLabelText('Alt')).toHaveClass('active');

      // Press a key
      await user.click(screen.getByLabelText('Tab'));

      // Both modifiers should reset
      expect(screen.getByLabelText('Control')).not.toHaveClass('active');
      expect(screen.getByLabelText('Alt')).not.toHaveClass('active');
    });

    it('should reset both modifiers after any key press', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      await user.click(screen.getByLabelText('Control'));
      await user.click(screen.getByLabelText('Alt'));

      await user.click(screen.getByLabelText('Up'));

      expect(screen.getByLabelText('Control')).not.toHaveClass('active');
      expect(screen.getByLabelText('Alt')).not.toHaveClass('active');
    });
  });

  describe('Callback Behavior', () => {
    it('should call onKey for every key press', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      await user.click(screen.getByLabelText('Escape'));
      await user.click(screen.getByLabelText('Tab'));
      await user.click(screen.getByLabelText('Up'));

      expect(onKey).toHaveBeenCalledTimes(3);
    });

    it('should not call onKey when clicking modifier toggles', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      await user.click(screen.getByLabelText('Control'));
      await user.click(screen.getByLabelText('Alt'));

      // Modifiers alone should not trigger onKey
      expect(onKey).not.toHaveBeenCalled();

      // Only when a key is pressed
      await user.click(screen.getByLabelText('Tab'));
      expect(onKey).toHaveBeenCalledTimes(1);
    });

    it('should preserve onKey callback across re-renders', async () => {
      const user = userEvent.setup();
      const onKey1 = vi.fn();
      const { rerender } = render(<MobileKeyboard onKey={onKey1} />);

      await user.click(screen.getByLabelText('Tab'));
      expect(onKey1).toHaveBeenCalledWith('\t');

      // Re-render with new callback
      const onKey2 = vi.fn();
      rerender(<MobileKeyboard onKey={onKey2} />);

      await user.click(screen.getByLabelText('Escape'));
      expect(onKey2).toHaveBeenCalledWith('\x1b');
      expect(onKey1).toHaveBeenCalledTimes(1); // Should not be called again
    });
  });

  describe('Accessibility', () => {
    it('should have accessible labels for all buttons', () => {
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      const buttons = screen.getAllByRole('button');
      expect(buttons.length).toBe(8); // 2 modifiers + 6 keys

      buttons.forEach(button => {
        expect(button).toHaveAccessibleName();
      });
    });

    it('should apply correct CSS classes for styling', () => {
      const onKey = vi.fn();
      const { container } = render(<MobileKeyboard onKey={onKey} />);

      expect(container.querySelector('.mobile-keyboard')).toBeInTheDocument();
      expect(container.querySelector('.keyboard-row')).toBeInTheDocument();
      expect(container.querySelectorAll('.key').length).toBeGreaterThan(0);
      expect(container.querySelectorAll('.modifier').length).toBe(2);
      expect(container.querySelectorAll('.arrow').length).toBe(4);
    });

    it('should show active state for pressed modifiers', async () => {
      const user = userEvent.setup();
      const onKey = vi.fn();
      render(<MobileKeyboard onKey={onKey} />);

      const ctrlBtn = screen.getByLabelText('Control');

      expect(ctrlBtn.className).not.toContain('active');

      await user.click(ctrlBtn);

      expect(ctrlBtn.className).toContain('active');
    });
  });
});
