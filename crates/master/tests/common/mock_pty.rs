// Mock PTY for unit testing without real ConPTY
// As specified in test-strategy-phase1.md §6.1

use std::io;
use tokio::sync::mpsc;

/// Mock PTY for unit testing
pub struct MockPty {
    input_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    output_tx: mpsc::UnboundedSender<Vec<u8>>,
    dimensions: (u16, u16),
}

impl MockPty {
    /// Create a new mock PTY and return handle
    pub fn new() -> (Self, MockPtyHandle) {
        let (input_tx, input_rx) = mpsc::unbounded_channel();
        let (output_tx, output_rx) = mpsc::unbounded_channel();

        let pty = MockPty {
            input_rx,
            output_tx,
            dimensions: (80, 24),
        };

        let handle = MockPtyHandle {
            input_tx,
            output_rx,
        };

        (pty, handle)
    }

    /// Read data from PTY (simulated)
    pub async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(data) = self.input_rx.recv().await {
            let len = buf.len().min(data.len());
            buf[..len].copy_from_slice(&data[..len]);
            Ok(len)
        } else {
            Ok(0)
        }
    }

    /// Write data to PTY (simulated)
    pub async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.output_tx
            .send(buf.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Output channel closed"))?;
        Ok(buf.len())
    }

    /// Resize PTY dimensions
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.dimensions = (rows, cols);
    }

    /// Get current dimensions
    pub fn dimensions(&self) -> (u16, u16) {
        self.dimensions
    }
}

impl Default for MockPty {
    fn default() -> Self {
        Self::new().0
    }
}

/// Handle for interacting with MockPty from test code
pub struct MockPtyHandle {
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
    output_rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl MockPtyHandle {
    /// Send input to the mock PTY
    pub fn send_input(&self, data: &[u8]) -> Result<(), mpsc::error::SendError<Vec<u8>>> {
        self.input_tx.send(data.to_vec())
    }

    /// Receive output from the mock PTY
    pub async fn recv_output(&mut self) -> Option<Vec<u8>> {
        self.output_rx.recv().await
    }

    /// Try to receive output without blocking
    #[allow(dead_code)]
    pub fn try_recv_output(&mut self) -> Result<Vec<u8>, mpsc::error::TryRecvError> {
        self.output_rx.try_recv()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_pty_read_write() {
        let (mut pty, mut handle) = MockPty::new();

        // Send input to PTY
        handle.send_input(b"hello").unwrap();

        // Read from PTY
        let mut buf = [0u8; 10];
        let n = pty.read(&mut buf).await.unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf[..n], b"hello");

        // Write to PTY
        pty.write(b"world").await.unwrap();

        // Receive output
        let output = handle.recv_output().await.unwrap();
        assert_eq!(output, b"world");
    }

    #[test]
    fn test_mock_pty_resize() {
        let (mut pty, _handle) = MockPty::new();

        assert_eq!(pty.dimensions(), (80, 24));

        pty.resize(100, 30);
        assert_eq!(pty.dimensions(), (100, 30));
    }
}
