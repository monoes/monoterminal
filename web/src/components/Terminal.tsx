import { useEffect, useRef, useState } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { WebglAddon } from '@xterm/addon-webgl';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import '@xterm/xterm/css/xterm.css';

interface TerminalProps {
  onData?: (data: string) => void;
  onResize?: (cols: number, rows: number) => void;
}

export function Terminal({ onData, onResize }: TerminalProps) {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const [useWebGL, setUseWebGL] = useState(true);

  useEffect(() => {
    if (!terminalRef.current) return;

    // Initialize xterm.js with WebGL addon
    const term = new XTerm({
      cursorBlink: true,
      fontFamily: 'Consolas, "Courier New", monospace',
      fontSize: 14,
      lineHeight: 1.2,
      theme: {
        background: '#1e1e1e',
        foreground: '#d4d4d4',
        cursor: '#d4d4d4',
        black: '#000000',
        red: '#cd3131',
        green: '#0dbc79',
        yellow: '#e5e510',
        blue: '#2472c8',
        magenta: '#bc3fbc',
        cyan: '#11a8cd',
        white: '#e5e5e5',
        brightBlack: '#666666',
        brightRed: '#f14c4c',
        brightGreen: '#23d18b',
        brightYellow: '#f5f543',
        brightBlue: '#3b8eea',
        brightMagenta: '#d670d6',
        brightCyan: '#29b8db',
        brightWhite: '#ffffff',
      },
      scrollback: 10000,
      allowProposedApi: true,
    });

    // Add fit addon
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    fitAddonRef.current = fitAddon;

    // Add web links addon
    term.loadAddon(new WebLinksAddon());

    // Try to load WebGL addon
    if (useWebGL) {
      try {
        const webglAddon = new WebglAddon();
        webglAddon.onContextLoss(() => {
          console.warn('WebGL context lost, falling back to Canvas');
          setUseWebGL(false);
        });
        term.loadAddon(webglAddon);
      } catch (e) {
        console.warn('WebGL not supported, using Canvas fallback', e);
        setUseWebGL(false);
      }
    }

    // Open terminal
    term.open(terminalRef.current);
    xtermRef.current = term;

    // Initial fit
    fitAddon.fit();

    // Handle data input
    if (onData) {
      term.onData(onData);
    }

    // Handle resize
    if (onResize) {
      term.onResize(({ cols, rows }) => {
        onResize(cols, rows);
      });
    }

    // Handle window resize
    const handleResize = () => {
      fitAddon.fit();
    };

    window.addEventListener('resize', handleResize);

    // Handle orientation change on mobile
    window.addEventListener('orientationchange', () => {
      setTimeout(handleResize, 100);
    });

    return () => {
      window.removeEventListener('resize', handleResize);
      term.dispose();
    };
  }, [onData, onResize, useWebGL]);

  // Public API for writing to terminal
  useEffect(() => {
    if (xtermRef.current) {
      (window as any).terminal = {
        write: (data: string) => xtermRef.current?.write(data),
        writeln: (data: string) => xtermRef.current?.writeln(data),
        clear: () => xtermRef.current?.clear(),
      };
    }
  }, []);

  return (
    <div
      ref={terminalRef}
      style={{
        width: '100%',
        height: '100%',
        padding: '4px',
        backgroundColor: '#1e1e1e',
      }}
    />
  );
}
