# Task-4: PTY Async I/O Runtime - Implementation Plan

**Owner:** rust-backend-lead  
**Status:** READY - Awaiting Dependencies (task-1, task-3)  
**SRS References:** §2.1.4 (Networking Layer), §3.1.4 (Output Buffering & Flow Control)

---

## Current State Analysis

### Existing Implementation
Located in `crates/master/src/session/manager.rs:225-265` (`pty_output_loop`)

**What's Already There:**
- ✅ Basic tokio async loop structure
- ✅ 4KB buffer allocation per SRS §3.1.4
- ✅ PTY read call (async trait method)
- ✅ EOF detection (n == 0 → terminate)
- ✅ Scrollback integration
- ✅ Session termination on PTY close

**What's Missing (TODOs in code):**
- ❌ Line 251: Arc<Bytes> zero-copy fan-out to clients
- ❌ Flush triggers (≥4KB, 100ms timeout, newline detection)
- ❌ Client broadcast channel integration
- ❌ Backpressure handling (1MB per-client bounded queue)
- ❌ VT sequence parsing (deferred, but needed for line detection)

---

## Implementation Requirements

### 1. Arc<Bytes> Zero-Copy Fan-Out (SRS §3.1.4)

**Pattern from SRS:**
```rust
// 1→N broadcast (single read, N writes)
let chunk = pty_read(4096)?;  // Read once from PTY
let bytes = Arc::new(chunk);  // Zero-copy Arc

for client in session.clients.iter() {
    client.send(bytes.clone())?;  // Reference-counted, no copy
}
```

**CPU Savings:** 40-60% vs N×1 (per-client PTY reads)

**Implementation Plan:**
1. Change buffer ownership to use `bytes::Bytes` crate
2. Wrap read chunk in `Arc<Bytes>`
3. Integrate with tokio broadcast channel (task-3 dependency)
4. Each client gets `Arc::clone()` - no memcpy

### 2. Flush Triggers (SRS §3.1.4)

**Three Trigger Conditions:**

| Trigger | Threshold | Action | Rationale |
|---------|-----------|--------|-----------|
| **Size** | Buffer ≥4KB | Flush immediately | Maximize throughput |
| **Time** | 100ms elapsed | Flush partial buffer | Interactive responsiveness |
| **Newline** | Detect `\n` | Flush | Line-buffered output |

**Implementation Plan:**
1. Add `last_flush: Instant` tracking
2. Check `buffer.len() >= 4096` → immediate flush
3. Use `tokio::time::interval(100ms)` select branch
4. Scan chunk for `\n` byte → flush on detection

### 3. Client Broadcast Channel

**Design:**
- Use `tokio::sync::broadcast::channel<Arc<Bytes>>`
- Channel capacity: 256 messages (4KB each = 1MB buffer)
- Each client gets a `broadcast::Receiver`
- Slow client detection: receiver lag > 80% capacity

**Integration Points:**
- `SessionManager::attach_client()` - create receiver for new client
- `pty_output_loop()` - broadcast each chunk
- Client connection handler (task-3) - consume receiver

### 4. Backpressure Handling (SRS §3.1.4)

**Per-Client Buffer:** 1 MB bounded queue

**Slow Client Detection:**
- Buffer >80% full for >5s → log warning
- Buffer 100% full → drop oldest output (per SRS)

**Implementation:**
- Broadcast channel naturally handles this via `recv()` lagging
- Use `try_recv()` to detect lag in client handler (task-3 side)
- No PTY read throttling (read as fast as PTY produces per SRS)

---

## Architecture Integration

### Data Flow
```
PTY (ConPTY) → read(4KB) → Arc<Bytes> → broadcast → N × Client Receivers
                                      ↓
                                  Scrollback
```

### Dependencies

**Requires from task-1 (ConPTY Implementation):**
- Working `PtyBackend::read()` async method
- Reliable EOF detection
- Process lifecycle management

**Requires from task-3 (Protocol Runtime):**
- Client connection tracking
- WebSocket frame sender integration
- Protocol buffer encoding pipeline

**Provides to task-5 (Session Manager):**
- Functional output streaming
- Client fan-out infrastructure
- Backpressure foundation

---

## Performance Targets (SRS §5.1.1)

| Metric | Target | Verification |
|--------|--------|--------------|
| **PTY Read Latency** | <10ms local | `pty_output_loop` iteration time |
| **Fan-out Overhead** | 40-60% CPU savings vs N×1 | Benchmark 10 clients vs 10 read loops |
| **Memory per Session** | 7MB budget | 4KB buffer + 1MB scrollback + client queues |
| **Throughput** | Saturate PTY | No artificial throttling |

---

## Code Changes Checklist

### File: `crates/master/src/session/manager.rs`

- [ ] Add `bytes` crate dependency to `Cargo.toml`
- [ ] Add `tokio::sync::broadcast` channel to `Session` struct
- [ ] Implement flush trigger logic in `pty_output_loop`:
  - [ ] Size trigger (≥4KB)
  - [ ] Time trigger (100ms interval)
  - [ ] Newline detection
- [ ] Replace TODO line 251 with Arc<Bytes> broadcast
- [ ] Add slow client detection logging
- [ ] Update `attach_client()` to create broadcast receiver

### File: `crates/master/src/session/session.rs`

- [ ] Add `broadcast::Sender<Arc<Bytes>>` field to `Session`
- [ ] Initialize broadcast channel in `Session::new()`
- [ ] Add method: `fn output_sender(&self) -> broadcast::Receiver<Arc<Bytes>>`

---

## Testing Strategy (Deferred to task-15/task-16)

**Unit Tests:**
- Flush trigger conditions (size/time/newline)
- Arc refcount behavior (verify zero-copy)
- Buffer wraparound edge cases

**Integration Tests:**
- Multi-client fan-out (10 clients, same output)
- Slow client lag detection
- Late-joiner scrollback sync

**Performance Tests:**
- `cat large.log` throughput
- Fan-out CPU usage (1 client vs 10 clients)
- Memory stability under load

---

## Risks & Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| VT parsing overhead | >5ms per chunk | Defer parsing to Phase 2, use byte scan for `\n` only |
| Broadcast channel deadlock | Session hang | Use `try_send()` with drop-oldest fallback |
| Arc clone overhead | CPU regression | Benchmark vs memcpy, verify 40-60% savings claim |

---

## Open Questions for Dependencies

**For rust-engineer-pty (task-1):**
- Does `ConPtyBackend::read()` return exactly on 4KB boundaries or variable?
- What's the typical read latency on Windows ConPTY?
- Any Windows-specific buffering quirks to handle?

**For rust-engineer-protocol (task-3):**
- How should `broadcast::Receiver` integrate with WebSocket sender?
- Should protocol layer handle OutputData compression or session layer?
- Client ID assignment - before or after attach?

---

## References

- SRS §2.1.4: Networking Layer (Arc<Bytes> pattern)
- SRS §3.1.4: Output Buffering & Flow Control (flush triggers)
- SRS §5.1.1: Performance targets (<10ms latency, 7MB memory)
- Architecture: `docs/architecture/phase1-overview.md` §2 (Session Management)

---

**Status:** READY to implement when task-1 + task-3 complete.  
**Estimated Duration:** 1 day (per task graph)  
**Next Task:** task-5 (Session Manager Runtime - builds on this)
