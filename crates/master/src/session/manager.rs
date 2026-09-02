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
use crate::layout::{LayoutManager, SplitDirection};
use crate::persistence::{layout::LayoutPersistence, session as db_session, Database};
use crate::pty::{PtyBackend, PtyConfig};

/// Result of a successful `handle_split_pane` call. Carries the new pane's
/// session id (not just the layout tree) because the caller — the WS
/// connection handler — needs it to also attach the connection's output
/// channel to the new pane's session, or the new pane's terminal would
/// never actually stream any output to the client.
#[derive(Debug)]
pub struct SplitPaneResult {
    pub layout_update: monoterminal_protocol::LayoutUpdate,
    pub new_session_id: SessionId,
    pub new_pane_id: String,
}

/// Central session manager
/// Phase 1: Single active session (simplified from SRS multi-session design)
/// Phase 4: Pane layout management (ADR-018, task-73)
pub struct SessionManager {
    /// Active sessions (Option A: SessionContainer with separated locks)
    sessions: Arc<RwLock<HashMap<SessionId, SessionContainer>>>,

    /// Maps a stable logical session key (e.g. "computer/workspace/terminal")
    /// to the live SessionId it currently resolves to, so multiple clients
    /// referring to the same logical terminal converge on one PTY session
    /// instead of each spawning their own (see `resolve_named_session`).
    named_sessions: Arc<RwLock<HashMap<String, SessionId>>>,

    /// Default shell (pwsh.exe if available, else cmd.exe per architecture)
    default_shell: String,

    /// Persistence layer (Phase 2: SQLite session + scrollback storage)
    db: Option<Arc<Database>>,

    /// Pane layout managers (Phase 4: Splits/Tabs, ADR-018), one per
    /// workspace — keyed by the workspace's root session id (its "pane-0",
    /// i.e. whatever session `resolve_named_session` first created for that
    /// name). Each workspace gets its own independent split tree; this must
    /// NOT be a single global layout, or splitting a pane in one workspace
    /// would corrupt/entangle every other workspace's sessions.
    layouts: Arc<RwLock<HashMap<SessionId, LayoutManager>>>,

    /// Layout persistence (Phase 4: task-74 Day 4)
    /// Saves/loads layout state to/from SQLite
    layout_persistence: Option<Arc<LayoutPersistence>>,
}

impl SessionManager {
    /// Create new session manager
    pub fn new(default_shell: Option<String>) -> Self {
        Self::new_with_db(default_shell, None)
    }

    /// Create new session manager with optional persistence
    pub fn new_with_db(default_shell: Option<String>, db: Option<Arc<Database>>) -> Self {
        let default_shell = default_shell.unwrap_or_else(|| {
            #[cfg(windows)]
            {
                "cmd.exe".to_string()
            }
            #[cfg(unix)]
            {
                "/bin/bash".to_string()
            }
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

        // Initialize layout persistence if database available
        let layout_persistence = db.as_ref().map(|db_arc| {
            Arc::new(LayoutPersistence::new(Arc::clone(db_arc)))
        });

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            named_sessions: Arc::new(RwLock::new(HashMap::new())),
            default_shell,
            db,
            layouts: Arc::new(RwLock::new(HashMap::new())), // Phase 4: one entry per workspace root
            layout_persistence,
        }
    }

    /// Find-or-create a session for a stable logical key.
    ///
    /// Multiple clients (e.g. the same "terminal" opened in two browser tabs
    /// or devices) can each independently derive the same `name` from their
    /// local workspace/terminal labels. The first caller creates the
    /// session; every subsequent caller with the same name attaches to that
    /// same live session instead of spawning a new PTY, which is what makes
    /// cross-client sync work without a server-side workspace database.
    ///
    /// `previous_name`, when set, means the caller knows this session used
    /// to be reachable under that name and wants it addressable as `name`
    /// from now on — e.g. the client renamed a workspace or its owning
    /// computer, and reconnects with the new name plus the old one it was
    /// just attached to. Without this, a rename would silently orphan the
    /// live session under its old name and hand back a fresh, empty one for
    /// the new name, since the two are otherwise unrelated strings to this
    /// map. Only consulted when `name` doesn't already resolve to a live
    /// session — an existing mapping for `name` always wins.
    ///
    /// If the previously-mapped session has since been terminated/removed,
    /// a fresh session is created and the mapping is updated.
    pub async fn resolve_named_session(
        &self,
        name: &str,
        previous_name: Option<&str>,
        owner_user_id: Option<String>,
        rows: u16,
        cols: u16,
    ) -> Result<SessionId> {
        let mut named = self.named_sessions.write().await;

        if let Some(existing_id) = named.get(name).copied() {
            if self.sessions.read().await.contains_key(&existing_id) {
                return Ok(existing_id);
            }
        }

        if let Some(prev) = previous_name.filter(|p| !p.is_empty() && *p != name) {
            if let Some(existing_id) = named.remove(prev) {
                if self.sessions.read().await.contains_key(&existing_id) {
                    named.insert(name.to_string(), existing_id);
                    return Ok(existing_id);
                }
            }
        }

        // create_session_with_user registers this new session as its own
        // workspace root (gets its own layout tree) — see `ensure_layout`.
        let id = self
            .create_session_with_user(owner_user_id, None, rows, cols)
            .await?;
        named.insert(name.to_string(), id);

        Ok(id)
    }

    /// Registers a brand-new, independent layout tree rooted at
    /// `root_session_id` (its "pane-0"), if one doesn't already exist.
    /// Called once per workspace, when that workspace's first session is
    /// created — never for sessions spawned by `handle_split_pane`, which
    /// belong to an existing workspace's layout instead of starting their
    /// own.
    async fn ensure_layout(&self, root_session_id: SessionId) {
        let mut layouts = self.layouts.write().await;
        if !layouts.contains_key(&root_session_id) {
            layouts.insert(root_session_id, LayoutManager::new(root_session_id));
            drop(layouts);
            if let Some(container) = self.sessions.read().await.get(&root_session_id) {
                container.session.write().await.pane_id = Some("pane-0".to_string());
            }
        }
    }

    /// Every (pane_id, session_id) pair in the workspace layout rooted at
    /// `root_session_id`, including the root itself. Empty if that workspace
    /// has no split layout (plain, non-paned session). Used when a client
    /// (re)attaches, so it can also subscribe to every other pane's output —
    /// otherwise only the root pane's output would ever reach it.
    pub async fn sibling_pane_sessions(&self, root_session_id: SessionId) -> Vec<(String, SessionId)> {
        let layouts = self.layouts.read().await;
        match layouts.get(&root_session_id) {
            Some(layout) => layout
                .collect_pane_ids()
                .into_iter()
                .filter_map(|pane_id| {
                    layout
                        .get_session_id(&pane_id)
                        .map(|session_id| (pane_id, session_id))
                })
                .collect(),
            None => Vec::new(),
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

    /// Create new terminal session with optional owner user_id (RBAC-enabled).
    ///
    /// This is the entry point for a brand-new *workspace root* — it gets
    /// its own independent layout tree (Phase 4: Splits/Tabs, ADR-018). A
    /// pane created by splitting an existing workspace must NOT go through
    /// here (it belongs to that workspace's existing layout instead) — see
    /// `create_session_inner`, which this delegates to.
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
        let id = self
            .create_session_inner(owner_user_id, working_dir, rows, cols)
            .await?;
        self.ensure_layout(id).await;
        Ok(id)
    }

    /// Spawns a PTY session without touching layout state at all. Used by
    /// `create_session_with_user` (which registers a new layout afterward)
    /// and by `handle_split_pane` (which instead attaches the new session
    /// into the *existing* workspace layout it was split from).
    async fn create_session_inner(
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

        // Spawn PTY backend (platform-conditional: ConPtyBackend for Windows, UnixPtyBackend for Unix)
        #[cfg(windows)]
        let pty = crate::pty::ConPtyBackend::create(config)
            .await
            .map_err(|e| SessionError::PtyCreateFailed(e.to_string()))?;

        #[cfg(unix)]
        let pty = crate::pty::UnixPtyBackend::create(config)
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

        // Store the container in the HashMap only after both task handles are
        // recorded on it. output_task/monomind_task are Arc<Mutex<...>> —
        // inserting a clone() earlier and continuing to use the original
        // `container` local meant that local's Drop (which aborts whatever
        // handle sits in the shared Arc<Mutex<Option<JoinHandle>>> at drop
        // time) fired as soon as this function returned, killing the
        // just-spawned output task instantly via the shared Mutex — even
        // though a clone of the container still lived on in the map. The
        // session appeared to attach fine (map lookup succeeded) but never
        // streamed any PTY output back, because its output task had already
        // been aborted before the map's clone was ever read.
        self.sessions.write().await.insert(id, container);
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
            .map_err(SessionError::IoError)?;

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

    // ============================================================================
    // Phase 4: Pane Layout Management (ADR-018, task-73)
    // ============================================================================

    /// Handle split pane command (Phase 4: Splits/Tabs)
    ///
    /// Creates a new PTY session and splits the specified pane into two panes.
    ///
    /// # Arguments
    /// * `root_session_id` - The workspace's root session id (identifies
    ///   which workspace's layout tree to mutate — pane ids are only unique
    ///   within one workspace's layout, not globally)
    /// * `pane_id` - Which pane to split
    /// * `direction` - Split direction (horizontal/vertical)
    /// * `new_session_shell` - Shell command for new pane (e.g., "cmd.exe", "/bin/bash")
    /// * `user_id` - User requesting split (for RBAC, optional)
    ///
    /// # Returns
    /// The new layout tree plus the new pane's id and session id — the
    /// caller (the WS connection handler) needs `new_session_id` to also
    /// attach this connection's output channel to the new pane's session,
    /// so its terminal output actually streams to the client.
    pub async fn handle_split_pane(
        &self,
        root_session_id: SessionId,
        pane_id: &str,
        direction: SplitDirection,
        new_session_shell: Option<String>,
        user_id: Option<String>,
    ) -> Result<SplitPaneResult> {
        // Get current working directory and dimensions from existing pane's session
        let (working_dir, rows, cols) = {
            let layouts = self.layouts.read().await;
            let layout = layouts.get(&root_session_id).ok_or_else(|| {
                SessionError::LayoutError("LayoutManager not initialized".to_string())
            })?;
            if let Some(session_id) = layout.get_session_id(pane_id) {
                let sessions = self.sessions.read().await;
                if let Some(container) = sessions.get(&session_id) {
                    let session = container.session.read().await;
                    (
                        Some(session.working_dir.clone()),
                        session.dimensions.rows,
                        session.dimensions.cols,
                    )
                } else {
                    (None, 24, 80) // Fallback defaults
                }
            } else {
                (None, 24, 80) // Fallback defaults
            }
        };

        // Create new PTY session for the new pane. Uses `create_session_inner`
        // (not `create_session_with_user`) — this session belongs to the
        // existing workspace layout being split, it must NOT become the root
        // of a brand-new layout of its own.
        // TODO: Support custom shell per pane via new_session_shell parameter
        // For now, use default shell (requires refactoring create_session to accept shell override)
        let _custom_shell = new_session_shell; // Reserved for future use

        let new_session_id = self
            .create_session_inner(user_id.clone(), working_dir, rows, cols)
            .await?;

        tracing::info!(
            "Created new session {} for split pane ({}x{})",
            new_session_id,
            rows,
            cols
        );

        // Update layout tree
        let (new_pane_id, layout_update) = {
            let mut layouts = self.layouts.write().await;
            let layout = layouts.get_mut(&root_session_id).ok_or_else(|| {
                SessionError::LayoutError("LayoutManager not initialized".to_string())
            })?;

            let new_pane_id = layout
                .split_pane(pane_id, direction, new_session_id)
                .map_err(|e| SessionError::LayoutError(format!("Split pane failed: {}", e)))?;

            tracing::info!(
                "Split pane '{}' ({:?}) → created new pane '{}'",
                pane_id,
                direction,
                new_pane_id
            );

            (new_pane_id, layout.to_proto())
        };

        // Tag the new session with its pane_id so its output can be
        // attributed to the right pane on the client (see OutputData.pane_id).
        if let Some(container) = self.sessions.read().await.get(&new_session_id) {
            container.session.write().await.pane_id = Some(new_pane_id.clone());
        }

        // Auto-save layout (if user_id available)
        if let Some(uid) = &user_id {
            if let Err(e) = self.auto_save_layout(uid, root_session_id).await {
                tracing::warn!("Failed to auto-save layout after split: {}", e);
            }
        }

        Ok(SplitPaneResult {
            layout_update,
            new_session_id,
            new_pane_id,
        })
    }

    /// Handle close pane command (Phase 4: Splits/Tabs)
    ///
    /// Closes a pane and kills its associated PTY session.
    ///
    /// # Arguments
    /// * `pane_id` - Which pane to close
    /// * `user_id` - User requesting close (for RBAC, optional)
    ///
    /// # Returns
    /// LayoutUpdate with new layout tree after pane removal
    pub async fn handle_close_pane(
        &self,
        root_session_id: SessionId,
        pane_id: &str,
        user_id: Option<String>,
    ) -> Result<monoterminal_protocol::LayoutUpdate> {
        // Get session ID for the pane, then validate + mutate the layout tree
        // BEFORE touching the PTY session. `close_pane` can fail (e.g.
        // CANNOT_CLOSE_LAST_PANE) — if we killed the session first and only
        // validated afterward, a rejected close would still have destroyed
        // the user's terminal, which is worse than doing nothing.
        let (session_id, layout_update) = {
            let mut layouts = self.layouts.write().await;
            let layout = layouts.get_mut(&root_session_id).ok_or_else(|| {
                SessionError::LayoutError("LayoutManager not initialized".to_string())
            })?;

            let session_id = layout
                .get_session_id(pane_id)
                .ok_or_else(|| SessionError::LayoutError(format!("Pane '{}' not found", pane_id)))?;

            layout
                .close_pane(pane_id)
                .map_err(|e| SessionError::LayoutError(format!("Close pane failed: {}", e)))?;

            tracing::info!("Closed pane '{}'", pane_id);

            (session_id, layout.to_proto())
        };

        // Now that the layout accepted the close, actually kill the PTY session.
        self.kill_session_with_user(session_id, user_id.clone()).await?;
        tracing::info!("Killed session {} for pane '{}'", session_id, pane_id);

        // Auto-save layout (if user_id available)
        if let Some(uid) = user_id {
            if let Err(e) = self.auto_save_layout(&uid, root_session_id).await {
                tracing::warn!("Failed to auto-save layout after close: {}", e);
            }
        }

        // Return updated layout
        Ok(layout_update)
    }

    /// Handle focus pane command (Phase 4: Splits/Tabs)
    ///
    /// Changes which pane has input focus.
    ///
    /// # Arguments
    /// * `root_session_id` - Which workspace's layout to mutate
    /// * `pane_id` - Which pane to focus
    ///
    /// # Returns
    /// LayoutUpdate with updated focus state
    pub async fn handle_focus_pane(
        &self,
        root_session_id: SessionId,
        pane_id: &str,
    ) -> Result<monoterminal_protocol::LayoutUpdate> {
        // Note: focus_pane doesn't have user_id parameter (not needed for RBAC)
        // Auto-save is skipped for focus changes (minor state change, can skip persistence)

        let mut layouts = self.layouts.write().await;
        let layout = layouts
            .get_mut(&root_session_id)
            .ok_or_else(|| SessionError::LayoutError("LayoutManager not initialized".to_string()))?;

        layout
            .focus_pane(pane_id)
            .map_err(|e| SessionError::LayoutError(format!("Focus pane failed: {}", e)))?;

        tracing::info!("Focused pane '{}'", pane_id);

        // Return updated layout
        Ok(layout.to_proto())
    }

    /// Resolves `pane_id` (or the workspace's currently focused pane, if
    /// `None`) to its underlying session id, within the layout rooted at
    /// `root_session_id`. Shared by every pane-targeted operation
    /// (input, resize) so they all resolve pane ids the same way.
    async fn resolve_pane_session(
        &self,
        root_session_id: SessionId,
        pane_id: Option<&str>,
    ) -> Result<(String, SessionId)> {
        let layouts = self.layouts.read().await;
        let layout = layouts
            .get(&root_session_id)
            .ok_or_else(|| SessionError::LayoutError("LayoutManager not initialized".to_string()))?;

        let target = pane_id.unwrap_or_else(|| layout.get_focused_pane_id());
        let session_id = layout
            .get_session_id(target)
            .ok_or_else(|| SessionError::LayoutError(format!("Pane '{}' not found", target)))?;

        Ok((target.to_string(), session_id))
    }

    /// Resize a specific pane's PTY (or the workspace's focused pane, if
    /// `pane_id` is `None`). Each pane is its own independent PTY, so
    /// resizing the root session alone would leave every other pane's
    /// terminal size stuck at whatever it was when the pane was created.
    ///
    /// # Arguments
    /// * `root_session_id` - Which workspace's layout to look the pane up in
    /// * `pane_id` - Target pane ID (None = focused pane)
    pub async fn resize_pane(
        &self,
        root_session_id: SessionId,
        pane_id: Option<&str>,
        rows: u16,
        cols: u16,
        user_id: Option<String>,
    ) -> Result<()> {
        let (_, session_id) = self.resolve_pane_session(root_session_id, pane_id).await?;
        self.resize_session_with_user(session_id, rows, cols, user_id)
            .await
    }

    /// Send input to the focused pane (or specified pane)
    ///
    /// Phase 4: Routes input to pane's session. Falls back to focused pane if pane_id is None.
    ///
    /// # Arguments
    /// * `root_session_id` - Which workspace's layout to look the pane up in
    /// * `pane_id` - Target pane ID (None = focused pane)
    /// * `data` - Input data to send
    /// * `user_id` - User sending input (for RBAC, optional)
    pub async fn send_input_to_pane(
        &self,
        root_session_id: SessionId,
        pane_id: Option<&str>,
        data: &[u8],
        user_id: Option<String>,
    ) -> Result<()> {
        // Determine target pane (specified or focused)
        let (target_pane_id, session_id) =
            self.resolve_pane_session(root_session_id, pane_id).await?;

        tracing::debug!(
            "Routing input ({} bytes) to pane '{}' (session {})",
            data.len(),
            target_pane_id,
            session_id
        );

        // Send input to the pane's session
        self.send_input_with_user(session_id, data, user_id).await
    }

    /// Persistence key for a workspace's layout: layouts are per-workspace
    /// (see `layouts` field), but the storage schema's primary key is a
    /// single TEXT column keyed by user — composing user_id with the
    /// workspace's root session id keeps one user's several workspaces from
    /// overwriting each other's saved layout under that one column.
    fn layout_persistence_key(user_id: &str, root_session_id: SessionId) -> String {
        format!("{}:{}", user_id, root_session_id)
    }

    /// Auto-save layout to persistence layer (Phase 4: task-74 Day 4)
    ///
    /// Called after every layout change (split/close/focus) to persist state.
    ///
    /// # Arguments
    /// * `user_id` - User ID to save layout for
    /// * `root_session_id` - Which workspace's layout this is
    ///
    /// # Returns
    /// * `Ok(())` - Layout saved successfully (or no persistence available)
    /// * `Err(_)` - Database error or serialization failure
    async fn auto_save_layout(&self, user_id: &str, root_session_id: SessionId) -> Result<()> {
        // Check if persistence is available
        let persistence = match &self.layout_persistence {
            Some(p) => p,
            None => {
                tracing::debug!("Layout persistence not available, skipping auto-save");
                return Ok(());
            }
        };

        // Get layout snapshot
        let layout_update = {
            let layouts = self.layouts.read().await;
            match layouts.get(&root_session_id) {
                Some(layout) => layout.to_proto(),
                None => {
                    tracing::debug!("No layout to save (LayoutManager not initialized)");
                    return Ok(());
                }
            }
        };

        // Save to persistence layer
        let key = Self::layout_persistence_key(user_id, root_session_id);
        persistence
            .save_layout(&key, &layout_update)
            .await
            .map_err(|e| {
                SessionError::LayoutError(format!("Failed to save layout: {}", e))
            })?;

        tracing::debug!("Auto-saved layout for user {} (key {})", user_id, key);

        Ok(())
    }

    /// Auto-load layout from persistence layer (Phase 4: task-74 Day 4)
    ///
    /// Called on session attachment to restore saved layout state.
    ///
    /// # Arguments
    /// * `user_id` - User ID to load layout for
    /// * `root_session_id` - Which workspace's layout this is
    ///
    /// # Returns
    /// * `Ok(())` - Layout loaded (or no saved layout available)
    /// * `Err(_)` - Database error or deserialization failure
    #[allow(dead_code)] // Restoration (LayoutManager::from_proto) not yet implemented
    async fn auto_load_layout(&self, user_id: &str, root_session_id: SessionId) -> Result<()> {
        // Check if persistence is available
        let persistence = match &self.layout_persistence {
            Some(p) => p,
            None => {
                tracing::debug!("Layout persistence not available, skipping auto-load");
                return Ok(());
            }
        };

        // Load layout from persistence
        let key = Self::layout_persistence_key(user_id, root_session_id);
        let _layout_update = match persistence.load_layout(&key).await {
            Ok(Some(layout)) => layout,
            Ok(None) => {
                tracing::debug!("No saved layout for user {}", user_id);
                return Ok(());
            }
            Err(e) => {
                return Err(SessionError::LayoutError(format!(
                    "Failed to load layout: {}",
                    e
                )));
            }
        };

        // TODO: Restore layout from LayoutUpdate proto
        // This requires implementing LayoutManager::from_proto() or similar
        // For now, just log that we loaded the layout
        tracing::info!(
            "Loaded layout for user {} (restoration not yet implemented)",
            user_id
        );

        // Store the loaded layout update for future use
        // When LayoutManager::from_proto() is implemented, reconstruct layout here

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
                pane_id: session.pane_id.clone(),
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
    pub fn get_session_cwd(&self, _session_id: SessionId) -> Option<PathBuf> {
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
