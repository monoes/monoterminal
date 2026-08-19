# Phase 1 Backend Implementation Plan

**Owner:** rust-backend-lead  
**Status:** Awaiting Architecture (task-1) and Repository Setup (task-2)  
**Target:** Windows + Web MVP per SRS v1.2 §7.1

---

## 1. Overview

This document details the implementation plan for Phase 1 Rust backend components:

- **Session Manager** (`crates/master/src/session.rs`) - Terminal session lifecycle, state machine, in-memory scrollback
- **WebSocket Server** (`crates/master/src/server.rs`) - TLS 1.3 server, Protocol Buffer framing, client management
- **Master Daemon** (`crates/master/src/main.rs`) - Windows Service integration, graceful shutdown, config loading
- **Integration Layer** (`crates/master/src/integration.rs`) - Wire up ConPTY ↔ Session ↔ WebSocket ↔ Protocol

---

## 2. Session Manager Implementation

**Module:** `crates/master/src/session/mod.rs`

### 2.1 Core Types

```rust
// session/types.rs
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Session identifier (UUID v4)
pub type SessionId = Uuid;

/// Client identifier (UUID v4)
pub type ClientId = Uuid;

/// Session state machine (SRS §2.1.3)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session created, PTY spawned, ready for clients
    Running,
    /// All clients detached, PTY still alive
    Detached,
    /// PTY terminated, cleanup pending
    Terminated,
}

/// Terminal session with ConPTY backend
pub struct Session {
    pub id: SessionId,
    pub state: SessionState,
    
    // PTY management
    pub pty_handle: PtyHandle,  // From rust-engineer-pty (task-10)
    pub shell_pid: u32,
    pub shell_type: String,
    
    // Terminal dimensions
    pub rows: u16,
    pub cols: u16,
    
    // Environment
    pub working_dir: PathBuf,
    pub environment: HashMap<String, String>,
    
    // In-memory scrollback (Phase 1: no SQLite)
    pub scrollback: RingBuffer<Line>,  // 10k lines capacity
    
    // Attached clients
    pub clients: Arc<RwLock<Vec<ClientHandle>>>,
    
    // Timestamps
    pub created_at: Instant,
    pub last_activity: Instant,
}

/// Ring buffer for scrollback (SRS §2.1.3: 10k lines)
pub struct RingBuffer<T> {
    buffer: Vec<T>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
            head: 0,
            len: 0,
        }
    }
    
    pub fn push(&mut self, item: T) {
        if self.len < self.capacity {
            self.buffer.push(item);
            self.len += 1;
        } else {
            self.buffer[self.head] = item;
            self.head = (self.head + 1) % self.capacity;
        }
    }
    
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        // Return items in chronological order
        self.buffer[self.head..].iter()
            .chain(self.buffer[..self.head].iter())
    }
}
```

### 2.2 Session Manager

```rust
// session/manager.rs
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Central session manager (singleton)
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SessionId, Arc<RwLock<Session>>>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Create new terminal session
    pub async fn create_session(
        &self,
        shell: String,
        working_dir: PathBuf,
        rows: u16,
        cols: u16,
    ) -> Result<SessionId, SessionError> {
        let id = Uuid::new_v4();
        
        // Spawn ConPTY (from rust-engineer-pty)
        let pty_handle = PtyManager::spawn_conpty(shell, working_dir, rows, cols)?;
        
        let session = Session {
            id,
            state: SessionState::Running,
            pty_handle,
            shell_pid: pty_handle.pid(),
            shell_type: shell,
            rows,
            cols,
            working_dir,
            environment: HashMap::new(),
            scrollback: RingBuffer::new(10_000),  // 10k lines
            clients: Arc::new(RwLock::new(Vec::new())),
            created_at: Instant::now(),
            last_activity: Instant::now(),
        };
        
        self.sessions.write().await.insert(id, Arc::new(RwLock::new(session)));
        
        Ok(id)
    }
    
    /// Attach client to existing session
    pub async fn attach_client(
        &self,
        session_id: SessionId,
        client: ClientHandle,
    ) -> Result<SessionSnapshot, SessionError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or(SessionError::NotFound)?;
        
        let mut session = session.write().await;
        
        // Add client
        session.clients.write().await.push(client);
        
        // Update state if was detached
        if session.state == SessionState::Detached {
            session.state = SessionState::Running;
        }
        
        // Return snapshot (scrollback + metadata)
        Ok(SessionSnapshot {
            id: session.id,
            scrollback: session.scrollback.iter().cloned().collect(),
            rows: session.rows,
            cols: session.cols,
            working_dir: session.working_dir.clone(),
        })
    }
    
    /// Detach client from session
    pub async fn detach_client(
        &self,
        session_id: SessionId,
        client_id: ClientId,
    ) -> Result<(), SessionError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or(SessionError::NotFound)?;
        
        let mut session = session.write().await;
        let mut clients = session.clients.write().await;
        
        clients.retain(|c| c.id != client_id);
        
        // If no clients remain, mark as detached
        if clients.is_empty() {
            session.state = SessionState::Detached;
        }
        
        Ok(())
    }
    
    /// Kill session and underlying PTY
    pub async fn kill_session(&self, session_id: SessionId) -> Result<(), SessionError> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .remove(&session_id)
            .ok_or(SessionError::NotFound)?;
        
        let mut session = session.write().await;
        session.state = SessionState::Terminated;
        
        // Kill PTY process
        session.pty_handle.kill()?;
        
        Ok(())
    }
}
```

### 2.3 PTY Output Fan-Out (Arc&lt;Bytes&gt; Pattern)

**Per SRS §3.1.4:** Zero-copy broadcast using reference-counted buffers.

```rust
// session/fanout.rs
use bytes::Bytes;
use std::sync::Arc;

/// PTY output reader task
pub async fn pty_output_loop(
    session: Arc<RwLock<Session>>,
    mut pty_output: PtyOutput,  // From rust-engineer-pty
) {
    let mut buffer = vec![0u8; 4096];
    
    loop {
        // Read from PTY (non-blocking via tokio)
        match pty_output.read(&mut buffer).await {
            Ok(n) if n > 0 => {
                let chunk = Bytes::copy_from_slice(&buffer[..n]);
                let shared_chunk = Arc::new(chunk);
                
                // Fan out to all clients (zero-copy)
                let session = session.read().await;
                let clients = session.clients.read().await;
                
                for client in clients.iter() {
                    // Arc::clone is cheap (atomic increment)
                    if let Err(e) = client.send(shared_chunk.clone()).await {
                        tracing::warn!("Client send failed: {}", e);
                    }
                }
                
                // Also append to scrollback
                drop(clients);
                drop(session);
                let mut session = session.write().await;
                session.scrollback.push(Line::from_bytes(&shared_chunk));
                session.last_activity = Instant::now();
            }
            Ok(_) => break,  // EOF
            Err(e) => {
                tracing::error!("PTY read error: {}", e);
                break;
            }
        }
    }
    
    // Mark session as terminated
    let mut session = session.write().await;
    session.state = SessionState::Terminated;
}
```

---

## 3. WebSocket Server Implementation

**Module:** `crates/master/src/server/mod.rs`

### 3.1 Core Server

```rust
// server/mod.rs
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use rustls::ServerConfig;
use std::sync::Arc;

/// WebSocket server with TLS 1.3 (SRS §3.1.2, §3.2.1)
pub struct WebSocketServer {
    bind_addr: String,
    tls_config: Arc<ServerConfig>,
    session_manager: Arc<SessionManager>,
    auth_manager: Arc<AuthManager>,  // From security-engineer (task-11)
}

impl WebSocketServer {
    pub fn new(
        bind_addr: String,
        tls_config: ServerConfig,
        session_manager: Arc<SessionManager>,
        auth_manager: Arc<AuthManager>,
    ) -> Self {
        Self {
            bind_addr,
            tls_config: Arc::new(tls_config),
            session_manager,
            auth_manager,
        }
    }
    
    /// Start server loop
    pub async fn run(&self) -> Result<(), ServerError> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        tracing::info!("WebSocket server listening on {}", self.bind_addr);
        
        loop {
            let (stream, peer_addr) = listener.accept().await?;
            
            let session_manager = self.session_manager.clone();
            let auth_manager = self.auth_manager.clone();
            let tls_config = self.tls_config.clone();
            
            // Spawn per-connection handler
            tokio::spawn(async move {
                if let Err(e) = handle_connection(
                    stream,
                    peer_addr,
                    tls_config,
                    session_manager,
                    auth_manager,
                ).await {
                    tracing::error!("Connection error from {}: {}", peer_addr, e);
                }
            });
        }
    }
}

/// Per-connection handler
async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    tls_config: Arc<ServerConfig>,
    session_manager: Arc<SessionManager>,
    auth_manager: Arc<AuthManager>,
) -> Result<(), ConnectionError> {
    // TLS handshake
    let tls_stream = rustls_stream::TlsStream::new(stream, tls_config).await?;
    
    // WebSocket upgrade
    let ws_stream = accept_async(tls_stream).await?;
    
    tracing::info!("WebSocket connection established from {}", peer_addr);
    
    // Handle client protocol
    handle_client(ws_stream, session_manager, auth_manager).await
}
```

### 3.2 Client Protocol Handler

```rust
// server/client.rs
use monoterminal_protocol::Envelope;  // From rust-engineer-protocol (task-9)
use prost::Message;

/// Handle client WebSocket messages
async fn handle_client(
    ws_stream: WebSocketStream,
    session_manager: Arc<SessionManager>,
    auth_manager: Arc<AuthManager>,
) -> Result<(), ClientError> {
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    
    let mut authenticated = false;
    let mut attached_session: Option<SessionId> = None;
    let client_id = Uuid::new_v4();
    
    while let Some(msg) = ws_receiver.next().await {
        let msg = msg?;
        
        match msg {
            Message::Binary(data) => {
                // Decode Protocol Buffer Envelope
                let envelope = Envelope::decode(&data[..])?;
                
                match envelope.message {
                    Some(envelope::Message::AttachRequest(req)) => {
                        // Authenticate (Ed25519 + JWT from security-engineer)
                        if !authenticated {
                            auth_manager.verify_jwt(&req.auth_token)?;
                            authenticated = true;
                        }
                        
                        // Attach to session
                        let session_id = if req.session_id.is_empty() {
                            // Create new session
                            session_manager.create_session(
                                "/bin/bash".to_string(),  // TODO: from config
                                PathBuf::from("/"),
                                req.rows as u16,
                                req.cols as u16,
                            ).await?
                        } else {
                            // Attach to existing
                            Uuid::parse_str(&req.session_id)?
                        };
                        
                        let snapshot = session_manager.attach_client(
                            session_id,
                            ClientHandle::new(client_id, ws_sender.clone()),
                        ).await?;
                        
                        attached_session = Some(session_id);
                        
                        // Send AttachResponse with scrollback
                        let response = Envelope {
                            sequence_number: 0,
                            message: Some(envelope::Message::AttachResponse(
                                AttachResponse {
                                    session_id: session_id.to_string(),
                                    metadata: Some(snapshot.metadata()),
                                    scrollback: snapshot.scrollback,
                                }
                            )),
                        };
                        
                        let mut buf = Vec::new();
                        response.encode(&mut buf)?;
                        ws_sender.send(Message::Binary(buf)).await?;
                    }
                    
                    Some(envelope::Message::InputData(input)) => {
                        if let Some(session_id) = attached_session {
                            // Forward input to PTY
                            session_manager.send_input(session_id, &input.data).await?;
                        }
                    }
                    
                    Some(envelope::Message::ResizeRequest(resize)) => {
                        if let Some(session_id) = attached_session {
                            session_manager.resize_session(
                                session_id,
                                resize.rows as u16,
                                resize.cols as u16,
                            ).await?;
                        }
                    }
                    
                    Some(envelope::Message::DetachRequest(_)) => {
                        if let Some(session_id) = attached_session {
                            session_manager.detach_client(session_id, client_id).await?;
                            break;
                        }
                    }
                    
                    _ => {
                        tracing::warn!("Unexpected message type");
                    }
                }
            }
            
            Message::Close(_) => break,
            _ => {}
        }
    }
    
    // Cleanup on disconnect
    if let Some(session_id) = attached_session {
        let _ = session_manager.detach_client(session_id, client_id).await;
    }
    
    Ok(())
}
```

### 3.3 Backpressure (SRS §3.1.4)

```rust
// server/backpressure.rs
use tokio::sync::mpsc;

/// Client handle with bounded queue (1MB backpressure)
pub struct ClientHandle {
    pub id: ClientId,
    sender: mpsc::Sender<Arc<Bytes>>,
}

impl ClientHandle {
    pub fn new(id: ClientId, ws_sender: WsSender) -> Self {
        let (tx, mut rx) = mpsc::channel::<Arc<Bytes>>(256);  // ~1MB at 4KB/msg
        
        // Spawn output task
        tokio::spawn(async move {
            while let Some(chunk) = rx.recv().await {
                let envelope = Envelope {
                    sequence_number: 0,  // TODO: track sequence
                    message: Some(envelope::Message::OutputData(
                        OutputData {
                            data: chunk.to_vec(),
                            sequence: 0,
                            compression: CompressionType::None as i32,
                        }
                    )),
                };
                
                let mut buf = Vec::new();
                if let Err(e) = envelope.encode(&mut buf) {
                    tracing::error!("Encode error: {}", e);
                    break;
                }
                
                if let Err(e) = ws_sender.send(Message::Binary(buf)).await {
                    tracing::error!("WebSocket send error: {}", e);
                    break;
                }
            }
        });
        
        Self { id, sender: tx }
    }
    
    /// Send output to client (non-blocking, drops if queue full)
    pub async fn send(&self, chunk: Arc<Bytes>) -> Result<(), SendError> {
        self.sender.try_send(chunk)
            .map_err(|e| match e {
                TrySendError::Full(_) => {
                    tracing::warn!("Client {} queue full, dropping output", self.id);
                    SendError::QueueFull
                }
                TrySendError::Closed(_) => SendError::Disconnected,
            })
    }
}
```

---

## 4. Master Daemon Binary

**Module:** `crates/master/src/main.rs`

### 4.1 Main Entry Point

```rust
// main.rs
use anyhow::Result;
use tokio::signal;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    tracing::info!("MONOTERMINAL master daemon starting...");
    tracing::info!("Phase 1: Windows + Web client");
    
    // Load configuration
    let config = Config::load()?;
    
    // Initialize components
    let session_manager = Arc::new(SessionManager::new());
    let auth_manager = Arc::new(AuthManager::new(config.auth)?);
    
    // TLS configuration (rustls)
    let tls_config = build_tls_config(&config.tls)?;
    
    // WebSocket server
    let server = WebSocketServer::new(
        config.server.bind_addr.clone(),
        tls_config,
        session_manager.clone(),
        auth_manager.clone(),
    );
    
    // Spawn server task
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.run().await {
            tracing::error!("Server error: {}", e);
        }
    });
    
    // Wait for shutdown signal
    signal::ctrl_c().await?;
    tracing::info!("Shutdown signal received, graceful shutdown...");
    
    // TODO: Graceful shutdown (SRS §2.1.3)
    // 1. Stop accepting new connections
    // 2. Wait for active sessions (timeout: 10s)
    // 3. Kill remaining sessions
    // 4. Flush state
    
    server_handle.abort();
    
    tracing::info!("Shutdown complete");
    Ok(())
}

/// Build rustls TLS 1.3 configuration (SRS §3.2.1)
fn build_tls_config(tls: &TlsConfig) -> Result<ServerConfig> {
    let certs = load_certs(&tls.cert_path)?;
    let key = load_private_key(&tls.key_path)?;
    
    let mut config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    
    // TLS 1.3 only (reject TLS 1.2)
    config.versions = vec![&rustls::version::TLS13];
    
    Ok(config)
}
```

### 4.2 Windows Service Integration

```rust
// service.rs (Windows-only)
#[cfg(windows)]
mod service {
    use windows::Win32::System::Services::*;
    use windows::core::*;
    
    /// Register as Windows Service (SRS §2.1.3)
    pub fn install_service() -> Result<()> {
        // TODO: Implement SERVICE_AUTO_START registration
        // - Create service with CreateService
        // - Set startup type to SERVICE_AUTO_START
        // - Configure failure actions
        unimplemented!("Windows Service installation pending architecture review")
    }
    
    /// Windows Service main function
    pub fn service_main() -> Result<()> {
        // TODO: Implement service control handler
        // - Register control handler (SERVICE_CONTROL_STOP, SERVICE_CONTROL_SHUTDOWN)
        // - Report SERVICE_RUNNING status
        // - Run main daemon loop
        // - Handle graceful shutdown
        unimplemented!("Windows Service main pending architecture review")
    }
}
```

---

## 5. Integration Flow

**Module:** `crates/master/src/integration.rs`

### 5.1 Data Flow

```
┌─────────────┐
│   Client    │
│ (Web/Mobile)│
└──────┬──────┘
       │ WebSocket + TLS 1.3
       │ (Protocol Buffer frames)
       ▼
┌─────────────────────────────────────┐
│      WebSocket Server               │
│  - TLS handshake (rustls)           │
│  - Auth (Ed25519 + JWT)             │
│  - Envelope decode (prost)          │
└──────┬──────────────────────────────┘
       │
       │ AttachRequest → create/attach
       │ InputData → write to PTY
       │ ResizeRequest → resize PTY
       ▼
┌─────────────────────────────────────┐
│     Session Manager                 │
│  - State machine (Running/Detached) │
│  - RingBuffer scrollback (10k)      │
│  - Client list (Arc<RwLock<Vec>>)   │
└──────┬──────────────────────────────┘
       │
       │ PTY spawn/resize/input
       │
       ▼
┌─────────────────────────────────────┐
│      PTY Manager (ConPTY)           │
│  - CreatePseudoConsole (Windows)    │
│  - CreateProcess + STARTUPINFOEX    │
│  - Async I/O (tokio)                │
└──────┬──────────────────────────────┘
       │
       │ Shell process I/O
       ▼
┌─────────────────────────────────────┐
│      Shell (cmd.exe, PowerShell)    │
└─────────────────────────────────────┘

Output Flow (reverse):
  Shell → PTY → Session (fan-out) → WebSocket → Clients
  (Arc<Bytes> zero-copy broadcast)
```

### 5.2 Integration Checklist

**After task-1 (Architecture) completes:**
- [ ] Review architecture decisions
- [ ] Brief rust-engineer-protocol on schema
- [ ] Brief rust-engineer-pty on ConPTY integration
- [ ] Finalize Session Manager API surface

**After task-9 (Protocol) completes:**
- [ ] Integrate Protocol Buffer types into WebSocket server
- [ ] Test Envelope encode/decode
- [ ] Implement sequence numbering

**After task-10 (ConPTY) completes:**
- [ ] Integrate PTY manager into Session Manager
- [ ] Test PTY spawn/resize/I/O
- [ ] Implement async output loop with fan-out

**After task-11 (Security) completes:**
- [ ] Integrate Auth Manager (Ed25519 + JWT)
- [ ] Test TLS 1.3 handshake
- [ ] Implement rate limiting

**Final Integration (task-18):**
- [ ] End-to-end testing: Client → WebSocket → Session → PTY → Shell
- [ ] Performance testing: 60 FPS rendering, <10ms latency
- [ ] Soak testing: 24-hour zero-crash test
- [ ] Resource testing: 7MB per session, 1000 concurrent sessions

---

## 6. Testing Strategy

### 6.1 Unit Tests

```rust
// session/tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_session_lifecycle() {
        let manager = SessionManager::new();
        
        // Create session
        let id = manager.create_session(
            "bash".to_string(),
            PathBuf::from("/tmp"),
            24, 80,
        ).await.unwrap();
        
        // Verify running state
        let session = manager.get_session(id).await.unwrap();
        assert_eq!(session.state, SessionState::Running);
        
        // Kill session
        manager.kill_session(id).await.unwrap();
    }
    
    #[test]
    fn test_ring_buffer() {
        let mut buffer = RingBuffer::new(3);
        
        buffer.push("line1");
        buffer.push("line2");
        buffer.push("line3");
        buffer.push("line4");  // Overwrites line1
        
        let lines: Vec<_> = buffer.iter().collect();
        assert_eq!(lines, vec!["line2", "line3", "line4"]);
    }
}
```

### 6.2 Integration Tests

```rust
// tests/integration_test.rs
#[tokio::test]
async fn test_end_to_end_session() {
    // 1. Start server
    // 2. Connect client (WebSocket + TLS)
    // 3. Send AttachRequest
    // 4. Send InputData ("echo hello\n")
    // 5. Receive OutputData (expect "hello")
    // 6. Send DetachRequest
    // 7. Verify session detached
}
```

### 6.3 Performance Tests

```rust
// benches/session_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_session_create(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let manager = SessionManager::new();
    
    c.bench_function("session_create", |b| {
        b.iter(|| {
            rt.block_on(async {
                manager.create_session(
                    black_box("bash".to_string()),
                    black_box(PathBuf::from("/")),
                    24, 80,
                ).await.unwrap()
            })
        })
    });
}

criterion_group!(benches, bench_session_create);
criterion_main!(benches);
```

---

## 7. Code Quality Gates

**Per rust-backend-lead responsibilities:**

### 7.1 Before Merge
- [ ] `cargo clippy -D warnings` (zero warnings)
- [ ] `cargo fmt --check` (formatting enforced)
- [ ] All unsafe blocks have documented safety comments
- [ ] Test coverage ≥70% (Phase 1 target per SRS §7.1)
- [ ] rust-backend-lead review of all unsafe/FFI code

### 7.2 Acceptance Criteria (SRS §7.1)
- [ ] 60 FPS master rendering on Windows 10 1809+
- [ ] <10ms local latency (measured)
- [ ] Zero crashes in 24-hour soak test
- [ ] 7MB per-session memory budget (measured)
- [ ] Web client connects from iPhone/Android browser successfully

---

## 8. Dependencies & Coordination

### 8.1 Blocked By
- **task-1** (Architecture) - Session/WebSocket design decisions
- **task-2** (Repository Setup) - ConPTY engineer can start
- **task-9** (Protocol Schema) - WebSocket integration
- **task-10** (ConPTY Integration) - Session Manager integration
- **task-11** (Security) - Auth Manager integration

### 8.2 Blocks
- **task-12** (Web Client) - Frontend gates on backend WebSocket API
- **task-14** (Test Strategy) - QA gates on backend implementation
- End-to-end testing gates on full backend integration

### 8.3 Coordination Points
- **security-engineer**: TLS 1.3 config, Ed25519/JWT integration
- **rust-engineer-protocol**: Protocol Buffer schema, code generation
- **rust-engineer-pty**: ConPTY API, async I/O integration
- **frontend-lead**: WebSocket API contract, Protocol Buffer types
- **monomind-integration-engineer**: Monomind bridge hooks

---

## 9. Risk Mitigation

### 9.1 Technical Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| **ConPTY API complexity** | High | rust-engineer-pty dedicated, rust-backend-lead reviews all unsafe |
| **TLS 1.3 config errors** | Medium | Use rustls safe defaults, test with ssl-lab scan |
| **WebSocket backpressure** | Medium | Implement bounded queues early, load test |
| **Windows Service bugs** | High | Defer to Phase 1 end, test on real Windows Service Manager |

### 9.2 Schedule Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| **Architecture task-1 delay** | High | Pre-brief team on SRS, start non-blocking prep work |
| **Protocol schema changes** | Medium | Version schema (v1), use prost feature flags |
| **Security integration delay** | Medium | Mock Auth Manager for early testing |

---

## 10. Next Actions

**Immediate (waiting on dependencies):**
1. ✅ Coordinate with protocol engineer (task-9 prep)
2. ✅ Coordinate with ConPTY engineer (task-10 prep)
3. ✅ Coordinate with principal-architect (architecture handoff)
4. ✅ Create task DAG entries (task-15, 16, 17, 18)

**When task-1 completes:**
1. Review architecture decisions
2. Brief rust-engineer-protocol on final schema
3. Brief rust-engineer-pty on final ConPTY approach
4. Start Session Manager implementation (task-15)
5. Start Master Daemon skeleton (task-17)

**When task-9 completes:**
1. Integrate Protocol Buffer types
2. Start WebSocket Server implementation (task-16)

**When task-10, 11 complete:**
1. Full integration (task-18)
2. End-to-end testing
3. Performance testing
4. Soak testing

---

**Document Status:** Draft, awaiting architecture (task-1)  
**Last Updated:** 2026-08-14  
**Owner:** rust-backend-lead
