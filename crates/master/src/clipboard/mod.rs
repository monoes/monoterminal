// Clipboard Backend Enforcement
// Phase 4: Bidirectional Clipboard (ADR-020)
// Security: Rate limiting, size validation, audit logging

mod error;
mod rate_limiter;

#[cfg(test)]
mod tests;

pub use error::{ClipboardError, Result};
use rate_limiter::RateLimiter;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Session ID type alias
pub type SessionId = Uuid;

/// User ID type alias
pub type UserId = String;

/// Authorization grant with TTL
struct AuthGrant {
    granted_at: std::time::Instant,
    ttl: Duration,
}

impl AuthGrant {
    fn new(ttl: Duration) -> Self {
        Self {
            granted_at: std::time::Instant::now(),
            ttl,
        }
    }

    fn is_valid(&self) -> bool {
        std::time::Instant::now().duration_since(self.granted_at) < self.ttl
    }
}

/// Clipboard backend enforcement manager
///
/// Security controls (ADR-020):
/// 1. Rate limiting: 3 requests per 60 seconds per session
/// 2. Exponential backoff: [5s, 10s, 20s, 40s, 60s cap]
/// 3. Size validation: 1MB (1,048,576 bytes) hard limit
/// 4. Audit logging: All operations logged (server-side)
/// 5. Authorization cache: Per-session, 1-hour TTL
/// 6. Session isolation: No cross-session state leakage
pub struct ClipboardManager {
    /// Per-session rate limiters (3/60sec)
    rate_limiters: Arc<RwLock<HashMap<SessionId, RateLimiter>>>,

    /// Authorization cache (per-session, 1-hour TTL)
    auth_cache: Arc<RwLock<HashMap<SessionId, AuthGrant>>>,

    /// Audit logger reference (placeholder for now)
    /// TODO: Wire up to actual AuditLogger in Day 5
    _audit_logger: Option<Arc<()>>,
}

impl ClipboardManager {
    /// Create new clipboard manager
    pub fn new() -> Self {
        Self {
            rate_limiters: Arc::new(RwLock::new(HashMap::new())),
            auth_cache: Arc::new(RwLock::new(HashMap::new())),
            _audit_logger: None, // Wire up in Day 5
        }
    }

    /// Handle clipboard read request (server → client clipboard)
    ///
    /// Security checks:
    /// 1. Rate limit (3/60sec)
    /// 2. Authorization cache (1-hour TTL)
    /// 3. Size validation (1MB limit)
    ///
    /// # Arguments
    /// * `session_id` - Session requesting clipboard access
    /// * `user_id` - User ID from JWT (for audit logging)
    /// * `client_ip` - Client IP address (for audit logging)
    ///
    /// # Returns
    /// * `Ok(String)` - Clipboard content
    /// * `Err(ClipboardError)` - Rate limited, unauthorized, or system error
    pub async fn handle_clipboard_get(
        &self,
        session_id: SessionId,
        _user_id: UserId,
        _client_ip: SocketAddr,
    ) -> Result<String> {
        // 1. Check rate limit (3/60sec)
        self.check_rate_limit(session_id).await?;

        // 2. Check authorization cache (or require user prompt)
        self.check_authorization(session_id).await?;

        // 3. Read OS clipboard (placeholder - will use arboard crate in integration)
        let clipboard_data = self.read_system_clipboard()?;

        // 4. Validate size (1MB limit)
        if clipboard_data.len() > 1_048_576 {
            return Err(ClipboardError::SizeLimitExceeded);
        }

        // 5. Log audit event (TODO: Day 5 audit logging integration)
        // self.audit_logger.log_clipboard_read(session_id, user_id, client_ip, clipboard_data.len(), true).await;

        Ok(clipboard_data)
    }

    /// Handle clipboard write request (client → server clipboard)
    ///
    /// Security checks:
    /// 1. Rate limit (3/60sec)
    /// 2. Size validation (1MB limit)
    ///
    /// # Arguments
    /// * `session_id` - Session writing to clipboard
    /// * `content` - Clipboard content to write
    /// * `user_id` - User ID from JWT (for audit logging)
    /// * `client_ip` - Client IP address (for audit logging)
    ///
    /// # Returns
    /// * `Ok(())` - Write successful
    /// * `Err(ClipboardError)` - Rate limited, size exceeded, or system error
    pub async fn handle_clipboard_set(
        &self,
        session_id: SessionId,
        content: String,
        _user_id: UserId,
        _client_ip: SocketAddr,
    ) -> Result<()> {
        // 1. Check rate limit
        self.check_rate_limit(session_id).await?;

        // 2. Validate size (1MB limit)
        if content.len() > 1_048_576 {
            // Log denial (TODO: Day 5 audit logging)
            // self.audit_logger.log_clipboard_write(session_id, user_id, client_ip, content.len(), false, Some("SIZE_LIMIT_EXCEEDED")).await;
            return Err(ClipboardError::SizeLimitExceeded);
        }

        // 3. Write to OS clipboard
        self.write_system_clipboard(&content)?;

        // 4. Log audit event (TODO: Day 5 audit logging)
        // self.audit_logger.log_clipboard_write(session_id, user_id, client_ip, content.len(), true, None).await;

        Ok(())
    }

    /// Handle OSC 52 clipboard write (PTY → client clipboard)
    ///
    /// Note: No rate limiting (comes from PTY, not user)
    ///
    /// # Arguments
    /// * `session_id` - Session with PTY output
    /// * `content` - Clipboard content (base64 decoded)
    /// * `selection` - Selection type (clipboard/primary/secondary)
    /// * `user_id` - User ID (for audit logging)
    /// * `client_ip` - Client IP (for audit logging)
    pub async fn handle_osc52_clipboard(
        &self,
        _session_id: SessionId,
        content: String,
        _selection: String,
        _user_id: UserId,
        _client_ip: SocketAddr,
    ) -> Result<()> {
        // 1. Validate size (1MB limit)
        if content.len() > 1_048_576 {
            return Err(ClipboardError::SizeLimitExceeded);
        }

        // 2. Log audit event (TODO: Day 5 - source: OSC52)
        // self.audit_logger.log_clipboard_osc52(session_id, user_id, client_ip, content.len(), selection).await;

        // 3. Forward to client (handled by protocol layer)
        Ok(())
    }

    /// Grant clipboard authorization for session
    ///
    /// Called when user approves clipboard access via UI prompt
    pub async fn grant_authorization(&self, session_id: SessionId) {
        let mut cache = self.auth_cache.write().await;
        cache.insert(session_id, AuthGrant::new(Duration::from_secs(3600))); // 1-hour TTL
    }

    /// Revoke clipboard authorization for session
    pub async fn revoke_authorization(&self, session_id: SessionId) {
        let mut cache = self.auth_cache.write().await;
        cache.remove(&session_id);
    }

    /// Check rate limit for session (3/60sec)
    async fn check_rate_limit(&self, session_id: SessionId) -> Result<()> {
        let mut limiters = self.rate_limiters.write().await;
        let limiter = limiters
            .entry(session_id)
            .or_insert_with(|| RateLimiter::new(3, Duration::from_secs(60)));

        limiter.check_request()
    }

    /// Check authorization cache (1-hour TTL)
    async fn check_authorization(&self, session_id: SessionId) -> Result<()> {
        let cache = self.auth_cache.read().await;

        if let Some(grant) = cache.get(&session_id) {
            if grant.is_valid() {
                return Ok(());
            }
        }

        // Not in cache or expired - client needs to prompt user
        Err(ClipboardError::AuthorizationRequired)
    }

    /// Read system clipboard (placeholder)
    ///
    /// TODO: Wire up arboard crate for actual OS clipboard access
    fn read_system_clipboard(&self) -> Result<String> {
        // Placeholder: Return empty string for now
        // Real implementation will use arboard crate:
        // use arboard::Clipboard;
        // let mut clipboard = Clipboard::new().map_err(|e| ClipboardError::SystemClipboardError(e.to_string()))?;
        // clipboard.get_text().map_err(|e| ClipboardError::SystemClipboardError(e.to_string()))
        Ok(String::new())
    }

    /// Write system clipboard (placeholder)
    ///
    /// TODO: Wire up arboard crate for actual OS clipboard access
    fn write_system_clipboard(&self, _content: &str) -> Result<()> {
        // Placeholder: No-op for now
        // Real implementation will use arboard crate:
        // use arboard::Clipboard;
        // let mut clipboard = Clipboard::new().map_err(|e| ClipboardError::SystemClipboardError(e.to_string()))?;
        // clipboard.set_text(content).map_err(|e| ClipboardError::SystemClipboardError(e.to_string()))
        Ok(())
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
