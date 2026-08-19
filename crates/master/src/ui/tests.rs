//! UI Integration Tests
//!
//! Day 1: Mock integration tests for VT parser → terminal grid → renderer pipeline

#[cfg(test)]
mod tests {
    use super::super::*;
    use bytes::Bytes;
    use glyph_cache::{GlyphCache, GlyphKey};
    use terminal_grid::TerminalGrid;
    use tokio::sync::mpsc;
    use vt_parser::{GridChange, VtParser};

    #[test]
    fn test_vt_parser_to_grid_pipeline() {
        // Test VT parser → terminal grid pipeline
        let mut parser = VtParser::new();
        let mut grid = TerminalGrid::new(24, 80);

        // Feed test data
        parser.feed(b"Hello, World!\n");

        // Apply changes to grid
        let changes = parser.drain_changes();
        grid.apply_changes(changes);

        // Verify grid state
        assert_eq!(grid.get_cell(0, 0).unwrap().ch, 'H');
        assert_eq!(grid.get_cell(0, 6).unwrap().ch, ' ');
        assert_eq!(grid.get_cell(0, 7).unwrap().ch, 'W');

        // Cursor should be at row 1 after newline
        assert_eq!(grid.cursor_position(), (1, 0));
    }

    #[test]
    fn test_glyph_cache_pipeline() {
        let mut cache = GlyphCache::new();

        // Insert glyphs
        let key_a = GlyphKey {
            ch: 'A',
            bold: false,
            italic: false,
        };
        let key_b = GlyphKey {
            ch: 'B',
            bold: true,
            italic: false,
        };

        cache.insert(key_a, 16, 16);
        cache.insert(key_b, 16, 16);

        // Lookup should succeed
        assert!(cache.lookup(key_a).is_some());
        assert!(cache.lookup(key_b).is_some());

        // Different styles should be different keys
        let key_a_bold = GlyphKey {
            ch: 'A',
            bold: true,
            italic: false,
        };
        assert!(cache.lookup(key_a_bold).is_none());
    }

    #[test]
    fn test_dirty_tracking_performance() {
        let mut grid = TerminalGrid::new(24, 80);

        // Clear initial dirty state
        grid.clear_dirty();

        // Print single character
        let start = std::time::Instant::now();
        grid.apply_changes(vec![GridChange::PrintChar('X')]);
        let elapsed = start.elapsed();

        // Should be < 0.5ms (dirty tracking budget)
        assert!(
            elapsed.as_micros() < 500,
            "Dirty tracking took {:?}",
            elapsed
        );

        // Only (0, 0) should be dirty
        assert!(grid.dirty_region().is_dirty(0, 0));
        assert!(!grid.dirty_region().is_dirty(0, 1));
    }

    #[tokio::test]
    async fn test_mock_pty_channel() {
        // Test mock PTY channel (Day 1 pattern)
        let (tx, mut rx) = mpsc::channel::<Bytes>(100);

        // Send mock PTY data
        tx.send(Bytes::from("test\n")).await.unwrap();

        // Receive and parse
        let data = rx.try_recv().unwrap();
        let mut parser = VtParser::new();
        parser.feed(&data);

        let changes = parser.drain_changes();
        assert_eq!(changes.len(), 5); // t, e, s, t, \n
    }

    #[test]
    fn test_60fps_frame_budget() {
        // Validate frame budget targets per SRS §2.1.1
        const FRAME_BUDGET_MS: f32 = 16.67;
        const PTY_PARSE_BUDGET_MS: f32 = 2.0;
        const DIRTY_TRACK_BUDGET_MS: f32 = 0.5;
        const GLYPH_LOOKUP_BUDGET_MS: f32 = 1.0;
        const GPU_RENDER_BUDGET_MS: f32 = 8.0;
        const VSYNC_BUDGET_MS: f32 = 5.0;

        // Total should be ≤ frame budget
        let total = PTY_PARSE_BUDGET_MS
            + DIRTY_TRACK_BUDGET_MS
            + GLYPH_LOOKUP_BUDGET_MS
            + GPU_RENDER_BUDGET_MS
            + VSYNC_BUDGET_MS;

        assert!(
            total <= FRAME_BUDGET_MS,
            "Budget total {:.2}ms exceeds {:.2}ms frame budget",
            total,
            FRAME_BUDGET_MS
        );
    }
}
