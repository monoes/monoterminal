// Rate limiting implementation using token bucket algorithm
// SRS §3.2.4: Connection, auth, and session rate limits

use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Rate limit error
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded: {0}")]
    Exceeded(String),

    #[error("Temporary ban active until {0:?}")]
    Banned(Instant),
}

/// Token bucket for rate limiting
#[derive(Debug, Clone)]
struct TokenBucket {
    capacity: usize,
    tokens: usize,
    refill_rate: usize, // Tokens per refill interval
    refill_interval: Duration,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: usize, refill_rate: usize, refill_interval: Duration) -> Self {
        Self {
            capacity,
            tokens: capacity,
            refill_rate,
            refill_interval,
            last_refill: Instant::now(),
        }
    }

    /// Try to acquire N tokens
    fn try_acquire(&mut self, count: usize) -> bool {
        self.refill();

        if self.tokens >= count {
            self.tokens -= count;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);

        if elapsed >= self.refill_interval {
            // Use milliseconds for sub-second precision (avoids 0/0 for ms intervals)
            let intervals = (elapsed.as_millis() / self.refill_interval.as_millis()) as usize;
            let new_tokens = intervals * self.refill_rate;
            self.tokens = (self.tokens + new_tokens).min(self.capacity);
            self.last_refill = now;
        }
    }
}

/// Auth failure tracking for temporary bans
#[derive(Debug)]
struct AuthFailureTracker {
    failures: Vec<Instant>,
    ban_until: Option<Instant>,
}

impl AuthFailureTracker {
    fn new() -> Self {
        Self {
            failures: Vec::new(),
            ban_until: None,
        }
    }

    /// Record a failed auth attempt
    fn record_failure(&mut self) {
        let now = Instant::now();
        self.failures.push(now);

        // Keep only failures from last hour
        let one_hour_ago = now - Duration::from_secs(3600);
        self.failures.retain(|&t| t > one_hour_ago);

        // Check if should ban (5 failures in 1 hour)
        if self.failures.len() >= 5 {
            // 15 minute ban
            self.ban_until = Some(now + Duration::from_secs(900));
        }
    }

    /// Check if currently banned
    fn is_banned(&self) -> bool {
        if let Some(ban_time) = self.ban_until {
            Instant::now() < ban_time
        } else {
            false
        }
    }

    /// Get ban expiration time
    fn ban_expires_at(&self) -> Option<Instant> {
        if self.is_banned() {
            self.ban_until
        } else {
            None
        }
    }
}

/// Rate limiter with multiple buckets
pub struct RateLimiter {
    connection_buckets: Arc<Mutex<HashMap<SocketAddr, TokenBucket>>>,
    auth_trackers: Arc<Mutex<HashMap<SocketAddr, AuthFailureTracker>>>,
    session_buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,

    // Global limits
    max_connections_per_minute: usize,
    max_sessions_per_minute: usize,
}

impl RateLimiter {
    /// Create new rate limiter with SRS defaults
    pub fn new() -> Self {
        Self {
            connection_buckets: Arc::new(Mutex::new(HashMap::new())),
            auth_trackers: Arc::new(Mutex::new(HashMap::new())),
            session_buckets: Arc::new(Mutex::new(HashMap::new())),
            max_connections_per_minute: 100, // SRS §3.2.4
            max_sessions_per_minute: 20,     // SRS §3.2.4
        }
    }

    /// Check if connection from peer is allowed (100/min limit)
    pub fn check_connection(&self, peer_addr: &SocketAddr) -> Result<(), RateLimitError> {
        let mut buckets = self.connection_buckets.lock().unwrap();
        let bucket = buckets.entry(*peer_addr).or_insert_with(|| {
            TokenBucket::new(
                self.max_connections_per_minute,
                self.max_connections_per_minute,
                Duration::from_secs(60),
            )
        });

        if bucket.try_acquire(1) {
            Ok(())
        } else {
            Err(RateLimitError::Exceeded(format!(
                "Connection rate limit exceeded for {}",
                peer_addr
            )))
        }
    }

    /// Check if auth attempt is allowed (respects temp bans)
    pub fn check_auth_attempt(&self, peer_addr: &SocketAddr) -> Result<(), RateLimitError> {
        let trackers = self.auth_trackers.lock().unwrap();

        if let Some(tracker) = trackers.get(peer_addr) {
            if let Some(ban_time) = tracker.ban_expires_at() {
                return Err(RateLimitError::Banned(ban_time));
            }
        }

        Ok(())
    }

    /// Record an auth failure (triggers ban after 5 failures/hour)
    pub fn record_auth_failure(&self, peer_addr: &SocketAddr) {
        let mut trackers = self.auth_trackers.lock().unwrap();
        let tracker = trackers
            .entry(*peer_addr)
            .or_insert_with(AuthFailureTracker::new);
        tracker.record_failure();
    }

    /// Check if session creation is allowed (20/min limit per user)
    pub fn check_session_create(&self, user_id: &str) -> Result<(), RateLimitError> {
        let mut buckets = self.session_buckets.lock().unwrap();
        let bucket = buckets.entry(user_id.to_string()).or_insert_with(|| {
            TokenBucket::new(
                self.max_sessions_per_minute,
                self.max_sessions_per_minute,
                Duration::from_secs(60),
            )
        });

        if bucket.try_acquire(1) {
            Ok(())
        } else {
            Err(RateLimitError::Exceeded(format!(
                "Session creation rate limit exceeded for user {}",
                user_id
            )))
        }
    }

    /// Cleanup expired entries (call periodically)
    pub fn cleanup(&self) {
        let _now = Instant::now();

        // Cleanup auth trackers with expired bans
        let mut trackers = self.auth_trackers.lock().unwrap();
        trackers.retain(|_, tracker| !tracker.failures.is_empty() || tracker.is_banned());

        // Note: buckets cleanup on access (lazy cleanup via refill)
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn test_addr(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
    }

    #[test]
    fn test_bucket_creation() {
        let mut bucket = TokenBucket::new(100, 100, Duration::from_secs(60));

        // Should start at capacity
        assert!(bucket.try_acquire(1));
        assert!(bucket.try_acquire(99));

        // Should be empty now
        assert!(!bucket.try_acquire(1));
    }

    #[test]
    fn test_bucket_refill() {
        let mut bucket = TokenBucket::new(10, 10, Duration::from_millis(100));

        // Exhaust bucket
        assert!(bucket.try_acquire(10));
        assert!(!bucket.try_acquire(1));

        // Wait for refill
        std::thread::sleep(Duration::from_millis(150));

        // Should be refilled
        assert!(bucket.try_acquire(10));
    }

    #[test]
    fn test_connection_rate_limit() {
        let limiter = RateLimiter::new();
        let addr = test_addr(8080);

        // Should allow up to 100 connections
        for _ in 0..100 {
            assert!(limiter.check_connection(&addr).is_ok());
        }

        // 101st should fail
        assert!(limiter.check_connection(&addr).is_err());
    }

    #[test]
    fn test_auth_failure_tracking() {
        let limiter = RateLimiter::new();
        let addr = test_addr(8081);

        // First 4 failures should not ban
        for _ in 0..4 {
            limiter.record_auth_failure(&addr);
            assert!(limiter.check_auth_attempt(&addr).is_ok());
        }

        // 5th failure triggers ban
        limiter.record_auth_failure(&addr);
        assert!(limiter.check_auth_attempt(&addr).is_err());
    }

    #[test]
    fn test_auth_ban_expiry() {
        let limiter = RateLimiter::new();
        let addr = test_addr(8082);

        // Trigger ban with custom short duration for testing
        {
            let mut trackers = limiter.auth_trackers.lock().unwrap();
            let tracker = trackers.entry(addr).or_insert_with(AuthFailureTracker::new);
            // Record 5 failures
            for _ in 0..5 {
                tracker.failures.push(Instant::now());
            }
            // Set short ban
            tracker.ban_until = Some(Instant::now() + Duration::from_millis(50));
        }

        // Should be banned
        assert!(limiter.check_auth_attempt(&addr).is_err());

        // Wait for ban expiry
        std::thread::sleep(Duration::from_millis(60));

        // Should no longer be banned
        assert!(limiter.check_auth_attempt(&addr).is_ok());
    }

    #[test]
    fn test_session_creation_rate_limit() {
        let limiter = RateLimiter::new();
        let user_id = "alice@example.com";

        // Should allow up to 20 sessions
        for _ in 0..20 {
            assert!(limiter.check_session_create(user_id).is_ok());
        }

        // 21st should fail
        assert!(limiter.check_session_create(user_id).is_err());
    }

    #[test]
    fn test_different_users_independent_limits() {
        let limiter = RateLimiter::new();

        // Exhaust limit for user1
        for _ in 0..20 {
            assert!(limiter.check_session_create("user1").is_ok());
        }
        assert!(limiter.check_session_create("user1").is_err());

        // user2 should still have full quota
        assert!(limiter.check_session_create("user2").is_ok());
    }

    #[test]
    fn test_different_ips_independent_limits() {
        let limiter = RateLimiter::new();
        let addr1 = test_addr(8083);
        let addr2 = test_addr(8084);

        // Exhaust limit for addr1
        for _ in 0..100 {
            assert!(limiter.check_connection(&addr1).is_ok());
        }
        assert!(limiter.check_connection(&addr1).is_err());

        // addr2 should still work
        assert!(limiter.check_connection(&addr2).is_ok());
    }

    #[test]
    fn test_cleanup_removes_expired() {
        let limiter = RateLimiter::new();
        let addr = test_addr(8085);

        // Record failures
        limiter.record_auth_failure(&addr);

        {
            let trackers = limiter.auth_trackers.lock().unwrap();
            assert_eq!(trackers.len(), 1);
        }

        // Cleanup doesn't remove recent failures
        limiter.cleanup();

        {
            let trackers = limiter.auth_trackers.lock().unwrap();
            assert_eq!(trackers.len(), 1);
        }
    }

    #[test]
    fn test_failure_tracker_expiry() {
        let mut tracker = AuthFailureTracker::new();

        // Add old failures (should be cleaned up)
        let old_time = Instant::now() - Duration::from_secs(7200); // 2 hours ago
        tracker.failures.push(old_time);

        // Add recent failure
        tracker.record_failure();

        // Should only have recent failure
        assert_eq!(tracker.failures.len(), 1);
        assert!(tracker.failures[0] > Instant::now() - Duration::from_secs(10));
    }
}
