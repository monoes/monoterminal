//! Unit tests for SessionManager layout integration (Phase 4: Splits/Tabs)
//!
//! Tests for task-73 Week 2 Day 2 (75% milestone)

use super::*;
use crate::layout::SplitDirection;

#[cfg(test)]
mod session_manager_layout_tests {
    use super::*;

    /// Test: Split pane creates new session and updates layout tree
    #[tokio::test]
    async fn test_handle_split_pane_creates_new_session() {
        let manager = SessionManager::new(None);

        // Create initial session (will become pane-0, and its own workspace root)
        let root = manager.create_session(None, 24, 80).await.unwrap();

        // Wait for layout initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Split pane-0 horizontally
        let result = manager
            .handle_split_pane(root, "pane-0", SplitDirection::Horizontal, None, None)
            .await
            .unwrap();

        // Assert: Layout update returned
        assert!(result.layout_update.root.is_some(), "Layout root should exist");

        // Assert: Focused pane ID exists
        assert!(
            !result.layout_update.focused_pane_id.is_empty(),
            "Focused pane ID should be set"
        );

        // Assert: 2 sessions exist now
        let session_count = manager.session_count().await;
        assert_eq!(
            session_count, 2,
            "Should have 2 sessions after split (original + new)"
        );
    }

    /// Test: Close pane kills session and collapses tree
    #[tokio::test]
    async fn test_handle_close_pane_kills_session() {
        let manager = SessionManager::new(None);

        // Create initial session
        let root = manager.create_session(None, 24, 80).await.unwrap();

        // Wait for layout initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Split pane to create 2 panes
        manager
            .handle_split_pane(root, "pane-0", SplitDirection::Horizontal, None, None)
            .await
            .unwrap();

        assert_eq!(
            manager.session_count().await,
            2,
            "Should have 2 sessions after split"
        );

        // Close pane-1 (the newly created pane)
        let layout = manager.handle_close_pane(root, "pane-1", None).await.unwrap();

        // Assert: Layout update returned
        assert!(layout.root.is_some(), "Layout root should exist");

        // Assert: Only 1 session remains
        let session_count = manager.session_count().await;
        assert_eq!(
            session_count, 1,
            "Should have 1 session after closing one pane"
        );

        // Assert: Tree should have collapsed back to single pane
        // (LayoutManager tests verify tree structure collapse)
    }

    /// Test: Focus pane updates focused_pane_id state
    #[tokio::test]
    async fn test_handle_focus_pane_updates_state() {
        let manager = SessionManager::new(None);

        // Create initial session
        let root = manager.create_session(None, 24, 80).await.unwrap();

        // Wait for layout initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Split pane to create 2 panes
        manager
            .handle_split_pane(root, "pane-0", SplitDirection::Horizontal, None, None)
            .await
            .unwrap();

        // Focus pane-1
        let layout = manager.handle_focus_pane(root, "pane-1").await.unwrap();

        // Assert: Focused pane ID is updated
        assert_eq!(
            layout.focused_pane_id, "pane-1",
            "Focused pane should be pane-1"
        );

        // Focus pane-0
        let layout = manager.handle_focus_pane(root, "pane-0").await.unwrap();

        // Assert: Focused pane ID changed
        assert_eq!(
            layout.focused_pane_id, "pane-0",
            "Focused pane should be pane-0"
        );
    }

    /// Test: send_input_to_pane routes to specific pane (Phase 4)
    #[tokio::test]
    async fn test_send_input_to_pane_with_pane_id() {
        let manager = SessionManager::new(None);

        // Create initial session
        let root = manager.create_session(None, 24, 80).await.unwrap();

        // Wait for layout initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Split pane to create 2 panes
        manager
            .handle_split_pane(root, "pane-0", SplitDirection::Horizontal, None, None)
            .await
            .unwrap();

        // Send input to pane-1 (specific pane)
        let result = manager
            .send_input_to_pane(root, Some("pane-1"), b"echo test", None)
            .await;

        // Assert: Input sent successfully
        assert!(
            result.is_ok(),
            "Should send input to specific pane without error"
        );
    }

    /// Test: send_input_to_pane with None routes to focused pane (backward compat)
    #[tokio::test]
    async fn test_send_input_backward_compat() {
        let manager = SessionManager::new(None);

        // Create initial session
        let root = manager.create_session(None, 24, 80).await.unwrap();

        // Wait for layout initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Split pane to create 2 panes
        manager
            .handle_split_pane(root, "pane-0", SplitDirection::Horizontal, None, None)
            .await
            .unwrap();

        // Focus pane-1
        manager.handle_focus_pane(root, "pane-1").await.unwrap();

        // Send input with None (should route to focused pane-1)
        let result = manager
            .send_input_to_pane(root, None, b"echo focused", None)
            .await;

        // Assert: Input sent successfully
        assert!(
            result.is_ok(),
            "Should send input to focused pane (backward compat)"
        );
    }

    /// Test: Cannot close last pane (error)
    #[tokio::test]
    async fn test_cannot_close_last_pane() {
        let manager = SessionManager::new(None);

        // Create initial session (single pane)
        let root = manager.create_session(None, 24, 80).await.unwrap();

        // Wait for layout initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Try to close the only pane
        let result = manager.handle_close_pane(root, "pane-0", None).await;

        // Assert: Error returned
        assert!(
            result.is_err(),
            "Should return error when trying to close last pane"
        );

        // Assert: Error message contains "CANNOT_CLOSE_LAST_PANE" or "last remaining pane"
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("last") || error_msg.contains("CANNOT_CLOSE"),
            "Error should mention cannot close last pane: {}",
            error_msg
        );
    }

    /// Test: Max 16 panes enforced (error)
    #[tokio::test]
    async fn test_max_panes_reached() {
        let manager = SessionManager::new(None);

        // Create initial session
        let root = manager.create_session(None, 24, 80).await.unwrap();

        // Wait for layout initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Split 15 times to create 16 panes (pane-0 through pane-15)
        for i in 0..15 {
            let pane_id = format!("pane-{}", i);
            let result = manager
                .handle_split_pane(root, &pane_id, SplitDirection::Horizontal, None, None)
                .await;

            // First 14 splits should succeed (up to 15 panes)
            // 15th split creates 16th pane (should succeed)
            assert!(
                result.is_ok(),
                "Split {} should succeed (creating pane {})",
                i + 1,
                i + 1
            );
        }

        // Assert: 16 sessions exist
        assert_eq!(
            manager.session_count().await,
            16,
            "Should have 16 sessions after 15 splits"
        );

        // Try 17th pane (should fail)
        let result = manager
            .handle_split_pane(root, "pane-0", SplitDirection::Horizontal, None, None)
            .await;

        // Assert: Error returned
        assert!(
            result.is_err(),
            "Should return error when trying to exceed 16 panes"
        );

        // Assert: Error message contains "MAX_PANES" or "16"
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("MAX_PANES") || error_msg.contains("16"),
            "Error should mention max panes limit: {}",
            error_msg
        );
    }

    /// Test: Invalid pane ID returns error
    #[tokio::test]
    async fn test_invalid_pane_id() {
        let manager = SessionManager::new(None);

        // Create initial session
        let root = manager.create_session(None, 24, 80).await.unwrap();

        // Wait for layout initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Try to split non-existent pane
        let result = manager
            .handle_split_pane(root, "pane-999", SplitDirection::Horizontal, None, None)
            .await;

        // Assert: Error returned
        assert!(result.is_err(), "Should return error for invalid pane ID");

        // Assert: Error message contains "INVALID_PANE_ID" or "not found"
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("INVALID") || error_msg.contains("not found"),
            "Error should mention invalid pane ID: {}",
            error_msg
        );

        // Try to focus non-existent pane
        let result = manager.handle_focus_pane(root, "pane-999").await;

        // Assert: Error returned
        assert!(
            result.is_err(),
            "Should return error for invalid pane ID on focus"
        );

        // Try to close non-existent pane
        let result = manager.handle_close_pane(root, "pane-999", None).await;

        // Assert: Error returned
        assert!(
            result.is_err(),
            "Should return error for invalid pane ID on close"
        );
    }

    // ============================================================================
    // Integration Tests (75% → 100% milestone)
    // ============================================================================

    /// Integration Test: End-to-end split → input → close flow
    #[tokio::test]
    async fn test_e2e_split_input_close_flow() {
        let manager = SessionManager::new(None);

        // Step 1: Create initial session
        let root = manager.create_session(None, 24, 80).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        assert_eq!(
            manager.session_count().await,
            1,
            "Should have 1 session initially"
        );

        // Step 2: Split pane horizontally (creates pane-1)
        let result = manager
            .handle_split_pane(root, "pane-0", SplitDirection::Horizontal, None, None)
            .await
            .unwrap();

        assert!(
            result.layout_update.root.is_some(),
            "Layout should exist after split"
        );
        assert_eq!(
            manager.session_count().await,
            2,
            "Should have 2 sessions after split"
        );

        // Step 3: Send input to pane-0 (specific pane routing)
        let result = manager
            .send_input_to_pane(root, Some("pane-0"), b"echo pane-0", None)
            .await;
        assert!(result.is_ok(), "Should send input to pane-0");

        // Step 4: Send input to pane-1
        let result = manager
            .send_input_to_pane(root, Some("pane-1"), b"echo pane-1", None)
            .await;
        assert!(result.is_ok(), "Should send input to pane-1");

        // Step 5: Focus pane-1
        let layout = manager.handle_focus_pane(root, "pane-1").await.unwrap();
        assert_eq!(
            layout.focused_pane_id, "pane-1",
            "Should focus pane-1"
        );

        // Step 6: Send input to focused pane (None = focused)
        let result = manager
            .send_input_to_pane(root, None, b"echo focused", None)
            .await;
        assert!(result.is_ok(), "Should send input to focused pane");

        // Step 7: Close pane-1
        let layout = manager.handle_close_pane(root, "pane-1", None).await.unwrap();
        assert!(layout.root.is_some(), "Layout should exist after close");
        assert_eq!(
            manager.session_count().await,
            1,
            "Should have 1 session after closing pane-1"
        );

        // Step 8: Verify focused pane auto-switched to pane-0
        assert_eq!(
            layout.focused_pane_id, "pane-0",
            "Should auto-focus pane-0 after closing pane-1"
        );

        // Step 9: Verify input still works after close
        let result = manager
            .send_input_to_pane(root, Some("pane-0"), b"echo still works", None)
            .await;
        assert!(result.is_ok(), "Should send input after pane close");
    }

    /// Integration Test: Concurrent split operations (thread safety)
    #[tokio::test]
    async fn test_concurrent_split_operations() {
        let manager = std::sync::Arc::new(SessionManager::new(None));

        // Create initial session
        let root = manager.create_session(None, 24, 80).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Spawn 3 concurrent split operations
        let mut handles = vec![];

        for i in 0..3 {
            let mgr = manager.clone();
            let handle = tokio::spawn(async move {
                // Each task tries to split pane-0
                tokio::time::sleep(tokio::time::Duration::from_millis(i * 10)).await;
                mgr.handle_split_pane(root, "pane-0", SplitDirection::Horizontal, None, None)
                    .await
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await);
        }

        // Count successes (first one should succeed, others may fail due to tree changes)
        let successes = results
            .iter()
            .filter(|r| r.as_ref().unwrap().is_ok())
            .count();

        // At least one should succeed
        assert!(
            successes >= 1,
            "At least one concurrent split should succeed"
        );

        // Verify final state consistency
        let session_count = manager.session_count().await;
        assert!(
            session_count >= 2 && session_count <= 4,
            "Should have 2-4 sessions after concurrent splits (actual: {})",
            session_count
        );
    }

    /// Integration Test: Full session lifecycle with layout tree collapse
    #[tokio::test]
    async fn test_session_lifecycle_with_layout() {
        let manager = SessionManager::new(None);

        // Step 1: Create initial session → single pane layout
        let root = manager.create_session(None, 24, 80).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        assert_eq!(manager.session_count().await, 1, "Initial: 1 session");

        // Step 2: Split horizontally → 2 panes (pane-0, pane-1)
        manager
            .handle_split_pane(root, "pane-0", SplitDirection::Horizontal, None, None)
            .await
            .unwrap();
        assert_eq!(manager.session_count().await, 2, "After split 1: 2 sessions");

        // Step 3: Split pane-1 vertically → 3 panes (pane-0, pane-1, pane-2)
        manager
            .handle_split_pane(root, "pane-1", SplitDirection::Vertical, None, None)
            .await
            .unwrap();
        assert_eq!(
            manager.session_count().await,
            3,
            "After split 2: 3 sessions"
        );

        // Step 4: Split pane-0 vertically → 4 panes (pane-0, pane-1, pane-2, pane-3)
        manager
            .handle_split_pane(root, "pane-0", SplitDirection::Vertical, None, None)
            .await
            .unwrap();
        assert_eq!(
            manager.session_count().await,
            4,
            "After split 3: 4 sessions"
        );

        // Verify: 4 panes exist
        let layout = manager.handle_focus_pane(root, "pane-3").await.unwrap();
        assert!(layout.root.is_some(), "Layout should exist with 4 panes");

        // Step 5: Close pane-3 → tree should collapse partially
        manager.handle_close_pane(root, "pane-3", None).await.unwrap();
        assert_eq!(
            manager.session_count().await,
            3,
            "After close 1: 3 sessions"
        );

        // Step 6: Close pane-2 → tree should collapse further
        manager.handle_close_pane(root, "pane-2", None).await.unwrap();
        assert_eq!(
            manager.session_count().await,
            2,
            "After close 2: 2 sessions"
        );

        // Step 7: Close pane-1 → should collapse to single pane
        manager.handle_close_pane(root, "pane-1", None).await.unwrap();
        assert_eq!(
            manager.session_count().await,
            1,
            "After close 3: 1 session"
        );

        // Step 8: Verify cannot close last pane
        let result = manager.handle_close_pane(root, "pane-0", None).await;
        assert!(
            result.is_err(),
            "Should not be able to close last pane"
        );
        assert_eq!(
            manager.session_count().await,
            1,
            "Should still have 1 session after failed close"
        );

        // Step 9: Verify layout is still functional
        let result = manager
            .send_input_to_pane(root, Some("pane-0"), b"final test", None)
            .await;
        assert!(
            result.is_ok(),
            "Should be able to send input to final pane"
        );
    }

    /// Regression test: layouts are scoped per workspace root, not global.
    /// Before this fix, `SessionManager` held a single `Option<LayoutManager>`
    /// shared by every session ever created — splitting a pane in one
    /// workspace would silently operate on (or fail against) whichever
    /// workspace happened to create the very first session in the process.
    #[tokio::test]
    async fn test_layouts_are_isolated_per_workspace() {
        let manager = SessionManager::new(None);

        // Two independent workspaces, each created as its own root session
        // (mirrors resolve_named_session's "creating new" branch).
        let workspace_a = manager
            .resolve_named_session("computer/api-service/server", None, 24, 80)
            .await
            .unwrap();
        let workspace_b = manager
            .resolve_named_session("computer/frontend/server", None, 24, 80)
            .await
            .unwrap();
        assert_ne!(workspace_a, workspace_b);

        // Splitting workspace A must not affect workspace B's layout at all.
        manager
            .handle_split_pane(workspace_a, "pane-0", SplitDirection::Horizontal, None, None)
            .await
            .unwrap();

        assert_eq!(
            manager.sibling_pane_sessions(workspace_a).await.len(),
            2,
            "Workspace A should have 2 panes after its own split"
        );
        assert_eq!(
            manager.sibling_pane_sessions(workspace_b).await.len(),
            1,
            "Workspace B must be untouched by workspace A's split"
        );

        // Workspace B can independently split too, using the SAME pane id
        // ("pane-0") that workspace A already used — pane ids are only
        // unique within one workspace's layout, not globally.
        manager
            .handle_split_pane(workspace_b, "pane-0", SplitDirection::Vertical, None, None)
            .await
            .unwrap();

        assert_eq!(manager.sibling_pane_sessions(workspace_a).await.len(), 2);
        assert_eq!(manager.sibling_pane_sessions(workspace_b).await.len(), 2);

        // Both workspaces independently numbered their new pane "pane-1"
        // (each LayoutManager's pane counter starts fresh) — proves lookups
        // are scoped per-workspace rather than sharing one global id space,
        // since two DIFFERENT sessions both legitimately answer to "pane-1"
        // depending on which workspace's layout you ask.
        let pane_1_in_a = manager
            .sibling_pane_sessions(workspace_a)
            .await
            .into_iter()
            .find(|(pane_id, _)| pane_id == "pane-1")
            .map(|(_, session_id)| session_id)
            .unwrap();
        let pane_1_in_b = manager
            .sibling_pane_sessions(workspace_b)
            .await
            .into_iter()
            .find(|(pane_id, _)| pane_id == "pane-1")
            .map(|(_, session_id)| session_id)
            .unwrap();
        assert_ne!(
            pane_1_in_a, pane_1_in_b,
            "each workspace's \"pane-1\" must be its own independent session"
        );

        // A pane id that genuinely doesn't exist in a given workspace must
        // still be rejected.
        let result = manager.handle_focus_pane(workspace_b, "pane-99").await;
        assert!(
            result.is_err(),
            "An unknown pane id must not resolve in any workspace's layout"
        );
    }
}
