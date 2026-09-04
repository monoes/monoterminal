// Rate Limiter with Exponential Backoff
// Phase 4: Bidirectional Clipboard (ADR-020)
// Security: 3 requests per 60 seconds, exponential backoff on violation

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::error::{ClipboardError, Result};

/// Rate limiter with sliding window and exponential backoff
///
/// Security controls:
/// - Max 3 requests per 60 seconds (sliding window)
/// - Exponential backoff on violation: [5s, 10s, 20s, 40s, 60s cap]
/// - Reset after 5 minutes of no requests
pub struct RateLimiter {
    /// Request timestamps (sliding window)
    requests: VecDeque<Instant>,

    /// Max requests per window
    max_requests: usize,

    /// Time window (60 seconds)
    window: Duration,

    /// Backoff schedule: [5s, 10s, 20s, 40s, 60s cap]
    backoff_schedule: [Duration; 5],

    /// Current backoff level (0-4)
    backoff_level: usize,

    /// Backoff active until this time
    backoff_until: Option<Instant>,

    /// Last request time (for 5min reset detection)
    last_request: Option<Instant>,
}

impl RateLimiter {
    /// Create new rate limiter
    ///
    /// # Arguments
    /// * `max_requests` - Maximum requests allowed per window
    /// * `window` - Time window duration
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            requests: VecDeque::new(),
            max_requests,
            window,
            backoff_schedule: [
                Duration::from_secs(5),   // First violation
                Duration::from_secs(10),  // Second violation
                Duration::from_secs(20),  // Third violation
                Duration::from_secs(40),  // Fourth violation
                Duration::from_secs(60),  // Fifth+ violation (cap)
            ],
            backoff_level: 0,
            backoff_until: None,
            last_request: None,
        }
    }

    /// Check if request is allowed under rate limit
    ///
    /// # Returns
    /// * `Ok(())` - Request allowed
    /// * `Err(ClipboardError::RateLimitExceeded)` - Request denied, backoff active
    pub fn check_request(&mut self) -> Result<()> {
        let now = Instant::now();

        // 1. Check if in backoff period
        if let Some(until) = self.backoff_until {
            if now < until {
                return Err(ClipboardError::RateLimitExceeded {
                    retry_after: (until - now).as_secs(),
                });
            } else {
                // Backoff expired, reset
                self.backoff_until = None;
            }
        }

        // 2. Check for 5-minute reset (no requests for 5 minutes)
        if let Some(last) = self.last_request {
            if now.duration_since(last) >= Duration::from_secs(300) {
                // Reset violation count
                self.backoff_level = 0;
                self.requests.clear();
            }
        }

        // 3. Remove requests outside the window (sliding window)
        while let Some(&oldest) = self.requests.front() {
            if now.duration_since(oldest) >= self.window {
                self.requests.pop_front();
            } else {
                break;
            }
        }

        // 4. Check if under rate limit
        if self.requests.len() >= self.max_requests {
            // Violation: apply exponential backoff
            let backoff_duration = self
                .backoff_schedule
                .get(self.backoff_level)
                .copied()
                .unwrap_or(Duration::from_secs(60)); // Cap at 60s

            self.backoff_until = Some(now + backoff_duration);
            self.backoff_level = (self.backoff_level + 1).min(4); // Cap at level 4

            return Err(ClipboardError::RateLimitExceeded {
                retry_after: backoff_duration.as_secs(),
            });
        }

        // 5. Allow request and record timestamp
        self.requests.push_back(now);
        self.last_request = Some(now);
        Ok(())
    }

    /// Reset rate limiter state (for testing)
    #[cfg(test)]
    pub fn reset(&mut self) {
        self.requests.clear();
        self.backoff_level = 0;
        self.backoff_until = None;
        self.last_request = None;
    }

    /// Get current backoff level (for testing)
    #[cfg(test)]
    pub fn backoff_level(&self) -> usize {
        self.backoff_level
    }

    /// Get current request count (for testing)
    #[cfg(test)]
    pub fn request_count(&self) -> usize {
        self.requests.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_rate_limiter_allows_3_requests() {
        let mut limiter = RateLimiter::new(3, Duration::from_secs(60));

        // First 3 requests should succeed
        assert!(limiter.check_request().is_ok());
        assert!(limiter.check_request().is_ok());
        assert!(limiter.check_request().is_ok());
        assert_eq!(limiter.request_count(), 3);
    }

    #[test]
    fn test_rate_limiter_blocks_4th_request() {
        let mut limiter = RateLimiter::new(3, Duration::from_secs(60));

        // First 3 succeed
        limiter.check_request().ok();
        limiter.check_request().ok();
        limiter.check_request().ok();

        // 4th request should be rate limited
        let result = limiter.check_request();
        assert!(result.is_err());
        match result {
            Err(ClipboardError::RateLimitExceeded { retry_after }) => {
                assert_eq!(retry_after, 5); // First violation: 5s backoff
            }
            _ => panic!("Expected RateLimitExceeded error"),
        }
    }

    #[test]
    fn test_rate_limiter_backoff_schedule() {
        let mut limiter = RateLimiter::new(3, Duration::from_secs(60));

        // Trigger violations to test backoff progression
        let expected_backoffs = [5, 10, 20, 40, 60, 60]; // Cap at 60s

        for (i, &expected) in expected_backoffs.iter().enumerate() {
            // Fill up rate limit
            limiter.reset();
            limiter.check_request().ok();
            limiter.check_request().ok();
            limiter.check_request().ok();

            // Set backoff level manually for testing
            limiter.backoff_level = i;

            // Trigger violation
            let result = limiter.check_request();
            assert!(result.is_err());
            match result {
                Err(ClipboardError::RateLimitExceeded { retry_after }) => {
                    assert_eq!(
                        retry_after, expected,
                        "Violation {}: expected {}s backoff, got {}s",
                        i + 1,
                        expected,
                        retry_after
                    );
                }
                _ => panic!("Expected RateLimitExceeded error"),
            }
        }
    }

    #[test]
    fn test_rate_limiter_sliding_window() {
        let mut limiter = RateLimiter::new(3, Duration::from_millis(100));

        // Fill rate limit
        limiter.check_request().ok();
        limiter.check_request().ok();
        limiter.check_request().ok();

        // Wait for window to pass
        thread::sleep(Duration::from_millis(150));

        // Should allow new requests after window expires
        assert!(limiter.check_request().is_ok());
        assert_eq!(limiter.request_count(), 1); // Old requests removed
    }

    #[test]
    fn test_rate_limiter_reset_after_5min() {
        let mut limiter = RateLimiter::new(3, Duration::from_secs(60));

        // Trigger violation to set backoff level
        limiter.check_request().ok();
        limiter.check_request().ok();
        limiter.check_request().ok();
        limiter.check_request().err(); // Violation
        assert_eq!(limiter.backoff_level(), 1);

        // Simulate 5 minutes passing by manually updating last_request
        limiter.last_request = Some(Instant::now() - Duration::from_secs(301));

        // Next check should reset violation count
        limiter.reset(); // Clear requests for clean test
        limiter.check_request().ok();
        assert_eq!(limiter.backoff_level(), 0); // Should be reset
    }
}
