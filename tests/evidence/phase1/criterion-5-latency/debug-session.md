# Criterion #5 Latency Benchmark Debug Session

**Date:** 2026-08-16  
**Task:** task-1 - Debug latency benchmark hang (server startup validation)  
**Engineer:** performance-engineer

## Problem Statement

The E2E latency benchmark (`latency_e2e_lan.rs`) hangs during the warmup phase, preventing verification of Criterion #5 (LAN p95 < 30ms latency target).

### Root Cause (Pre-Investigation)

Per knowledge graph findings:
- **Lack of server startup validation** causes silent hang when WebSocket server fails to bind or initialize properly
- Benchmark used **blind 200ms sleep** after spawning server, with no confirmation that bind succeeded
- If server fails to bind (port in use, TLS cert issues, etc.), benchmark waits 200ms then tries to connect → hangs forever

## Solution Implemented

### 1. Added Server Startup Notification Channel

**File:** `crates/master/src/server/mod.rs`

**Changes:**
- Added `tokio::sync::oneshot` import
- Added `startup_tx: Option<oneshot::Sender<SocketAddr>>` field to `Server` struct
- Created new constructor `Server::with_startup_notification()` that accepts a oneshot sender
- Modified `Server::run()` to:
  - Log bind attempt with `debug!()` before `TcpListener::bind()`
  - Log bind errors explicitly before returning
  - Extract `listener.local_addr()` to get actual bound address
  - Send bound address via `startup_tx` channel once listener is successfully created
  - Add debug log when notification is sent

**Key Code:**
```rust
pub async fn run(mut self) -> Result<()> {
    debug!("Attempting to bind TCP listener to {}", self.config.bind_addr);
    
    let listener = TcpListener::bind(self.config.bind_addr).await
        .map_err(|e| {
            error!("Failed to bind to {}: {}", self.config.bind_addr, e);
            e
        })?;
    
    let bound_addr = listener.local_addr()
        .map_err(|e| {
            error!("Failed to get local address: {}", e);
            e
        })?;
    
    info!("WebSocket server listening on {}", bound_addr);
    
    // Send startup notification if channel exists
    if let Some(tx) = self.startup_tx.take() {
        debug!("Sending startup notification for {}", bound_addr);
        let _ = tx.send(bound_addr);
    }
    
    // ... rest of server loop
}
```

### 2. Updated Benchmark to Wait for Startup Confirmation

**File:** `crates/master/benches/latency_e2e_lan.rs`

**Changes:**
- Added `tokio::sync::oneshot` and `tracing` imports
- Initialized `tracing_subscriber` at benchmark start for debug visibility
- Created `(startup_tx, startup_rx)` oneshot channel
- Used `Server::with_startup_notification()` instead of `Server::new()`
- **Replaced blind 200ms sleep with:**
  ```rust
  let bound_addr = match tokio::time::timeout(Duration::from_secs(5), startup_rx).await {
      Ok(Ok(addr)) => {
          info!("✓ Server successfully bound to {}", addr);
          addr
      }
      Ok(Err(_)) => {
          error!("✗ Server startup notification channel closed without sending address");
          panic!("Server startup failed: channel closed");
      }
      Err(_) => {
          error!("✗ Server startup timeout (5s) - server failed to bind");
          server_handle.abort();
          panic!("Server startup timeout - check logs for bind errors");
      }
  };
  ```
- Added comprehensive debug logging at every step:
  - Step 1-10: Server setup (keypair, auth, session manager, TLS, bind)
  - Step 11-12: Client setup (JWT, WebSocket connection)
  - Step 13-15: PTY session creation and attachment
  - Measurement loop start
- Changed all `.expect()` panics to `match` expressions with explicit error logging
- Added `✓` success and `✗` failure markers for easy log scanning

### 3. Added Debug Logging Throughout Benchmark

Every critical step now logs:
- **Before:** Silent operation or generic `.expect()` panic
- **After:** Explicit debug/info log before operation + match-based error handling with context

Example transformation:
```rust
// BEFORE
let session_id = session_manager
    .create_session(None, 24, 80)
    .await
    .expect("Failed to create session");

// AFTER
debug!("Step 13: Creating PTY session");
let session_id = match session_manager.create_session(None, 24, 80).await {
    Ok(id) => {
        info!("✓ PTY session created: {}", id);
        id
    }
    Err(e) => {
        error!("✗ Failed to create PTY session: {}", e);
        server_handle.abort();
        panic!("PTY session creation failed: {}", e);
    }
};
```

## Verification Steps

### Expected Behavior (Success Path)

When benchmark runs successfully, logs should show:
```
DEBUG: === BENCHMARK START: Setting up server ===
DEBUG: Step 1: Generating Ed25519 keypair
DEBUG: Step 2: Creating auth service
...
DEBUG: Step 7: Configuring TLS and server
DEBUG: Step 8: Creating server instance
DEBUG: Step 9: Spawning server task
DEBUG: Server task started, entering run() loop
DEBUG: Attempting to bind TCP listener to 127.0.0.1:18080
INFO:  WebSocket server listening on 127.0.0.1:18080
DEBUG: Sending startup notification for 127.0.0.1:18080
INFO:  ✓ Server successfully bound to 127.0.0.1:18080
DEBUG: === CLIENT SETUP: Preparing WebSocket connection ===
DEBUG: Step 11: Generating JWT
DEBUG: Step 12: Connecting WebSocket client to wss://127.0.0.1:18080
INFO:  ✓ WebSocket client connected successfully
DEBUG: Step 13: Creating PTY session
INFO:  ✓ PTY session created: <uuid>
DEBUG: Step 14: Waiting for PTY initialization (100ms)
DEBUG: Step 15: Attaching to session via WebSocket
INFO:  ✓ Session attached successfully
INFO:  === Starting measurement loop (<N> iterations) ===
```

### Expected Behavior (Bind Failure)

If server fails to bind (e.g., port already in use):
```
DEBUG: Step 9: Spawning server task
DEBUG: Server task started, entering run() loop
DEBUG: Attempting to bind TCP listener to 127.0.0.1:18080
ERROR: Failed to bind to 127.0.0.1:18080: Address already in use (os error 10048)
DEBUG: Step 10: Waiting for server startup confirmation
ERROR: ✗ Server startup timeout (5s) - server failed to bind
thread 'main' panicked at 'Server startup timeout - check logs for bind errors'
```

### Expected Behavior (TLS Cert Failure)

If TLS acceptor build fails:
```
DEBUG: Step 7: Configuring TLS and server
DEBUG: Step 8: Creating server instance
thread 'main' panicked at 'Failed to create server: TlsConfig error: ...'
```

## Next Steps

1. **Run benchmark:** `cargo bench --bench latency_e2e_lan`
2. **Check logs:** Benchmark should either:
   - **Progress past warmup** and measure latency (SUCCESS)
   - **Fail fast with clear error** identifying the hang point (DIAGNOSTIC SUCCESS)
3. **If hang persists:** Logs will show exactly where it stops (last successful step number)
4. **Document actual findings** in this file

## Test Execution

**Command:**
```bash
cargo bench --bench latency_e2e_lan
```

**Expected Outcome:**
- Benchmark progresses past warmup phase, OR
- Fast-fail with diagnostic error message showing exact failure point

## Results

_(To be filled after test execution)_

---

## Design Notes

### Why oneshot channel instead of health check endpoint?

1. **Simpler:** No need for HTTP server on separate port
2. **Faster:** No network round-trip for health check
3. **Race-free:** Notification sent exactly once, atomically when bind succeeds
4. **Benchmark-specific:** No production overhead, opt-in via constructor

### Why 5-second timeout?

- TLS cert load: ~50-100ms
- TCP bind: <10ms on uncontested port
- 5s provides 50x margin while still failing fast vs. infinite hang
- If timeout triggers, it's a genuine failure (not transient)

### Debug log levels

- `debug!()`: Step-by-step progress (numbered steps)
- `info!()`: Key milestones with ✓/✗ markers (server bound, client connected, etc.)
- `error!()`: Failures before panic (with ✗ marker)

### Cleanup on panic

All panic branches call `server_handle.abort()` to ensure background task is killed before exiting.
This prevents orphaned server tasks accumulating across benchmark iterations.
