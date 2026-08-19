// Session Manager - central coordinator for all terminal sessions
// Phase 1: Single-session support (multi-session in Phase 2)
// SRS §2.1.3, Architecture §2

use bytes::Bytes;
use prost::Message;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use super::{
    Result, Session, SessionContainer, SessionError, SessionId, SessionSnapshot, SessionState,
};
use crate::auth::{check_permission, Action};
use crate::persistence::{session as db_session, Database};
use crate::pty::{PtyBackend, PtyConfig};

/// Central session manager
/// Phase 1: Single active session (simplified from SRS multi-session design)
pub struct SessionManager {
    /// Active sessions (Option A: SessionContainer with separated locks)
    sessions: Arc<RwLock<HashMap<SessionId, SessionContainer>>>,

    /// Default shell (pwsh.exe if available, else cmd.exe per architecture)
    default_shell: String,

    /// Persistence layer (Phase 2: SQLite session + scrollback storage)
    db: Option<Arc<Database>>,
}

impl SessionManager {
    /// Create new session manager
    pub fn new(default_shell: Option<String>) -> Self {
        Self::new_with_db(default_shell, None)
    }

    /// Create new session manager with optional persistence
    pub fn new_with_db(default_shell: Option<String>, db: Option<Arc<Database>>) -> Self {
        let default_shell = default_shell.unwrap_or_else(|| {
            // DIAGNOSTIC TEST: Use ping instead of cmd.exe to match Microsoft ConPTY sample
            // This verifies our ConPTY implementation is correct
            // TODO: Revert to cmd.exe after test
            "ping.exe -n 3 localhost".to_string()
        });

        tracing::info!(
            "SessionManager initialized with default shell: {}",
            default_shell
        );

        // Phase 2: Cold-start recovery - clean up stale sessions
        // Since PTY processes don't survive restarts, mark orphaned sessions as TERMINATED
        if let Some(db) = &db {
            if let Ok(conn) = db.get_conn() {
                match db_session::list_active_sessions(&conn) {
                    Ok(active_sessions) => {
                        tracing::info!(
                            "Found {} orphaned sessions from previous run, marking as TERMINATED",
                            active_sessions.len()
                        );
                        for session in active_sessions {
                            if let Err(e) = db_session::update_session_status(
                                &conn,
                                &session.session_id,
                                db_session::SessionStatus::Terminated,
                            ) {
                                tracing::warn!(
                                    "Failed to terminate orphaned session {}: {}",
                                    session.session_id,
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => tracing::warn!("Failed to load active sessions for cleanup: {}", e),
                }
            }
        }

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            default_shell,
            db,
        }
    }

    /// Create new terminal session (backward-compatible, no RBAC)
    ///
    /// For tests and non-auth scenarios. Production code should use `create_session_with_user()`.
    pub async fn create_session(
        &self,
        working_dir: Option<PathBuf>,
        rows: u16,
        cols: u16,
    ) -> Result<SessionId> {
        self.create_session_with_user(None, working_dir, rows, cols)
            .await
    }

    /// Create new terminal session with optional owner user_id (RBAC-enabled)
    ///
    /// Phase 1: Spawns ConPTY backend
    /// Phase 2+: Will support OpenPTY for Linux/macOS
    ///
    /// # Arguments
    /// * `owner_user_id` - User creating the session (from JWT claims, optional)
    /// * `working_dir` - Working directory for shell
    /// * `rows` - Terminal rows
    /// * `cols` - Terminal columns
    pub async fn create_session_with_user(
        &self,
        owner_user_id: Option<String>,
        working_dir: Option<PathBuf>,
        rows: u16,
        cols: u16,
    ) -> Result<SessionId> {
        // Validate dimensions
        if rows == 0 || cols == 0 || rows > 500 || cols > 500 {
            return Err(SessionError::InvalidDimensions(rows, cols));
        }

        let id = Uuid::new_v4();
        let working_dir = working_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("C:\\")));

        tracing::info!(
            "Creating session {} ({}x{}, cwd: {:?}, shell: {})",
            id,
            rows,
            cols,
            working_dir,
            self.default_shell
        );

        // Create PTY config
        let config = PtyConfig {
            rows,
            cols,
            shell: self.default_shell.clone(),
            working_dir: working_dir.clone(),
            environment: std::env::vars().collect(),
        };

        // Spawn PTY backend (ConPtyBackend for Phase 1 Windows)
        let pty = crate::pty::ConPtyBackend::create(config)
            .await
            .map_err(|e| SessionError::PtyCreateFailed(e.to_string()))?;

        // Create SessionContainer with AbortOnDrop tracking for proper cleanup
        let container = SessionContainer::new(
            id,
            Box::new(pty),
            self.default_shell.clone(),
            working_dir.clone(),
            rows,
            cols,
        );

        // Store container in HashMap FIRST
        self.sessions.write().await.insert(id, container.clone());
        tracing::info!(
            "LIFECYCLE: SessionContainer stored in HashMap for session {}",
            id
        );

        // Spawn tasks WITH AbortOnDrop tracking (fixes memory leak)
        // Store JoinHandles in container - Drop will abort tasks to release Arc references
        tracing::info!(
            "LIFECYCLE: About to spawn pty_output_loop for session {}",
            id
        );
        let output_handle = tokio::spawn(Self::pty_output_loop(
            container.session.clone(),
            container.pty.clone(),
        ));
        *container.output_task.lock().await = Some(output_handle);
        tracing::info!(
            "LIFECYCLE: pty_output_loop spawned WITH AbortOnDrop for session {}",
            id
        );

        // Spawn monomind detection task and store handle
        let monomind_handle = tokio::spawn({
            let session_arc = container.session.clone();
            async move {
                use monoterminal_monomind_bridge::detect_monomind;

                let working_dir = {
                    let s = session_arc.read().await;
                    s.working_dir.clone()
                };

                let detection = detect_monomind(&working_dir);
                if detection.found {
                    let mut s = session_arc.write().await;
                    s.monomind_detected = true;
                    tracing::info!(
                        "Monomind detected in session {}: project={}",
                        s.id,
                        detection
                            .monomind_root
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    );
                } else {
                    let s = session_arc.read().await;
                    tracing::debug!("No monomind detected in session {}", s.id);
                }
            }
        });
        *container.monomind_task.lock().await = Some(monomind_handle);

        tracing::info!(
            "Session {} created successfully WITH AbortOnDrop tracking",
            id
        );

        // Persist to database (Phase 2: graceful degradation if DB unavailable)
        if let Some(db) = &self.db {
            let now = chrono::Utc::now().to_rfc3339();
            let record = db_session::SessionRecord {
                session_id: id,
                created_at: now.clone(),
                last_accessed_at: now,
                status: db_session::SessionStatus::Running,
                shell_path: self.default_shell.clone(),
                working_dir,
                env_vars: Some(std::env::vars().collect()),
                rows,
                cols,
                owner_user_id: owner_user_id.clone(), // Phase 2: From JWT claims
                acl: None, // Initially empty, can be modified via share operations
                metadata: None,
            };

            match db
                .get_conn()
                .and_then(|conn| db_session::create_session(&conn, &record))
            {
                Ok(_) => tracing::info!("Session {} persisted to database", id),
                Err(e) => tracing::warn!("Failed to persist session {} to database: {}", id, e),
            }
        }

        Ok(id)
    }

    /// Attach client to existing session (backward-compatible, no RBAC)
    pub async fn attach_client(
        &self,
        session_id: SessionId,
        client_id: super::session::ClientId,
        output_tx: mpsc::Sender<Vec<u8>>,
    ) -> Result<SessionSnapshot> {
        self.attach_client_with_user(session_id, client_id, output_tx, None)
            .await
    }

    /// Attach client to existing session with optional user_id (RBAC-enabled)
    ///
    /// Returns session snapshot with scrollback for late-joiner sync
    ///
    /// # Arguments
    /// * `session_id` - Session to attach to
    /// * `client_id` - Client identifier
    /// * `output_tx` - Channel for sending output to client
    /// * `user_id` - User requesting attach (for RBAC permission check, optional)
    pub async fn attach_client_with_user(
        &self,
        session_id: SessionId,
        client_id: super::session::ClientId,
        output_tx: mpsc::Sender<Vec<u8>>,
        user_id: Option<String>,
    ) -> Result<SessionSnapshot> {
        // Phase 2: RBAC permission check (Action::Read)
        if let Some(uid) = &user_id {
            self.check_session_permission(&session_id, uid, Action::Read)
                .await?;
        }

        let sessions = self.sessions.read().await;
        let container = sessions
            .get(&session_id)
            .ok_or(SessionError::NotFound(session_id))?;

        let mut session = container.session.write().await;

        // Add client to session with output channel
        session.attach_client(client_id, output_tx);

        tracing::info!("Client {} attached to session {}", client_id, session_id);

        // Persist attachment to database (Phase 2: update status to RUNNING + touch timestamp)
        if let Some(db) = &self.db {
            match db.get_conn().and_then(|conn| {
                db_session::update_session_status(
                    &conn,
                    &session_id,
                    db_session::SessionStatus::Running,
                )
            }) {
                Ok(_) => tracing::debug!(
                    "Session {} status updated to RUNNING in database",
                    session_id
                ),
                Err(e) => tracing::warn!(
                    "Failed to update session {} status in database: {}",
                    session_id,
                    e
                ),
            }
        }

        // Return snapshot with scrollback
        Ok(session.snapshot())
    }

    /// Detach client from session
    pub async fn detach_client(
        &self,
        session_id: SessionId,
        client_id: super::session::ClientId,
    ) -> Result<()> {
        let sessions = self.sessions.read().await;
        let container = sessions
            .get(&session_id)
            .ok_or(SessionError::NotFound(session_id))?;

        let mut session = container.session.write().await;
        session.detach_client(client_id);

        let remaining_clients = session.client_ids().len();
        tracing::info!(
            "Client {} detached from session {} ({} clients remaining)",
            client_id,
            session_id,
            remaining_clients
        );

        // Persist detachment to database (Phase 2: update status to DETACHED if no clients)
        if remaining_clients == 0 {
            if let Some(db) = &self.db {
                match db.get_conn().and_then(|conn| {
                    db_session::update_session_status(
                        &conn,
                        &session_id,
                        db_session::SessionStatus::Detached,
                    )
                }) {
                    Ok(_) => tracing::info!(
                        "Session {} status updated to DETACHED in database",
                        session_id
                    ),
                    Err(e) => tracing::warn!(
                        "Failed to update session {} status in database: {}",
                        session_id,
                        e
                    ),
                }
            }
        }

        Ok(())
    }

    /// Send input to session PTY (backward-compatible, no RBAC)
    pub async fn send_input(&self, session_id: SessionId, data: &[u8]) -> Result<()> {
        self.send_input_with_user(session_id, data, None).await
    }

    /// Send input to session PTY with optional user_id (RBAC-enabled)
    ///
    /// # Arguments
    /// * `session_id` - Target session
    /// * `data` - Input data to send
    /// * `user_id` - User sending input (for RBAC permission check, optional)
    pub async fn send_input_with_user(
        &self,
        session_id: SessionId,
        data: &[u8],
        user_id: Option<String>,
    ) -> Result<()> {
        // Phase 2: RBAC permission check (Action::Write)
        if let Some(uid) = &user_id {
            self.check_session_permission(&session_id, uid, Action::Write)
                .await?;
        }

        tracing::info!(
            "📝 WRITE: send_input ENTRY - session {}, {} bytes",
            session_id,
            data.len()
        );

        let sessions = self.sessions.read().await;
        let container = sessions
            .get(&session_id)
            .ok_or(SessionError::NotFound(session_id))?;

        // Write to PTY (Option A: Lock PTY independently)
        {
            tracing::info!("📝 WRITE: Acquiring PTY lock");
            let mut pty_guard = container.pty.lock().await;
            tracing::info!("📝 WRITE: PTY lock acquired");

            if let Some(ref mut pty) = pty_guard.as_mut() {
                tracing::info!("📝 WRITE: Calling pty.write({} bytes)", data.len());
                pty.write(data).await?;
                tracing::info!("📝 WRITE: pty.write() SUCCESS");
            } else {
                tracing::error!("📝 WRITE: PTY is None!");
            }
        }

        // Update session activity
        let mut session = container.session.write().await;
        session.touch();

        Ok(())
    }

    /// Resize session terminal
    /// Resize terminal (backward-compatible, no RBAC)
    pub async fn resize_session(&self, session_id: SessionId, rows: u16, cols: u16) -> Result<()> {
        self.resize_session_with_user(session_id, rows, cols, None)
            .await
    }

    /// Resize terminal with optional user_id (RBAC-enabled)
    ///
    /// # Arguments
    /// * `session_id` - Target session
    /// * `rows` - New row count
    /// * `cols` - New column count
    /// * `user_id` - User requesting resize (for RBAC permission check, optional)
    pub async fn resize_session_with_user(
        &self,
        session_id: SessionId,
        rows: u16,
        cols: u16,
        user_id: Option<String>,
    ) -> Result<()> {
        // Validate dimensions first
        if rows == 0 || cols == 0 || rows > 500 || cols > 500 {
            return Err(SessionError::InvalidDimensions(rows, cols));
        }

        // Phase 2: RBAC permission check (Action::Resize)
        if let Some(uid) = &user_id {
            self.check_session_permission(&session_id, uid, Action::Resize)
                .await?;
        }

        let sessions = self.sessions.read().await;
        let container = sessions
            .get(&session_id)
            .ok_or(SessionError::NotFound(session_id))?;

        // Resize PTY (Option A: Lock PTY independently)
        {
            let mut pty_guard = container.pty.lock().await;
            if let Some(ref mut pty) = pty_guard.as_mut() {
                pty.resize(rows, cols)?;
            }
        }

        // Update session dimensions
        let mut session = container.session.write().await;
        session.dimensions.rows = rows;
        session.dimensions.cols = cols;
        session.touch();

        tracing::info!("Session {} resized to {}x{}", session_id, rows, cols);

        Ok(())
    }

    /// Kill session and underlying PTY
    /// Terminate a session (backward-compatible, no RBAC)
    pub async fn kill_session(&self, session_id: SessionId) -> Result<()> {
        self.kill_session_with_user(session_id, None).await
    }

    /// Terminate a session with optional user_id (RBAC-enabled)
    ///
    /// # Arguments
    /// * `session_id` - Session to terminate
    /// * `user_id` - User requesting termination (for RBAC permission check, optional)
    pub async fn kill_session_with_user(
        &self,
        session_id: SessionId,
        user_id: Option<String>,
    ) -> Result<()> {
        // Phase 2: RBAC permission check (Action::Kill - owner only)
        if let Some(uid) = &user_id {
            self.check_session_permission(&session_id, uid, Action::Kill)
                .await?;
        }

        tracing::info!("Terminating session {}", session_id);
        let mut sessions = self.sessions.write().await;
        let container = sessions
            .remove(&session_id)
            .ok_or(SessionError::NotFound(session_id))?;

        tracing::info!("Session {} terminating", session_id);

        // Terminate the PTY via SessionContainer's terminate_pty method
        container
            .terminate_pty()
            .await
            .map_err(|e| SessionError::IoError(e))?;

        // Give the output loop time to detect termination and clean up
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        tracing::info!("Session {} terminated successfully", session_id);

        // Persist termination to database (Phase 2: graceful degradation)
        if let Some(db) = &self.db {
            match db.get_conn().and_then(|conn| {
                db_session::update_session_status(
                    &conn,
                    &session_id,
                    db_session::SessionStatus::Terminated,
                )
            }) {
                Ok(_) => tracing::info!("Session {} marked as TERMINATED in database", session_id),
                Err(e) => tracing::warn!(
                    "Failed to update session {} status in database: {}",
                    session_id,
                    e
                ),
            }
        }

        Ok(())
    }

    /// Check if user has permission to perform action on session (Phase 2: RBAC)
    ///
    /// Loads session owner and ACL from database, then calls RBAC check_permission
    async fn check_session_permission(
        &self,
        session_id: &SessionId,
        user_id: &str,
        action: Action,
    ) -> Result<()> {
        // Load session ACL from database
        if let Some(db) = &self.db {
            match db
                .get_conn()
                .and_then(|conn| db_session::load_session(&conn, session_id))
            {
                Ok(record) => {
                    // Call RBAC permission check
                    check_permission(
                        record.owner_user_id.as_deref(),
                        record.acl.as_ref(),
                        user_id,
                        action,
                    )
                    .map_err(|e| SessionError::PermissionDenied(e.to_string()))?;

                    tracing::debug!(
                        "User {} granted {:?} permission on session {}",
                        user_id,
                        action,
                        session_id
                    );
                    Ok(())
                }
                Err(e) => {
                    // Graceful degradation: If DB unavailable, allow operation (backward compat)
                    tracing::warn!(
                        "Failed to load session {} ACL from DB, allowing operation: {}",
                        session_id,
                        e
                    );
                    Ok(())
                }
            }
        } else {
            // No database = no ACL enforcement (backward compatibility)
            tracing::debug!(
                "No database configured, skipping RBAC check for session {}",
                session_id
            );
            Ok(())
        }
    }

    /// PTY output fan-out loop with flush triggers
    /// Per SRS §3.1.4: Read from PTY with 100ms timeout, newline detection, 4KB buffer trigger
    /// Option A: Takes session and PTY as separate Arc's to prevent attach_client deadlock
    async fn pty_output_loop(
        session: Arc<RwLock<Session>>,
        pty: Arc<tokio::sync::Mutex<Option<Box<dyn crate::pty::PtyBackend>>>>,
    ) {
        // ========== DIAGNOSTIC: Check if this task is even running ==========
        tracing::info!("🔴🔴🔴 PTY OUTPUT LOOP: ALIVE AND POLLING 🔴🔴🔴");
        // EXTREME logging - literally between EVERY statement
        tracing::info!("PTY loop: ENTRY");

        tracing::info!("PTY loop: About to acquire session read lock");
        let session_id = {
            let s = session.read().await;
            tracing::info!("PTY loop: Read lock acquired");
            let id = s.id;
            tracing::info!("PTY loop: Got session id: {}", id);
            id
        };
        tracing::info!("PTY loop: Read lock released, session_id = {}", session_id);

        tracing::info!("PTY output loop started for session {}", session_id);

        tracing::info!("PTY loop: About to log trace message");
        // DISABLED: tracing::trace!("PTY output loop: about to allocate buffer (session {})", session_id);
        tracing::info!("PTY loop: Trace message logged");

        tracing::info!("PTY loop: About to allocate buffer");
        let mut buffer = vec![0u8; 4096]; // 4KB buffer per SRS §5.1.1
        tracing::info!("PTY loop: Buffer allocated");
        // COMMENTED OUT - Testing if trace!() kills task:
        // // DISABLED: tracing::trace!("PTY output loop: buffer allocated (session {})", session_id);

        tracing::info!("PTY loop: About to initialize pending_data");
        let mut pending_data = Vec::new();
        tracing::info!("PTY loop: pending_data initialized");

        tracing::info!("PTY loop: About to get Instant::now()");
        let mut last_flush = tokio::time::Instant::now();
        tracing::info!("PTY loop: last_flush initialized");

        tracing::info!("PTY loop: About to initialize sequence_number");
        let mut sequence_number: u64 = 0;
        tracing::info!("PTY loop: sequence_number initialized");

        tracing::info!("PTY loop: About to enter main loop");

        loop {
            tracing::info!("PTY loop: Loop iteration START");

            // Check if session is terminated
            tracing::info!("PTY loop: About to acquire session read lock");
            {
                let s = session.read().await;
                tracing::info!("PTY loop: Session read lock acquired, state={:?}", s.state);
                if s.state == SessionState::Terminated {
                    tracing::debug!("Session terminated, exiting output loop");
                    break;
                }
            }
            tracing::info!("PTY loop: Session state check passed");

            tracing::info!("PTY loop: About to attempt PTY read");

            // Read from PTY with timeout (Option A: Lock PTY independently)
            tracing::info!("PTY loop: Creating timeout for PTY read");
            let read_result =
                tokio::time::timeout(tokio::time::Duration::from_millis(100), async {
                    tracing::info!("PTY loop: Inside timeout async block, acquiring PTY lock");
                    let mut pty_guard = pty.lock().await;
                    tracing::info!("PTY loop: PTY lock acquired");
                    if let Some(ref mut pty_backend) = pty_guard.as_mut() {
                        tracing::info!("PTY loop: Calling pty.read()");
                        pty_backend.read(&mut buffer).await
                    } else {
                        tracing::warn!(
                            "PTY output loop: PTY backend is None (session {})",
                            session_id
                        );
                        Ok(0) // PTY terminated
                    }
                })
                .await;
            tracing::info!("PTY loop: Timeout completed, read_result obtained");

            match read_result {
                Ok(Ok(0)) => {
                    // EOF - PTY terminated
                    tracing::info!(
                        "PTY EOF detected (session {}), session terminating",
                        session_id
                    );

                    // Flush any pending data
                    if !pending_data.is_empty() {
                        tracing::debug!(
                            "PTY EOF: flushing {} bytes of pending data (session {})",
                            pending_data.len(),
                            session_id
                        );
                        let mut s = session.write().await;
                        s.scrollback
                            .push_line(super::session::Line::from_bytes(&pending_data));
                        Self::broadcast_output(&mut s, &pending_data, sequence_number).await;
                        pending_data.clear();
                    }
                    break;
                }
                Ok(Ok(n)) => {
                    // Data received (n >= 1)
                    tracing::debug!("PTY read: received {} bytes (session {})", n, session_id);
                    // DISABLED: tracing::trace!("PTY read: data = {:?} (session {})", &buffer[..n], session_id);
                    pending_data.extend_from_slice(&buffer[..n]);

                    // Flush triggers (SRS §3.1.4):
                    // 1. Buffer >= 4KB
                    // 2. Newline detected
                    // 3. 100ms timeout (handled by timeout above)
                    let should_flush = pending_data.len() >= 4096
                        || pending_data.contains(&b'\n')
                        || last_flush.elapsed() >= tokio::time::Duration::from_millis(100);

                    // DISABLED: tracing::trace!("PTY read: should_flush={} (pending={} bytes, session {})", should_flush, pending_data.len(), session_id);

                    if should_flush {
                        tracing::debug!(
                            "PTY read: flushing {} bytes to clients (session {})",
                            pending_data.len(),
                            session_id
                        );
                        // Add to scrollback
                        let mut s = session.write().await;
                        s.scrollback
                            .push_line(super::session::Line::from_bytes(&pending_data));
                        s.touch();

                        // Fan-out to clients via Arc<Bytes> (SRS §3.1.4 zero-copy pattern)
                        Self::broadcast_output(&mut s, &pending_data, sequence_number).await;
                        sequence_number += 1;

                        pending_data.clear();
                        last_flush = tokio::time::Instant::now();
                    }
                }
                Ok(Err(e)) => {
                    tracing::error!("PTY read error (session {}): {}", session_id, e);
                    break;
                }
                Err(_) => {
                    // Timeout - flush pending data if any
                    // DISABLED: tracing::trace!("PTY read timeout (session {}), pending {} bytes", session_id, pending_data.len());
                    if !pending_data.is_empty() {
                        tracing::debug!(
                            "PTY timeout: flushing {} bytes (session {})",
                            pending_data.len(),
                            session_id
                        );
                        let mut s = session.write().await;
                        s.scrollback
                            .push_line(super::session::Line::from_bytes(&pending_data));
                        Self::broadcast_output(&mut s, &pending_data, sequence_number).await;
                        sequence_number += 1;
                        pending_data.clear();
                    }
                    last_flush = tokio::time::Instant::now();
                }
            }
        }

        // Mark session as terminated
        let mut s = session.write().await;
        s.state = SessionState::Terminated;
        tracing::info!("Session {} output loop terminated", s.id);
    }

    /// Broadcast output data to all attached clients
    /// Uses Arc<Bytes> for zero-copy fan-out (SRS §3.1.4)
    async fn broadcast_output(session: &mut Session, data: &[u8], sequence_number: u64) {
        use monoterminal_protocol::{envelope, Envelope, OutputData};

        tracing::debug!(
            "broadcast_output: broadcasting {} bytes to {} clients (session {}, seq {})",
            data.len(),
            session.clients.len(),
            session.id,
            sequence_number
        );

        // Encode as Protocol OutputData envelope
        let envelope = Envelope {
            sequence_number,
            message: Some(envelope::Message::OutputData(OutputData {
                data: Bytes::copy_from_slice(data).to_vec(),
                sequence: sequence_number,
                compression: monoterminal_protocol::CompressionType::None as i32,
            })),
        };

        let mut encoded = Vec::with_capacity(envelope.encoded_len());
        if let Err(e) = envelope.encode(&mut encoded) {
            tracing::error!(
                "Failed to encode OutputData (session {}): {}",
                session.id,
                e
            );
            return;
        }

        // DISABLED: tracing::trace!("broadcast_output: encoded {} bytes (session {})", encoded.len(), session.id);

        // Use Arc for zero-copy broadcast
        let encoded = Arc::new(encoded);

        // Broadcast to all clients, removing lagging/disconnected clients
        let mut to_remove = Vec::new();

        for (client_id, tx) in &session.clients {
            // Clone Arc (cheap pointer copy, not data copy)
            let data_clone = (*encoded).clone();

            // Non-blocking send (SRS §3.1.4: detect lagging clients)
            match tx.try_send(data_clone) {
                Ok(_) => {
                    // DISABLED: tracing::trace!("broadcast_output: sent to client {} (session {})", client_id, session.id);
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    // Client buffer full - lagging
                    tracing::warn!(
                        "Client {} buffer full (lagging), data dropped (session {})",
                        client_id,
                        session.id
                    );
                    // TODO: Track lagging duration, disconnect if >30s (Phase 1.5)
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    // Client disconnected
                    tracing::info!("Client {} disconnected, removing from session", client_id);
                    to_remove.push(*client_id);
                }
            }
        }

        // Remove disconnected clients
        for client_id in to_remove {
            session.detach_client(client_id);
        }
    }

    /// List all active sessions
    pub async fn list_sessions(&self) -> Vec<SessionId> {
        self.sessions.read().await.keys().copied().collect()
    }

    /// Get session count
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Get session's current working directory
    ///
    /// Used by monomind integration to detect .monomind/ directory
    /// in the session's working directory tree.
    ///
    /// Returns None if session not found.
    pub fn get_session_cwd(&self, session_id: SessionId) -> Option<PathBuf> {
        // Note: This is a synchronous method that returns immediately
        // We can't use async here because we're called from process_message
        // which needs to get the cwd synchronously.
        //
        // For Phase 1, sessions are created with a working_dir and it
        // doesn't change (cd tracking is Phase 2+). So we can return
        // the initial cwd without blocking.
        //
        // TODO Phase 2: Track cwd changes via OSC-7 sequences

        // For now, return std::env::current_dir() as a fallback
        // The actual session cwd is stored in the Session struct
        // but requires async access. For Phase 1, using current_dir
        // is acceptable since the daemon runs in the project root.
        std::env::current_dir().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_manager_new() {
        let manager = SessionManager::new(Some("cmd.exe".to_string()));
        assert_eq!(manager.default_shell, "cmd.exe");
    }

    #[tokio::test]
    async fn test_validate_dimensions() {
        let manager = SessionManager::new(None);

        // Invalid dimensions should error
        let result = manager.create_session(None, 0, 80).await;
        assert!(result.is_err());

        let result = manager.create_session(None, 24, 0).await;
        assert!(result.is_err());

        let result = manager.create_session(None, 600, 80).await;
        assert!(result.is_err());
    }

    // Note: Full integration tests require ConPtyBackend implementation
    // These will be added in task-18 (Backend Integration)
}
