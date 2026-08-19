// WebSocket connection management
// Implements SRS §3.1.4 (Output Buffering & Flow Control)

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tracing::{debug, warn};

/// Per-client connection state
pub struct Connection {
    /// Client identifier
    pub client_id: String,

    /// Output buffer (bounded queue - SRS §3.1.4: 1 MB limit)
    /// Each OutputData message is ~4KB typical, so 256 messages ≈ 1MB
    pub output_tx: mpsc::Sender<Vec<u8>>,

    /// Connection timestamp
    pub connected_at: Instant,

    /// Compression support
    pub supports_compression: bool,

    /// Lagging detection
    pub lagging_since: Option<Instant>,
}

impl Connection {
    /// Create a new connection
    pub fn new(client_id: String, supports_compression: bool) -> (Self, mpsc::Receiver<Vec<u8>>) {
        // SRS §3.1.4: 1 MB bounded queue
        // Assuming 4KB per message: 256 messages ≈ 1MB
        let (output_tx, output_rx) = mpsc::channel(256);

        let conn = Self {
            client_id,
            output_tx,
            connected_at: Instant::now(),
            supports_compression,
            lagging_since: None,
        };

        (conn, output_rx)
    }

    /// Send output data to client
    /// Returns Ok(()) if sent, Err if buffer full (client lagging)
    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), ()> {
        match self.output_tx.try_send(data) {
            Ok(_) => {
                // Reset lagging state
                if self.lagging_since.is_some() {
                    debug!("Client {} recovered from lagging", self.client_id);
                    self.lagging_since = None;
                }
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Buffer full - client is lagging
                if self.lagging_since.is_none() {
                    self.lagging_since = Some(Instant::now());
                    warn!("Client {} buffer full - marked as lagging", self.client_id);
                }
                Err(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                warn!("Client {} channel closed", self.client_id);
                Err(())
            }
        }
    }

    /// Check if client is lagging
    /// SRS §3.1.4: Disconnect if lagging >30s
    pub fn is_lagging(&self) -> bool {
        self.lagging_since.is_some()
    }

    /// Check if client should be disconnected due to prolonged lagging
    pub fn should_disconnect(&self) -> bool {
        if let Some(lagging_since) = self.lagging_since {
            let lagging_duration = Instant::now() - lagging_since;
            lagging_duration > Duration::from_secs(30)
        } else {
            false
        }
    }

    /// Get buffer fill percentage (0-100)
    pub fn buffer_fill_percentage(&self) -> u8 {
        let capacity = self.output_tx.capacity();
        let available = self.output_tx.max_capacity() - capacity;
        let used = self.output_tx.max_capacity().saturating_sub(available);
        ((used as f32 / self.output_tx.max_capacity() as f32) * 100.0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_send() {
        let (mut conn, mut rx) = Connection::new("test-client".to_string(), false);

        // Send data
        let data = vec![1, 2, 3, 4];
        assert!(conn.send(data.clone()).await.is_ok());

        // Receive data
        let received = rx.recv().await.unwrap();
        assert_eq!(received, data);
    }

    #[tokio::test]
    async fn test_connection_lagging() {
        let (mut conn, _rx) = Connection::new("test-client".to_string(), false);

        // Fill buffer
        for i in 0..256 {
            let data = vec![i as u8; 4096];
            conn.send(data).await.ok();
        }

        // Next send should fail
        let data = vec![1, 2, 3, 4];
        assert!(conn.send(data).await.is_err());
        assert!(conn.is_lagging());
    }
}
