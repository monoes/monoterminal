// Clipboard Manager Unit Tests
// Phase 4: Bidirectional Clipboard (ADR-020)

use super::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use uuid::Uuid;

/// Helper: Create test socket address
fn test_socket_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)
}

#[tokio::test]
async fn test_clipboard_get_requires_authorization() {
    let manager = ClipboardManager::new();
    let session_id = Uuid::new_v4();
    let user_id = "test-user".to_string();
    let client_ip = test_socket_addr();

    // Without authorization, should fail
    let result = manager
        .handle_clipboard_get(session_id, user_id.clone(), client_ip)
        .await;

    assert!(result.is_err());
    match result {
        Err(ClipboardError::AuthorizationRequired) => {}
        _ => panic!("Expected AuthorizationRequired error"),
    }
}

#[tokio::test]
async fn test_clipboard_get_success_with_authorization() {
    let manager = ClipboardManager::new();
    let session_id = Uuid::new_v4();
    let user_id = "test-user".to_string();
    let client_ip = test_socket_addr();

    // Grant authorization
    manager.grant_authorization(session_id).await;

    // Should succeed
    let result = manager
        .handle_clipboard_get(session_id, user_id, client_ip)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_clipboard_set_success() {
    let manager = ClipboardManager::new();
    let session_id = Uuid::new_v4();
    let user_id = "test-user".to_string();
    let client_ip = test_socket_addr();
    let content = "test clipboard content".to_string();

    // Should succeed (no authorization required for write)
    let result = manager
        .handle_clipboard_set(session_id, content, user_id, client_ip)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_clipboard_set_size_limit_exceeded() {
    let manager = ClipboardManager::new();
    let session_id = Uuid::new_v4();
    let user_id = "test-user".to_string();
    let client_ip = test_socket_addr();

    // Create content > 1MB
    let large_content = "x".repeat(1_048_577); // 1MB + 1 byte

    let result = manager
        .handle_clipboard_set(session_id, large_content, user_id, client_ip)
        .await;

    assert!(result.is_err());
    match result {
        Err(ClipboardError::SizeLimitExceeded) => {}
        _ => panic!("Expected SizeLimitExceeded error"),
    }
}

#[tokio::test]
async fn test_clipboard_get_rate_limit_exceeded() {
    let manager = ClipboardManager::new();
    let session_id = Uuid::new_v4();
    let user_id = "test-user".to_string();
    let client_ip = test_socket_addr();

    // Grant authorization
    manager.grant_authorization(session_id).await;

    // First 3 requests should succeed
    assert!(manager.handle_clipboard_get(session_id, user_id.clone(), client_ip).await.is_ok());
    assert!(manager.handle_clipboard_get(session_id, user_id.clone(), client_ip).await.is_ok());
    assert!(manager.handle_clipboard_get(session_id, user_id.clone(), client_ip).await.is_ok());

    // 4th request should be rate limited
    let result = manager.handle_clipboard_get(session_id, user_id, client_ip).await;
    assert!(result.is_err());
    match result {
        Err(ClipboardError::RateLimitExceeded { retry_after }) => {
            assert_eq!(retry_after, 5); // First violation: 5s backoff
        }
        _ => panic!("Expected RateLimitExceeded error"),
    }
}

#[tokio::test]
async fn test_clipboard_set_rate_limit_exceeded() {
    let manager = ClipboardManager::new();
    let session_id = Uuid::new_v4();
    let user_id = "test-user".to_string();
    let client_ip = test_socket_addr();

    // First 3 requests should succeed
    assert!(manager.handle_clipboard_set(session_id, "test1".to_string(), user_id.clone(), client_ip).await.is_ok());
    assert!(manager.handle_clipboard_set(session_id, "test2".to_string(), user_id.clone(), client_ip).await.is_ok());
    assert!(manager.handle_clipboard_set(session_id, "test3".to_string(), user_id.clone(), client_ip).await.is_ok());

    // 4th request should be rate limited
    let result = manager.handle_clipboard_set(session_id, "test4".to_string(), user_id, client_ip).await;
    assert!(result.is_err());
    match result {
        Err(ClipboardError::RateLimitExceeded { retry_after }) => {
            assert_eq!(retry_after, 5); // First violation: 5s backoff
        }
        _ => panic!("Expected RateLimitExceeded error"),
    }
}

#[tokio::test]
async fn test_different_sessions_independent_limits() {
    let manager = ClipboardManager::new();
    let session_a = Uuid::new_v4();
    let session_b = Uuid::new_v4();
    let user_id = "test-user".to_string();
    let client_ip = test_socket_addr();

    // Exhaust session A's rate limit
    manager.handle_clipboard_set(session_a, "test1".to_string(), user_id.clone(), client_ip).await.ok();
    manager.handle_clipboard_set(session_a, "test2".to_string(), user_id.clone(), client_ip).await.ok();
    manager.handle_clipboard_set(session_a, "test3".to_string(), user_id.clone(), client_ip).await.ok();

    // Session A should be rate limited
    let result_a = manager.handle_clipboard_set(session_a, "test4".to_string(), user_id.clone(), client_ip).await;
    assert!(result_a.is_err());

    // Session B should still have requests available
    let result_b = manager.handle_clipboard_set(session_b, "test1".to_string(), user_id, client_ip).await;
    assert!(result_b.is_ok());
}

#[tokio::test]
async fn test_authorization_cache_grant_and_revoke() {
    let manager = ClipboardManager::new();
    let session_id = Uuid::new_v4();
    let user_id = "test-user".to_string();
    let client_ip = test_socket_addr();

    // Initially no authorization
    let result = manager.handle_clipboard_get(session_id, user_id.clone(), client_ip).await;
    assert!(result.is_err());

    // Grant authorization
    manager.grant_authorization(session_id).await;

    // Should succeed now
    let result = manager.handle_clipboard_get(session_id, user_id.clone(), client_ip).await;
    assert!(result.is_ok());

    // Revoke authorization
    manager.revoke_authorization(session_id).await;

    // Should fail again
    let result = manager.handle_clipboard_get(session_id, user_id, client_ip).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_osc52_clipboard_success() {
    let manager = ClipboardManager::new();
    let session_id = Uuid::new_v4();
    let user_id = "test-user".to_string();
    let client_ip = test_socket_addr();
    let content = "osc52 clipboard content".to_string();
    let selection = "clipboard".to_string();

    // Should succeed (no rate limit, no authorization for OSC 52)
    let result = manager
        .handle_osc52_clipboard(session_id, content, selection, user_id, client_ip)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_osc52_clipboard_size_limit() {
    let manager = ClipboardManager::new();
    let session_id = Uuid::new_v4();
    let user_id = "test-user".to_string();
    let client_ip = test_socket_addr();
    let selection = "clipboard".to_string();

    // Create content > 1MB
    let large_content = "x".repeat(1_048_577);

    let result = manager
        .handle_osc52_clipboard(session_id, large_content, selection, user_id, client_ip)
        .await;

    assert!(result.is_err());
    match result {
        Err(ClipboardError::SizeLimitExceeded) => {}
        _ => panic!("Expected SizeLimitExceeded error"),
    }
}

#[tokio::test]
async fn test_concurrent_clipboard_requests_same_session() {
    use tokio::task::JoinSet;

    let manager = Arc::new(ClipboardManager::new());
    let session_id = Uuid::new_v4();
    let user_id = "test-user".to_string();
    let client_ip = test_socket_addr();

    // Spawn 10 concurrent requests
    let mut tasks = JoinSet::new();
    for i in 0..10 {
        let mgr = manager.clone();
        let uid = user_id.clone();
        let content = format!("test{}", i);
        tasks.spawn(async move {
            mgr.handle_clipboard_set(session_id, content, uid, client_ip).await
        });
    }

    // Collect results
    let mut success_count = 0;
    let mut rate_limited_count = 0;

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => success_count += 1,
            Ok(Err(ClipboardError::RateLimitExceeded { .. })) => rate_limited_count += 1,
            Ok(Err(e)) => panic!("Unexpected error: {}", e),
            Err(e) => panic!("Task join error: {}", e),
        }
    }

    // Should have exactly 3 successes (rate limit) and 7 rate limited
    assert_eq!(success_count, 3, "Expected 3 successful requests");
    assert_eq!(rate_limited_count, 7, "Expected 7 rate limited requests");
}
