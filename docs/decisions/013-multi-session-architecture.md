# ADR-013: Multi-Session Architecture

**Status:** Draft — Pending Phase 1 Gate  
**Date:** 2026-08-17  
**Deciders:** principal-architect, rust-backend-lead  
**SRS Reference:** §2.1.3, §7.2 (Multi-Session Management)  
**Phase:** Phase 2 (P2P + Persistence)

---

## Context

Phase 2 introduces multi-session management to enable:
- **Multiple terminal sessions per daemon** (no 1:1 daemon:session limit)
- **Session discovery** (list, filter, search active sessions)
- **Client multiplexing** (one WebSocket connection → many sessions)
- **Session lifecycle management** (create, attach, detach, terminate, persist)

**Current State (Phase 1):**
- **Single session per daemon** (daemon starts → creates one default session → client attaches)
- **No session routing** (all messages go to the single session)
- **No persistence** (session dies when daemon restarts)

**Phase 2 Requirements (SRS §7.2):**
- 100 concurrent sessions tested
- Multi-client attach (collaboration: multiple clients on one session)
- Session lifecycle: CREATE → RUNNING → DETACHED/TERMINATED (§2.1.3)

---

## Decision

Implement **multi-session architecture** with the following design:

---

## 1. Session Lifecycle State Machine

### 1.1 State Transitions

**Extended state machine (builds on SRS §2.1.3):**

```
                 ┌─────────────┐
                 │   CREATE    │ (SessionCreateRequest)
                 └──────┬──────┘
                        │
                        ▼
                 ┌─────────────┐
          ┌──────│   RUNNING   │◄──────┐
          │      └──────┬──────┘       │ (AttachRequest after DETACHED)
          │             │              │
          │             │ (All clients detach)
          │             ▼              │
          │      ┌─────────────┐       │
          │      │  DETACHED   │───────┘
          │      └──────┬──────┘
          │             │
          │             │ (TTL expires OR explicit terminate)
          │             ▼
          │      ┌─────────────┐
          └─────►│ TERMINATED  │
                 └─────────────┘
                        │
                        ▼
                 (Cleanup: PTY killed, resources freed)
```

**State descriptions:**

| State | PTY Status | Clients Attached | Persistence | Description |
|-------|------------|------------------|-------------|-------------|
| **CREATE** | Spawning | 0 | Not yet | Session requested, shell starting |
| **RUNNING** | Alive | 1+ | Yes (Phase 2) | Active session with ≥1 client |
| **DETACHED** | Alive | 0 | Yes | Background session (no clients, but PTY running) |
| **TERMINATED** | Dead | 0 | Archive only | Session exited, cleanup pending |

**New states (Phase 2 additions):**
- **CREATE:** Explicit creation phase (Phase 1 combined create+attach into one step)
- **DETACHED:** Background sessions (Phase 1 terminated on last client detach)

---

### 1.2 State Transition Rules

**RUNNING → DETACHED:**
```rust
impl SessionManager {
    async fn on_last_client_detach(&mut self, session_id: &str) -> Result<()> {
        let session = self.sessions.get_mut(session_id)?;
        
        if session.attached_clients.is_empty() {
            tracing::info!("Session {} detached (no clients), keeping PTY alive", session_id);
            
            session.state = SessionState::Detached;
            session.detached_at = Some(Instant::now());
            
            // Persist state to SQLite
            self.db.update_session_state(session_id, "DETACHED").await?;
            
            // Schedule TTL cleanup (default: 24 hours)
            self.schedule_detached_cleanup(session_id, Duration::from_secs(86400));
        }
        
        Ok(())
    }
}
```

**DETACHED → RUNNING:**
```rust
async fn on_attach_request(&mut self, req: AttachRequest) -> Result<AttachResponse> {
    let session = self.sessions.get_mut(&req.session_id)?;
    
    match session.state {
        SessionState::Detached => {
            tracing::info!("Reattaching to detached session {}", req.session_id);
            
            session.state = SessionState::Running;
            session.detached_at = None;
            
            // Cancel TTL cleanup timer
            self.cancel_detached_cleanup(&req.session_id);
            
            // Return scrollback since detachment
            let scrollback = self.get_scrollback_since(
                &req.session_id,
                session.last_seen_sequence,
            ).await?;
            
            Ok(AttachResponse { scrollback, ..})
        }
        SessionState::Running => {
            // Multi-client attach (Phase 2 collaboration feature)
            tracing::info!("Multi-client attach to running session {}", req.session_id);
            Ok(AttachResponse { .. })
        }
        SessionState::Terminated => {
            Err(Error::SessionTerminated)
        }
        _ => Err(Error::InvalidState),
    }
}
```

**DETACHED → TERMINATED (TTL expiry):**
```rust
async fn cleanup_detached_session(&mut self, session_id: &str) -> Result<()> {
    let session = self.sessions.get_mut(session_id)?;
    
    if session.state != SessionState::Detached {
        return Ok(()); // Already reattached, skip cleanup
    }
    
    let detached_duration = session.detached_at
        .map(|t| t.elapsed())
        .unwrap_or(Duration::ZERO);
    
    if detached_duration > Duration::from_secs(86400) {  // 24-hour TTL
        tracing::info!("Terminating detached session {} (TTL expired)", session_id);
        
        // Send SIGTERM to PTY process
        session.pty.terminate().await?;
        
        session.state = SessionState::Terminated;
        session.terminated_at = Some(Instant::now());
        
        // Archive to SQLite (status = TERMINATED)
        self.db.update_session_state(session_id, "TERMINATED").await?;
        
        // Remove from active sessions map
        self.sessions.remove(session_id);
    }
    
    Ok(())
}
```

**TTL configuration (Phase 2 default):**
- **24 hours:** Detached sessions cleaned up after 1 day
- **Configurable:** User can set per-session TTL via metadata: `{"ttl_seconds": 3600}` for 1-hour TTL
- **Never expire:** Set TTL = 0 (manual termination only)

---

## 2. Session Routing

### 2.1 Routing Table

**Problem:** One WebSocket connection → many sessions (how to route messages?)

**Solution:** Session ID in every message (already in protocol)

```rust
pub struct RoutingTable {
    /// Map: session_id → Session handle
    sessions: HashMap<String, Arc<Session>>,
    
    /// Map: client_id → Set of subscribed session_ids
    client_subscriptions: HashMap<String, HashSet<String>>,
}

impl RoutingTable {
    pub fn route_message(&self, envelope: Envelope, client_id: &str) -> Result<()> {
        match envelope.message {
            Some(Message::InputData(input)) => {
                // Extract session_id from InputData (implicit: client's current session)
                let session_id = self.get_client_current_session(client_id)?;
                let session = self.sessions.get(session_id)
                    .ok_or(Error::SessionNotFound)?;
                
                // Route input to PTY
                session.write_to_pty(input.data).await?;
            }
            Some(Message::AttachRequest(req)) => {
                // Subscribe client to session
                self.client_subscriptions
                    .entry(client_id.to_string())
                    .or_default()
                    .insert(req.session_id.clone());
                
                // Route attach to session
                let session = self.sessions.get(&req.session_id)?;
                session.attach_client(client_id, req).await?;
            }
            // ... other message types
        }
    }
}
```

**Routing strategies:**

| Message Type | Routing Key | Target |
|--------------|-------------|--------|
| **InputData** | Implicit (client's current session) | Single session |
| **AttachRequest** | Explicit (`session_id` field) | Single session |
| **ListSessionsRequest** | N/A | SessionManager (returns all sessions) |
| **OutputData** | Broadcast | All clients subscribed to session |

---

### 2.2 Client Multiplexing

**Use case:** Client wants to switch between sessions without closing WebSocket

```
Client                          Master
  │                               │
  │  AttachRequest(session-A) ────►│ Subscribe to session-A
  │◄────── AttachResponse ─────────┤ (OutputData from session-A flows)
  │                               │
  │  AttachRequest(session-B) ────►│ Subscribe to session-B (keep session-A)
  │◄────── AttachResponse ─────────┤ (OutputData from both sessions flows)
  │                               │
  │  DetachRequest(session-A) ────►│ Unsubscribe from session-A
  │                               │
  │  (Only session-B output flows)│
```

**Implementation:**

```rust
pub struct ClientState {
    client_id: String,
    active_sessions: HashSet<String>,  // Sessions client is attached to
    websocket: WebSocketSender,
    datachannel: Option<DataChannelSender>,
}

impl ClientState {
    pub async fn subscribe_to_session(&mut self, session_id: String) -> Result<()> {
        self.active_sessions.insert(session_id.clone());
        tracing::info!("Client {} subscribed to session {}", self.client_id, session_id);
        Ok(())
    }
    
    pub async fn unsubscribe_from_session(&mut self, session_id: &str) -> Result<()> {
        self.active_sessions.remove(session_id);
        tracing::info!("Client {} unsubscribed from session {}", self.client_id, session_id);
        Ok(())
    }
    
    pub async fn send_if_subscribed(&self, session_id: &str, envelope: Envelope) -> Result<()> {
        if self.active_sessions.contains(session_id) {
            self.websocket.send(envelope).await?;
        }
        Ok(())
    }
}
```

**UI pattern (web client):**
- **Tab switcher:** Each tab = one session (AttachRequest on tab open, DetachRequest on tab close)
- **Split panes:** Multiple visible sessions (Phase 4+, deferred)

---

## 3. Session Discovery & Filtering

### 3.1 List Sessions API

**Protocol message (per protocol-phase2-design.md §1.2):**

```protobuf
message ListSessionsRequest {
    optional string auth_token = 1;  // JWT for RBAC filtering (Phase 2+)
}

message ListSessionsResponse {
    repeated SessionSummary sessions = 1;
}

message SessionSummary {
    string session_id = 1;
    SessionMetadata metadata = 2;
    uint32 attached_clients = 3;     // Number of clients currently attached
    uint64 total_scrollback_lines = 4;
    bool is_active = 5;               // Has received input in last 5 minutes
}
```

**Implementation:**

```rust
impl SessionManager {
    pub async fn list_sessions(&self, user_id: Option<&str>) -> Result<Vec<SessionSummary>> {
        let sessions = self.sessions.values()
            .filter(|s| {
                // RBAC: Filter by ownership (Phase 2+)
                if let Some(uid) = user_id {
                    s.owner_user_id.as_deref() == Some(uid)
                } else {
                    true  // No auth: return all (Phase 1 compat)
                }
            })
            .filter(|s| {
                // Exclude terminated sessions
                s.state != SessionState::Terminated
            })
            .map(|s| SessionSummary {
                session_id: s.id.clone(),
                metadata: s.metadata.clone(),
                attached_clients: s.attached_clients.len() as u32,
                total_scrollback_lines: s.scrollback.total_lines(),
                is_active: s.last_input_at
                    .map(|t| t.elapsed() < Duration::from_secs(300))
                    .unwrap_or(false),
            })
            .collect();
        
        Ok(sessions)
    }
}
```

**Filtering criteria (client-side):**
- **By status:** Running, Detached
- **By activity:** Active (input <5min ago), Idle (input >5min ago)
- **By name:** Filter by `metadata.name` (user-provided session name)
- **By owner:** Filter by `owner_user_id` (Phase 2 RBAC)

---

### 3.2 Session Metadata

**Extensible metadata (per ADR-012 §2.1):**

```rust
pub struct SessionMetadata {
    pub name: Option<String>,          // User-friendly name: "build-server", "dev-shell"
    pub tags: Vec<String>,             // ["work", "linux", "project-A"]
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub custom: HashMap<String, String>,  // User-defined key-value pairs
}
```

**Example usage:**
```bash
# Create session with metadata
monoterminal session create --name "build-server" --tags "work,linux" --shell /bin/bash

# Filter sessions by tag
monoterminal session list --tag work
```

---

## 4. Resource Limits & Quotas

### 4.1 Per-Daemon Limits

**Problem:** Unbounded session creation → resource exhaustion (RAM, file descriptors)

**Solution:** Configurable limits

```rust
pub struct SessionLimits {
    max_sessions: usize,               // Default: 100 (SRS §7.2 acceptance)
    max_sessions_per_user: usize,      // Default: 10 (Phase 2 RBAC)
    max_scrollback_lines: usize,       // Default: 100k per session
    max_total_scrollback_mb: usize,    // Default: 1GB (across all sessions)
}

impl SessionManager {
    pub async fn create_session(&mut self, req: SessionCreateRequest) -> Result<String> {
        // Check global limit
        if self.sessions.len() >= self.limits.max_sessions {
            return Err(Error::QuotaExceeded("Max sessions reached"));
        }
        
        // Check per-user limit (Phase 2 RBAC)
        if let Some(user_id) = &req.user_id {
            let user_sessions = self.sessions.values()
                .filter(|s| s.owner_user_id.as_deref() == Some(user_id))
                .count();
            
            if user_sessions >= self.limits.max_sessions_per_user {
                return Err(Error::QuotaExceeded("User session limit reached"));
            }
        }
        
        // Create session
        let session = Session::spawn(req.shell_path, req.working_dir)?;
        self.sessions.insert(session.id.clone(), Arc::new(session));
        
        Ok(session.id)
    }
}
```

**Error handling:**
- `ErrorCode::SESSION_FULL` (per ADR-004 §6.2) returned to client
- Client shows: "Session limit reached. Close unused sessions or contact admin."

---

### 4.2 Scrollback Memory Management

**Problem:** 100 sessions × 100k lines × 200 bytes/line = 2 GB RAM

**Solution:** Hot/cold tiering (per ADR-012 §4.2)

```rust
pub struct Session {
    scrollback_hot: RingBuffer<Line>,   // Last 10k lines (RAM)
    scrollback_cold: ScrollbackHandle,  // Older lines (SQLite)
    scrollback_limit: usize,            // Max total lines (default: 100k)
}

impl Session {
    pub async fn append_output(&mut self, data: &[u8]) -> Result<()> {
        let line = Line::new(data, self.next_sequence);
        
        // Append to hot buffer
        self.scrollback_hot.push(line.clone());
        
        // Flush to cold storage if buffer full
        if self.scrollback_hot.len() >= 10_000 {
            let oldest = self.scrollback_hot.pop_front();
            self.scrollback_cold.append(oldest).await?;
        }
        
        // Enforce global scrollback limit
        if self.scrollback_cold.total_lines() + self.scrollback_hot.len() > self.scrollback_limit {
            self.scrollback_cold.delete_oldest(1000).await?;  // Drop oldest 1k lines
        }
        
        self.next_sequence += 1;
        Ok(())
    }
}
```

**Quota enforcement:**
- Drop oldest lines when limit reached (acceptable for Phase 2 MVP)
- Phase 3: Warn user before dropping (UI notification: "Scrollback limit reached")

---

## 5. Session Creation Flow

### 5.1 Protocol Flow

**New in Phase 2:** Explicit session creation (separate from attach)

```
Client                          Master
  │                               │
  │  SessionCreateRequest ────────►│
  │  {shell: "cmd.exe",            │
  │   working_dir: "C:\\Users",    │
  │   metadata: {name: "shell-1"}} │
  │                               │
  │◄────── SessionCreateResponse ──┤
  │  {session_id: "uuid-1234"}    │
  │                               │
  │  AttachRequest ────────────────►│
  │  {session_id: "uuid-1234"}    │
  │                               │
  │◄────── AttachResponse ─────────┤
  │  {scrollback: [...]}          │
  │                               │
  │  (Terminal I/O flows)         │
```

**Phase 1 compatibility:**
- Phase 1 clients: Send AttachRequest without session_id → Master auto-creates default session
- Phase 2 clients: Explicitly create session first, then attach

---

### 5.2 Default Session Behavior

**Backward compatibility:**

```rust
impl SessionManager {
    pub async fn handle_attach_request(&mut self, req: AttachRequest) -> Result<AttachResponse> {
        let session_id = if req.session_id.is_empty() {
            // Phase 1 client: Create default session
            let default_shell = get_default_shell();  // cmd.exe on Windows
            let default_dir = env::var("USERPROFILE").unwrap_or("C:\\".into());
            
            let create_req = SessionCreateRequest {
                shell_path: default_shell,
                working_dir: default_dir,
                metadata: SessionMetadata {
                    name: Some("default".into()),
                    ..Default::default()
                },
                ..Default::default()
            };
            
            self.create_session(create_req).await?
        } else {
            // Phase 2 client: Use provided session_id
            req.session_id.clone()
        };
        
        // Attach to session (shared code path)
        self.attach_to_session(&session_id, req).await
    }
}
```

---

## 6. Testing Strategy

### 6.1 State Machine Tests

```rust
#[tokio::test]
async fn test_session_lifecycle() {
    let mut mgr = SessionManager::new();
    
    // CREATE
    let id = mgr.create_session(SessionCreateRequest { .. }).await?;
    assert_eq!(mgr.get_session(&id)?.state, SessionState::Running);
    
    // DETACH (all clients leave)
    mgr.detach_client(&id, "client-1").await?;
    assert_eq!(mgr.get_session(&id)?.state, SessionState::Detached);
    
    // REATTACH
    mgr.attach_to_session(&id, AttachRequest { .. }).await?;
    assert_eq!(mgr.get_session(&id)?.state, SessionState::Running);
    
    // TERMINATE
    mgr.terminate_session(&id).await?;
    assert_eq!(mgr.get_session(&id)?.state, SessionState::Terminated);
}

#[tokio::test]
async fn test_detached_ttl_cleanup() {
    let mut mgr = SessionManager::new();
    let id = mgr.create_session(SessionCreateRequest { .. }).await?;
    
    // Detach session
    mgr.detach_client(&id, "client-1").await?;
    
    // Fast-forward time (mock)
    advance_time(Duration::from_secs(86400 + 1));  // 24h + 1s
    
    // Cleanup task runs
    mgr.cleanup_detached_sessions().await?;
    
    // Session should be terminated
    assert!(mgr.get_session(&id).is_err());
}
```

---

### 6.2 Concurrency Stress Tests

```rust
#[tokio::test]
async fn test_100_concurrent_sessions() {
    let mgr = Arc::new(Mutex::new(SessionManager::new()));
    
    // Spawn 100 sessions concurrently
    let handles: Vec<_> = (0..100).map(|i| {
        let mgr = mgr.clone();
        tokio::spawn(async move {
            let id = mgr.lock().await.create_session(SessionCreateRequest {
                shell_path: "cmd.exe".into(),
                working_dir: format!("C:\\test{}", i),
                ..Default::default()
            }).await?;
            
            // Attach client
            mgr.lock().await.attach_to_session(&id, AttachRequest { .. }).await?;
            
            // Send 1000 lines of input
            for j in 0..1000 {
                mgr.lock().await.send_input(&id, format!("echo {}\n", j).as_bytes()).await?;
            }
            
            Result::<_, Error>::Ok(id)
        })
    }).collect();
    
    // Wait for all sessions to complete
    let results = futures::future::try_join_all(handles).await?;
    assert_eq!(results.len(), 100);
}
```

**Performance targets (SRS §7.2):**
- 100 concurrent sessions: RAM <500 MB, CPU <50%
- Session creation latency: <100ms p95
- Session attach latency: <50ms p95

---

## Consequences

### Positive
- ✅ Unlimited sessions per daemon (no 1:1 limit)
- ✅ Background sessions (DETACHED state, reconnect later)
- ✅ Client multiplexing (switch sessions without reconnecting)
- ✅ Resource quotas prevent exhaustion

### Negative
- ⚠️ Complexity increase (state machine, routing, quotas)
- ⚠️ TTL cleanup adds background task overhead
- ⚠️ Per-client subscription tracking adds memory

### Neutral
- Session discovery adds protocol message type (already in Phase 2 schema)
- Backward compatibility maintained (Phase 1 clients auto-create default session)

---

## References

- **ADR-004:** Protocol Schema Evolution (SessionCreateRequest, ListSessionsRequest)
- **ADR-012:** Persistence Layer (sessions table, state persistence)
- **SRS §2.1.3:** Session Lifecycle State Machine
- **SRS §7.2:** Phase 2 acceptance (100 concurrent sessions)

---

## Follow-up Actions

1. ⏳ **Pending Phase 1 gate passage** (Friday 5/7 threshold)
2. ⏳ **Approve ADR-013** (eng-director, rust-backend-lead review)
3. ⏳ **Implement SessionManager** (rust-backend-lead, Week 1-3)
4. ⏳ **Stress test 100 concurrent sessions** (performance-engineer, Week 4)
5. ⏳ **UI mockups for session switcher** (frontend-lead, Week 2)

---

**Next:** ADR-014 (Collaboration Primitives)
