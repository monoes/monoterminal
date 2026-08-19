// Ring buffer implementation for scrollback
// Phase 1: In-memory only (10k lines, ~1MB)
// Phase 2+: SQLite persistence for unlimited history

use std::collections::VecDeque;

/// Fixed-capacity ring buffer for scrollback lines
/// FIFO: oldest lines are dropped when capacity is reached
pub struct RingBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    /// Create new ring buffer with given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push item to buffer (drops oldest if at capacity)
    pub fn push(&mut self, item: T) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
    }

    /// Push line data to buffer (alias for push)
    pub fn push_line(&mut self, item: T) {
        self.push(item);
    }

    /// Get current length
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Check if full (at capacity)
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.capacity
    }

    /// Get iterator over items
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let mut buf = RingBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.len(), 3);

        // Push 4th item, should drop first
        buf.push(4);
        assert_eq!(buf.len(), 3);
        let items: Vec<_> = buf.iter().copied().collect();
        assert_eq!(items, vec![2, 3, 4]);
    }

    #[test]
    fn test_ring_buffer_clear() {
        let mut buf = RingBuffer::new(10);
        buf.push(1);
        buf.push(2);
        assert_eq!(buf.len(), 2);

        buf.clear();
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
    }
}
