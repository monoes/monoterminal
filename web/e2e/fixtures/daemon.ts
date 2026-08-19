/**
 * Playwright Test Fixture: Master Daemon Management
 *
 * Automatically launches monoterminal-master.exe before tests and cleans up after.
 * Ensures each test has a fresh daemon instance on port 8080.
 */

import { test as base } from '@playwright/test';
import { spawn, ChildProcess } from 'child_process';
import { join } from 'path';

type DaemonFixture = {
  daemon: ChildProcess;
};

export const test = base.extend<DaemonFixture>({
  daemon: async ({}, use) => {
    // Path to built master daemon (relative to web/ directory)
    const daemonPath = join('..', 'target', 'release', 'monoterminal-master.exe');

    console.log('[Daemon Fixture] Starting master daemon...');

    // Launch master daemon with test configuration
    const daemon = spawn(daemonPath, [
      '--port', '8080',
      '--log-level', 'debug',
      '--data-dir', '.test-data',  // Isolated test data
      '--dev-mode',  // Skip auth verification for E2E tests (security-engineer approved)
    ], {
      stdio: ['ignore', 'pipe', 'pipe'],
      env: {
        ...process.env,
        RUST_LOG: 'monoterminal=debug',
        MONOTERMINAL_ENV: 'test',
      },
    });

    // Capture daemon output for debugging
    daemon.stdout?.on('data', (data) => {
      console.log(`[Daemon STDOUT] ${data.toString().trim()}`);
    });

    daemon.stderr?.on('data', (data) => {
      console.error(`[Daemon STDERR] ${data.toString().trim()}`);
    });

    // Handle daemon exit during tests
    daemon.on('exit', (code, signal) => {
      if (code !== null && code !== 0) {
        console.error(`[Daemon Fixture] Master daemon exited with code ${code}`);
      } else if (signal) {
        console.log(`[Daemon Fixture] Master daemon killed with signal ${signal}`);
      }
    });

    // Wait for daemon startup (check health endpoint or wait fixed time)
    // TODO: Replace with actual health check once implemented
    await new Promise(resolve => setTimeout(resolve, 3000));

    console.log('[Daemon Fixture] Daemon ready, proceeding with test...');

    // Provide daemon to test
    await use(daemon);

    // Cleanup: Kill daemon after test
    console.log('[Daemon Fixture] Test complete, stopping daemon...');

    daemon.kill('SIGTERM');

    // Wait for graceful shutdown
    await new Promise<void>((resolve) => {
      const timeout = setTimeout(() => {
        console.warn('[Daemon Fixture] Graceful shutdown timeout, force killing...');
        daemon.kill('SIGKILL');
        resolve();
      }, 5000);

      daemon.on('exit', () => {
        clearTimeout(timeout);
        resolve();
      });
    });

    console.log('[Daemon Fixture] Daemon stopped');
  },
});

export { expect } from '@playwright/test';
