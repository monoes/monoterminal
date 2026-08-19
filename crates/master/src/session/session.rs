// Core Session types
// SRS §2.1.3: Session lifecycle, state machine, in-memory scrollback

use super::scrollback::RingBuffer;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

/// Session identifier (UUID v4)
pub type SessionId = Uuid;

/// Client identifier (UUID v4)
pub type ClientId = Uuid;

/// Session state machine (Phase 1: simplified, per architecture)
/// CREATE → RUNNING → TERMINATED
/// (DETACHED state deferred to Phase 2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session created, PTY spawned, ready for clients
    Running,

    /// PTY terminated, cleanup pending
    Terminated,
}

/// Terminal dimensions
#[derive(Debug, Clone, Copy)]
pub struct Dimensions {
    pub rows: u16,
    pub cols: u16,
}

/// Terminal line (for scrollback)
#[derive(Debug, Clone)]
pub struct Line {
    /// Raw line data (UTF-8 with ANSI escape codes)
    pub data: Vec<u8>,

    /// Sequential line number
    pub line_number: u64,
}

impl Line {
    pub fn from_bytes(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            line_number: 0, // Will be set by scrollback
        }
    }
}

/// Session snapshot (returned on attach for late-joiner sync)
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub id: SessionId,
    pub scrollback: Vec<Line>,
    pub rows: u16,
    pub cols: u16,
    pub working_dir: PathBuf,
    pub shell_type: String,
}

/// Session container - holds session state and PTY with separate locks
/// Option A: Separate PTY I/O from session metadata to eliminate deadlock
/// This prevents RwLock contention between attach_client and pty_output_loop
#[derive(Clone)]
pub struct SessionContainer {
    /// Session state (metadata, scrollback, clients)
    /// RwLock allows multiple readers (concurrent attaches)
    pub session: Arc<RwLock<Session>>,

    /// PTY backend (I/O operations)
    /// Mutex for exclusive I/O access (single writer)
    /// Option allows graceful termination (set to None, pty_output_loop exits)
    pub pty: Arc<Mutex<Option<Box<dyn crate::pty::PtyBackend>>>>,

    /// PTY output loop task handle (aborted on drop to prevent memory leaks)
    /// Per ADR-006: Fire-and-forget tokio::spawn tasks hold Arc references indefinitely
    /// Storing JoinHandle and aborting on drop releases references immediately
    pub output_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,

    /// Monomind detection task handle (aborted on drop to prevent memory leaks)
    pub monomind_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

/// Terminal session (metadata and state only - PTY moved to SessionContainer)
/// Option A: PTY extracted to SessionContainer for independent lock management
pub struct Session {
    /// Session UUID
    pub id: SessionId,

    /// Current state
    pub state: SessionState,

    /// Shell process ID (cached from PTY at creation, PTY now in SessionContainer)
    pub shell_pid: u32,

    /// Shell type (e.g., "pwsh.exe", "cmd.exe")
    pub shell_type: String,

    /// Terminal dimensions
    pub dimensions: Dimensions,

    /// Working directory
    pub working_dir: PathBuf,

    /// In-memory scrollback (10k lines, ~1MB, Phase 1)
    /// No SQLite persistence in Phase 1
    pub scrollback: RingBuffer<Line>,

    /// Attached clients (for fan-out broadcast)
    /// Each client has an output channel for sending encoded Protocol messages
    pub clients: Vec<(ClientId, mpsc::Sender<Vec<u8>>)>,

    /// Timestamps
    pub created_at: Instant,
    pub last_activity: Instant,

    /// Monomind detection flag (SRS §2.4.1)
    pub monomind_detected: bool,
    // NOTE: AbortOnDrop pattern moved to SessionContainer (Aug 2026)
    // Task JoinHandles stored in SessionContainer (output_task, monomind_task)
    // SessionContainer::Drop aborts tasks to prevent memory leaks
    // See: crates/master/src/session/session.rs SessionContainer struct
}

impl Session {
    /// Create new session (metadata only - PTY in SessionContainer)
    /// AbortOnDrop pattern implemented via JoinHandle tracking in SessionContainer
    pub fn new(
        id: SessionId,
        shell_pid: u32,
        shell_type: String,
        working_dir: PathBuf,
        rows: u16,
        cols: u16,
    ) -> Self {
        Self {
            id,
            state: SessionState::Running,
            shell_pid,
            shell_type,
            dimensions: Dimensions { rows, cols },
            working_dir,
            scrollback: RingBuffer::new(10_000), // 10k lines capacity
            clients: Vec::new(),
            created_at: Instant::now(),
            last_activity: Instant::now(),
            monomind_detected: false,
        }
    }

    /// Get session snapshot for late-joiner sync
    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            id: self.id,
            scrollback: self.scrollback.iter().cloned().collect(),
            rows: self.dimensions.rows,
            cols: self.dimensions.cols,
            working_dir: self.working_dir.clone(),
            shell_type: self.shell_type.clone(),
        }
    }

    /// Attach client to session with output channel
    pub fn attach_client(&mut self, client_id: ClientId, output_tx: mpsc::Sender<Vec<u8>>) {
        // Check if already attached
        if !self.clients.iter().any(|(id, _)| *id == client_id) {
            self.clients.push((client_id, output_tx));
        }
    }

    /// Detach client from session
    pub fn detach_client(&mut self, client_id: ClientId) {
        self.clients.retain(|(id, _)| *id != client_id);
    }

    /// Get list of attached client IDs
    pub fn client_ids(&self) -> Vec<ClientId> {
        self.clients.iter().map(|(id, _)| *id).collect()
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        tracing::error!("🔴 DROP: Session dropped for session {}", self.id);
    }
}

impl Drop for SessionContainer {
    fn drop(&mut self) {
        // Get session id for logging (may fail if session lock is poisoned)
        let session_id = self
            .session
            .try_read()
            .ok()
            .map(|s| s.id.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        tracing::info!(
            "🔴 DROP: SessionContainer dropping for session {} - ABORTING TASKS",
            session_id
        );

        // CRITICAL: Abort background tasks to release Arc references immediately
        // Without this, tasks hold Arc<Session> and Arc<Pty> until natural EOF,
        // causing memory leaks during rapid session churn (smoke test: 18% growth in 5 min)
        if let Ok(mut task_guard) = self.output_task.try_lock() {
            if let Some(handle) = task_guard.take() {
                tracing::info!("🔴 DROP: Aborting output_task for session {}", session_id);
                handle.abort();
            }
        }

        if let Ok(mut task_guard) = self.monomind_task.try_lock() {
            if let Some(handle) = task_guard.take() {
                tracing::info!("🔴 DROP: Aborting monomind_task for session {}", session_id);
                handle.abort();
            }
        }

        tracing::info!(
            "🔴 DROP: SessionContainer dropped for session {} - tasks aborted",
            session_id
        );
    }
}

impl SessionContainer {
    /// Create new session container with session and PTY
    /// PTY and Session have separate locks to prevent attach_client deadlock
    /// Tasks are NOT spawned here - SessionManager spawns and stores handles
    pub fn new(
        id: SessionId,
        pty: Box<dyn crate::pty::PtyBackend>,
        shell_type: String,
        working_dir: PathBuf,
        rows: u16,
        cols: u16,
    ) -> Self {
        let shell_pid = pty.shell_pid();

        let session = Arc::new(RwLock::new(Session::new(
            id,
            shell_pid,
            shell_type,
            working_dir,
            rows,
            cols,
        )));

        let pty = Arc::new(Mutex::new(Some(pty)));

        Self {
            session,
            pty,
            output_task: Arc::new(Mutex::new(None)),
            monomind_task: Arc::new(Mutex::new(None)),
        }
    }

    /// Terminate the PTY session gracefully
    /// This sets PTY to None, signaling pty_output_loop to exit
    pub async fn terminate_pty(&self) -> Result<(), std::io::Error> {
        let mut pty_guard = self.pty.lock().await;

        if let Some(pty) = pty_guard.take() {
            let session_id = {
                let s = self.session.read().await;
                s.id
            };

            tracing::info!(
                "SessionContainer terminating PTY for session {}",
                session_id
            );

            // Mark session as terminated
            {
                let mut s = self.session.write().await;
                s.state = SessionState::Terminated;
            }

            // Drop PTY lock before calling terminate (PTY.terminate() may block)
            drop(pty_guard);

            // Call PTY terminate - consumes the Box<dyn PtyBackend>
            pty.terminate()
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

            tracing::info!(
                "SessionContainer PTY terminated successfully for session {}",
                session_id
            );
        } else {
            tracing::warn!("SessionContainer terminate_pty called but PTY already None");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state() {
        assert_eq!(SessionState::Running, SessionState::Running);
        assert_ne!(SessionState::Running, SessionState::Terminated);
    }

    #[test]
    fn test_dimensions() {
        let dims = Dimensions { rows: 24, cols: 80 };
        assert_eq!(dims.rows, 24);
        assert_eq!(dims.cols, 80);
    }

    #[test]
    fn test_line_from_bytes() {
        let line = Line::from_bytes(b"hello world");
        assert_eq!(line.data, b"hello world");
    }
}
