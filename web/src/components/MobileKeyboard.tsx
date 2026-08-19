import { useState } from 'react';
import './MobileKeyboard.css';

interface MobileKeyboardProps {
  onKey: (key: string) => void;
}

/**
 * On-screen accessory keyboard for mobile devices
 * Provides essential terminal keys: Esc, Tab, Ctrl, Alt, arrows
 * Per SRS §2.2 mobile keyboard requirements
 */
export function MobileKeyboard({ onKey }: MobileKeyboardProps) {
  const [ctrlPressed, setCtrlPressed] = useState(false);
  const [altPressed, setAltPressed] = useState(false);

  const handleKeyPress = (key: string) => {
    let modifiedKey = key;

    // Apply modifiers
    if (ctrlPressed) {
      // Ctrl+C = \x03, Ctrl+D = \x04, etc.
      const code = key.charCodeAt(0);
      if (code >= 65 && code <= 90) {
        // A-Z
        modifiedKey = String.fromCharCode(code - 64);
      } else if (code >= 97 && code <= 122) {
        // a-z
        modifiedKey = String.fromCharCode(code - 96);
      }
    }

    if (altPressed) {
      // Alt sends ESC prefix
      modifiedKey = '\x1b' + modifiedKey;
    }

    onKey(modifiedKey);

    // Reset modifiers after key press
    setCtrlPressed(false);
    setAltPressed(false);
  };

  const toggleCtrl = () => setCtrlPressed(!ctrlPressed);
  const toggleAlt = () => setAltPressed(!altPressed);

  return (
    <div className="mobile-keyboard">
      <div className="keyboard-row">
        {/* Modifier keys */}
        <button
          className={`key modifier ${ctrlPressed ? 'active' : ''}`}
          onClick={toggleCtrl}
          aria-label="Control"
        >
          Ctrl
        </button>
        <button
          className={`key modifier ${altPressed ? 'active' : ''}`}
          onClick={toggleAlt}
          aria-label="Alt"
        >
          Alt
        </button>

        {/* Essential keys */}
        <button className="key" onClick={() => handleKeyPress('\x1b')} aria-label="Escape">
          Esc
        </button>
        <button className="key" onClick={() => handleKeyPress('\t')} aria-label="Tab">
          Tab
        </button>

        {/* Arrow keys */}
        <button className="key arrow" onClick={() => handleKeyPress('\x1b[A')} aria-label="Up">
          ↑
        </button>
        <button className="key arrow" onClick={() => handleKeyPress('\x1b[B')} aria-label="Down">
          ↓
        </button>
        <button className="key arrow" onClick={() => handleKeyPress('\x1b[D')} aria-label="Left">
          ←
        </button>
        <button
          className="key arrow"
          onClick={() => handleKeyPress('\x1b[C')}
          aria-label="Right"
        >
          →
        </button>
      </div>
    </div>
  );
}
