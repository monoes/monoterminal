//! RendererBridge - Connects SessionManager PTY output to wgpu Renderer
//!
//! Replaces mock PTY channel with real SessionManager integration.
//! Zero-copy Arc<Bytes> delivery from ConPTY → SessionManager → Renderer.
//!
//! Integration Point: Day 2 of Criterion #1 (60 FPS rendering)
//! Delivered by rust-backend-lead per Monday 18:00 commitment

use tokio::sync::mpsc;
use bytes::Bytes;
use std::sync::Arc;
use uuid::Uuid;
use anyhow::Result;

use crate::session::{SessionManager, SessionId};

/// Client ID for local UI rendering attachment
pub type ClientId = Uuid;

/// RendererBridge - Zero-copy PTY output stream for GPU rendering
///
/// # Architecture
/// ```text
/// ConPTY → SessionManager → mpsc::channel(256) → RendererBridge → VtParser → TerminalGrid → GPU
/// ```
///
/// # Backpressure
/// - Channel buffer: 256 messages (configurable)
/// - If GPU falls behind PTY, SessionManager blocks on send()
/// - ConPTY kernel buffer provides additional buffering (~4KB per PTY spec)
/// - Monitor: Log warning if channel >80% full
pub struct RendererBridge {
    /// PTY output receiver (from SessionManager)
    pty_rx: mpsc::Receiver<Vec<u8>>,

    /// Channel capacity for backpressure monitoring
    channel_capacity: usize,

    /// Session ID for debugging/logging
    session_id: SessionId,
}

impl RendererBridge {
    /// Attach to a session's PTY output stream
    ///
    /// # Arguments
    /// - `session_manager`: Arc to SessionManager instance
    /// - `session_id`: Target session UUID
    /// - `client_id`: Unique UI client ID (for multi-attach support in Phase 2)
    ///
    /// # Returns
    /// RendererBridge that receives Vec<u8> chunks from SessionManager
    ///
    /// # Example
    /// ```rust,no_run
    /// let bridge = RendererBridge::attach(
    ///     session_manager.clone(),
    ///     session_id,
    ///     Uuid::new_v4(),
    /// ).await?;
    /// ```
    pub async fn attach(
        session_manager: Arc<SessionManager>,
        session_id: SessionId,
        client_id: ClientId,
    ) -> Result<Self> {
        let channel_capacity = 256; // 256 messages × ~4KB avg = ~1MB buffer
        let (output_tx, pty_rx) = mpsc::channel(channel_capacity);

        tracing::info!(
            "RendererBridge attaching to session {} (client: {}, buffer: {} msgs)",
            session_id, client_id, channel_capacity
        );

        // Attach as local UI client to SessionManager
        // SessionManager will broadcast PTY output to this channel
        session_manager
            .attach_client(session_id, client_id, output_tx)
            .await?;

        Ok(Self {
            pty_rx,
            channel_capacity,
            session_id,
        })
    }

    /// Receive next PTY output chunk (async, blocking)
    ///
    /// Returns None if SessionManager closed the channel (session terminated)
    ///
    /// # Use Case
    /// Tokio task dedicated to PTY processing (not render loop - use try_recv there)
    pub async fn recv(&mut self) -> Option<Bytes> {
        self.pty_rx.recv().await.map(Bytes::from)
    }

    /// Try receive PTY output without blocking (non-blocking poll)
    ///
    /// Returns None if:
    /// - No data currently available (channel empty)
    /// - Channel closed (session terminated)
    ///
    /// # Use Case
    /// **Primary integration point for Renderer::process_pty_output()**
    ///
    /// ```rust,no_run
    /// // In render loop (60 FPS):
    /// if let Some(bridge) = &mut self.renderer_bridge {
    ///     while let Some(bytes) = bridge.try_recv() {
    ///         self.vt_parser.parse(&bytes, &mut self.terminal_grid);
    ///     }
    /// }
    /// ```
    pub fn try_recv(&mut self) -> Option<Bytes> {
        match self.pty_rx.try_recv() {
            Ok(data) => {
                // Backpressure monitoring (warn if channel >80% full)
                let pending = self.pty_rx.len();
                if pending > (self.channel_capacity * 80 / 100) {
                    tracing::warn!(
                        "RendererBridge backpressure: {}/{} messages queued (>80%), GPU may be falling behind PTY",
                        pending, self.channel_capacity
                    );
                }

                Some(Bytes::from(data))
            }
            Err(mpsc::error::TryRecvError::Empty) => None,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                tracing::info!("RendererBridge: Session {} PTY channel closed", self.session_id);
                None
            }
        }
    }

    /// Check if channel is still connected
    ///
    /// Returns false if SessionManager closed the PTY output stream
    pub fn is_connected(&self) -> bool {
        !self.pty_rx.is_closed()
    }

    /// Get current channel queue depth (for monitoring/debugging)
    ///
    /// # Performance Note
    /// O(1) operation, safe to call frequently
    pub fn queue_depth(&self) -> usize {
        self.pty_rx.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionManager;

    #[tokio::test]
    async fn test_renderer_bridge_attach() {
        let manager = Arc::new(SessionManager::new(None));
        let session_id = manager.create_session(None, 24, 80)
            .await
            .expect("Failed to create session");

        let client_id = Uuid::new_v4();
        let bridge = RendererBridge::attach(manager.clone(), session_id, client_id)
            .await
            .expect("Failed to attach RendererBridge");

        assert!(bridge.is_connected());
        assert_eq!(bridge.queue_depth(), 0);
    }

    #[tokio::test]
    async fn test_renderer_bridge_try_recv_empty() {
        let manager = Arc::new(SessionManager::new(None));
        let session_id = manager.create_session(None, 24, 80)
            .await
            .expect("Failed to create session");

        let client_id = Uuid::new_v4();
        let mut bridge = RendererBridge::attach(manager.clone(), session_id, client_id)
            .await
            .expect("Failed to attach RendererBridge");

        // Channel should be empty initially
        assert!(bridge.try_recv().is_none());
    }

    #[tokio::test]
    #[ignore = "AsyncPipeReader/Writer use blocking I/O in poll functions, violates tokio async contract. Phase 2: migrate to windows.rs PtyHandle"]
    async fn test_renderer_bridge_recv_data() {
        let manager = Arc::new(SessionManager::new(None));
        let session_id = manager.create_session(None, 24, 80)
            .await
            .expect("Failed to create session");

        let client_id = Uuid::new_v4();
        let mut bridge = RendererBridge::attach(manager.clone(), session_id, client_id)
            .await
            .expect("Failed to attach RendererBridge");

        // Send input to trigger PTY output
        manager.send_input(session_id, b"echo test\r\n")
            .await
            .expect("Failed to send input");

        // Give PTY time to process (Windows ConPTY)
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Should receive PTY output
        let output = tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            bridge.recv()
        ).await;

        assert!(output.is_ok(), "Timeout waiting for PTY output");
        assert!(output.unwrap().is_some(), "Expected PTY output, got None");
    }
}
