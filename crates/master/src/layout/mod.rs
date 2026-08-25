//! Layout Manager for Phase 4 Splits/Tabs (ADR-018, task-69)
//!
//! Master-side layout management with recursive pane tree structure.
//! The server owns layout state (not client) for consistency across multiple clients
//! and to enable collaboration features.
//!
//! ## Architecture
//!
//! - **PaneLayout**: Recursive tree (TerminalPane | SplitPane)
//! - **TerminalPane**: Leaf node with session_id, focused flag, pane_id
//! - **SplitPane**: Branch node with direction, children, ratios
//!
//! ## Tree Operations
//!
//! - `split_pane()`: Find pane, replace with SplitPane[old, new]
//! - `close_pane()`: Remove pane, collapse parent if 1 child remains
//! - `focus_pane()`: Update focused_pane_id
//! - `to_proto()`: Serialize to wire format (LayoutUpdate)
//!
//! ## Limits
//!
//! - Max 16 panes per session (UX + memory limit)
//! - Cannot close last pane (CANNOT_CLOSE_LAST_PANE error)
//! - Sequential pane IDs: "pane-0", "pane-1", "pane-2", ...

use anyhow::{anyhow, bail, Result};
use monoterminal_protocol::{
    split_pane::Direction as ProtoDirection, LayoutUpdate, PaneLayout as ProtoPaneLayout,
    SplitPane as ProtoSplitPane, TerminalPane as ProtoTerminalPane,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::debug;
use uuid::Uuid;

/// Maximum number of panes per session (SRS §7.4 performance target: 4 panes at 60 FPS)
/// Hard limit: 16 panes (practical UX limit)
const MAX_PANES: usize = 16;

/// Split direction (matches protobuf enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal, // Left | Right
    Vertical,   // Top | Bottom
}

impl From<ProtoDirection> for SplitDirection {
    fn from(proto: ProtoDirection) -> Self {
        match proto {
            ProtoDirection::Horizontal => SplitDirection::Horizontal,
            ProtoDirection::Vertical => SplitDirection::Vertical,
        }
    }
}

impl From<SplitDirection> for ProtoDirection {
    fn from(dir: SplitDirection) -> Self {
        match dir {
            SplitDirection::Horizontal => ProtoDirection::Horizontal,
            SplitDirection::Vertical => ProtoDirection::Vertical,
        }
    }
}

/// Pane layout tree node (recursive structure)
#[derive(Debug, Clone, PartialEq)]
pub enum PaneLayout {
    Terminal {
        session_id: Uuid,
        pane_id: String,
    },
    Split {
        direction: SplitDirection,
        children: Vec<PaneLayout>,
        ratios: Vec<f32>, // [0.5, 0.5] = 50/50 split
    },
}

impl PaneLayout {
    /// Create a new terminal pane
    pub fn terminal(session_id: Uuid, pane_id: String) -> Self {
        PaneLayout::Terminal {
            session_id,
            pane_id,
        }
    }

    /// Create a new split pane
    pub fn split(direction: SplitDirection, children: Vec<PaneLayout>, ratios: Vec<f32>) -> Self {
        PaneLayout::Split {
            direction,
            children,
            ratios,
        }
    }

    /// Count total number of terminal panes in tree
    pub fn count_panes(&self) -> usize {
        match self {
            PaneLayout::Terminal { .. } => 1,
            PaneLayout::Split { children, .. } => children.iter().map(|c| c.count_panes()).sum(),
        }
    }

    /// Find pane by ID (returns mutable reference)
    fn find_pane_mut(&mut self, target_pane_id: &str) -> Option<&mut PaneLayout> {
        match self {
            PaneLayout::Terminal { pane_id, .. } => {
                if pane_id == target_pane_id {
                    Some(self)
                } else {
                    None
                }
            }
            PaneLayout::Split { children, .. } => {
                for child in children {
                    if let Some(found) = child.find_pane_mut(target_pane_id) {
                        return Some(found);
                    }
                }
                None
            }
        }
    }

    /// Collect all terminal pane IDs in tree
    pub fn collect_pane_ids(&self) -> Vec<String> {
        match self {
            PaneLayout::Terminal { pane_id, .. } => vec![pane_id.clone()],
            PaneLayout::Split { children, .. } => {
                children.iter().flat_map(|c| c.collect_pane_ids()).collect()
            }
        }
    }
}

/// Layout manager for splits/tabs functionality
///
/// Owns the pane layout tree and manages all layout mutations.
pub struct LayoutManager {
    /// Root of the pane layout tree
    root: PaneLayout,

    /// Currently focused pane ID
    focused_pane_id: String,

    /// Map pane ID → session ID for quick lookup
    pane_to_session: HashMap<String, Uuid>,

    /// Sequential pane ID counter (pane-0, pane-1, ...)
    next_pane_id: AtomicU32,
}

impl LayoutManager {
    /// Create a new layout manager with single initial pane
    ///
    /// # Arguments
    /// * `initial_session_id` - Session ID for the first pane
    ///
    /// # Returns
    /// LayoutManager with single pane ("pane-0")
    pub fn new(initial_session_id: Uuid) -> Self {
        let pane_id = "pane-0".to_string();
        let root = PaneLayout::terminal(initial_session_id, pane_id.clone());

        let mut pane_to_session = HashMap::new();
        pane_to_session.insert(pane_id.clone(), initial_session_id);

        Self {
            root,
            focused_pane_id: pane_id,
            pane_to_session,
            next_pane_id: AtomicU32::new(1), // Next ID will be "pane-1"
        }
    }

    /// Generate next sequential pane ID
    fn next_pane_id(&self) -> String {
        let id = self.next_pane_id.fetch_add(1, Ordering::SeqCst);
        format!("pane-{}", id)
    }

    /// Split an existing pane into two panes
    ///
    /// # Arguments
    /// * `pane_id` - ID of pane to split
    /// * `direction` - Split direction (horizontal/vertical)
    /// * `new_session_id` - Session ID for the new pane
    ///
    /// # Returns
    /// New pane ID on success
    ///
    /// # Errors
    /// - `INVALID_PANE_ID` if pane not found
    /// - `MAX_PANES_REACHED` if already at 16 panes
    pub fn split_pane(
        &mut self,
        pane_id: &str,
        direction: SplitDirection,
        new_session_id: Uuid,
    ) -> Result<String> {
        // Check max panes limit
        if self.root.count_panes() >= MAX_PANES {
            bail!("MAX_PANES_REACHED: Cannot exceed {} panes per session", MAX_PANES);
        }

        // Generate new pane ID
        let new_pane_id = self.next_pane_id();

        // Find the pane to split
        let target_pane = self
            .root
            .find_pane_mut(pane_id)
            .ok_or_else(|| anyhow!("INVALID_PANE_ID: Pane '{}' not found", pane_id))?;

        // Clone the existing pane (will become first child of split)
        let old_pane = target_pane.clone();

        // Create new terminal pane (will become second child of split)
        let new_pane = PaneLayout::terminal(new_session_id, new_pane_id.clone());

        // Replace target pane with split containing [old, new]
        *target_pane = PaneLayout::split(
            direction,
            vec![old_pane, new_pane],
            vec![0.5, 0.5], // 50/50 split
        );

        // Update pane → session mapping
        self.pane_to_session.insert(new_pane_id.clone(), new_session_id);

        debug!(
            "Split pane '{}' ({:?}) → created '{}'",
            pane_id, direction, new_pane_id
        );

        Ok(new_pane_id)
    }

    /// Close a pane (and kill its PTY session)
    ///
    /// If the parent split has only 1 child after close, collapse it.
    ///
    /// # Arguments
    /// * `pane_id` - ID of pane to close
    ///
    /// # Errors
    /// - `INVALID_PANE_ID` if pane not found
    /// - `CANNOT_CLOSE_LAST_PANE` if this is the only pane
    pub fn close_pane(&mut self, pane_id: &str) -> Result<()> {
        // Check if this is the last pane
        if self.root.count_panes() == 1 {
            bail!("CANNOT_CLOSE_LAST_PANE: Cannot close the only remaining pane");
        }

        // Remove from pane → session mapping
        self.pane_to_session
            .remove(pane_id)
            .ok_or_else(|| anyhow!("INVALID_PANE_ID: Pane '{}' not found", pane_id))?;

        // Remove pane from tree and collapse if needed
        self.remove_pane_from_tree(pane_id)?;

        // If closed pane was focused, focus the first available pane
        if self.focused_pane_id == pane_id {
            let pane_ids = self.root.collect_pane_ids();
            self.focused_pane_id = pane_ids
                .first()
                .ok_or_else(|| anyhow!("No panes remaining after close"))?
                .clone();
            debug!("Focused pane '{}' after close", self.focused_pane_id);
        }

        debug!("Closed pane '{}'", pane_id);
        Ok(())
    }

    /// Remove pane from tree and collapse parent if needed
    fn remove_pane_from_tree(&mut self, pane_id: &str) -> Result<()> {
        // Special case: if root is a terminal pane, cannot remove it
        if let PaneLayout::Terminal { pane_id: root_pane_id, .. } = &self.root {
            if root_pane_id == pane_id {
                bail!("Cannot remove root terminal pane");
            }
        }

        // Recursive removal: find parent split, remove child, collapse if needed
        remove_from_split(&mut self.root, pane_id)?;

        Ok(())
    }

    /// Focus a pane (set it as the active input target)
    ///
    /// # Arguments
    /// * `pane_id` - ID of pane to focus
    ///
    /// # Errors
    /// - `INVALID_PANE_ID` if pane not found
    pub fn focus_pane(&mut self, pane_id: &str) -> Result<()> {
        // Verify pane exists
        if !self.pane_to_session.contains_key(pane_id) {
            bail!("INVALID_PANE_ID: Pane '{}' not found", pane_id);
        }

        self.focused_pane_id = pane_id.to_string();
        debug!("Focused pane '{}'", pane_id);
        Ok(())
    }

    /// Get session ID for a pane
    pub fn get_session_id(&self, pane_id: &str) -> Option<Uuid> {
        self.pane_to_session.get(pane_id).copied()
    }

    /// Get currently focused pane ID
    pub fn get_focused_pane_id(&self) -> &str {
        &self.focused_pane_id
    }

    /// All pane IDs currently in the layout tree (including the root pane).
    pub fn collect_pane_ids(&self) -> Vec<String> {
        self.root.collect_pane_ids()
    }

    /// Serialize layout to protobuf LayoutUpdate message
    pub fn to_proto(&self) -> LayoutUpdate {
        LayoutUpdate {
            root: Some(self.pane_layout_to_proto(&self.root, &self.focused_pane_id)),
            focused_pane_id: self.focused_pane_id.clone(),
        }
    }

    /// Convert PaneLayout to protobuf PaneLayout (recursive)
    fn pane_layout_to_proto(&self, layout: &PaneLayout, focused_pane_id: &str) -> ProtoPaneLayout {
        match layout {
            PaneLayout::Terminal { session_id, pane_id } => ProtoPaneLayout {
                pane: Some(monoterminal_protocol::pane_layout::Pane::Terminal(
                    ProtoTerminalPane {
                        session_id: session_id.to_string(),
                        focused: pane_id == focused_pane_id,
                        pane_id: pane_id.clone(),
                    },
                )),
            },
            PaneLayout::Split {
                direction,
                children,
                ratios,
            } => ProtoPaneLayout {
                pane: Some(monoterminal_protocol::pane_layout::Pane::Split(
                    ProtoSplitPane {
                        direction: i32::from(ProtoDirection::from(*direction)),
                        children: children
                            .iter()
                            .map(|c| self.pane_layout_to_proto(c, focused_pane_id))
                            .collect(),
                        ratios: ratios.clone(),
                    },
                )),
            },
        }
    }
}

/// Recursive helper: remove pane from split and collapse if needed (free function to avoid borrow conflicts).
///
/// Returns whether `target_pane_id` was found and removed somewhere in this
/// subtree. Splicing a child out of a `children`/`ratios` list must happen
/// exactly once, at the level whose *direct* children contain the target —
/// not at every ancestor on the way back up. A version that spliced at every
/// level that saw `Ok(true)` bubble up double-removed one slot per nesting
/// level above the target (e.g. splitting pane-1 into [pane-1, pane-2] and
/// then closing pane-2 also removed the just-collapsed pane-1 slot at the
/// parent split, leaving one pane instead of two).
fn remove_from_split(node: &mut PaneLayout, target_pane_id: &str) -> Result<bool> {
    let PaneLayout::Split {
        children, ratios, ..
    } = node
    else {
        return Ok(false);
    };

    // A direct terminal child matching the target: splice it out here.
    if let Some(index) = children.iter().position(
        |c| matches!(c, PaneLayout::Terminal { pane_id, .. } if pane_id == target_pane_id),
    ) {
        children.remove(index);
        ratios.remove(index);

        if children.len() == 1 {
            *node = children.remove(0);
            debug!("Collapsed split after removing pane '{}'", target_pane_id);
        } else {
            let sum: f32 = ratios.iter().sum();
            for ratio in ratios.iter_mut() {
                *ratio /= sum;
            }
        }
        return Ok(true);
    }

    // Not a direct child: recurse into split children only. Whichever one
    // (if any) already handled the removal, stop — no splicing at this level.
    for child in children.iter_mut() {
        if matches!(child, PaneLayout::Split { .. }) && remove_from_split(child, target_pane_id)? {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session_id(n: u8) -> Uuid {
        Uuid::from_u128(n as u128)
    }

    #[test]
    fn test_new_layout_single_pane() {
        let session_id = test_session_id(1);
        let layout = LayoutManager::new(session_id);

        assert_eq!(layout.root.count_panes(), 1);
        assert_eq!(layout.focused_pane_id, "pane-0");
        assert_eq!(layout.get_session_id("pane-0"), Some(session_id));
    }

    #[test]
    fn test_split_pane_horizontal() {
        let session_id_1 = test_session_id(1);
        let session_id_2 = test_session_id(2);

        let mut layout = LayoutManager::new(session_id_1);
        let new_pane_id = layout
            .split_pane("pane-0", SplitDirection::Horizontal, session_id_2)
            .unwrap();

        assert_eq!(new_pane_id, "pane-1");
        assert_eq!(layout.root.count_panes(), 2);
        assert_eq!(layout.get_session_id("pane-0"), Some(session_id_1));
        assert_eq!(layout.get_session_id("pane-1"), Some(session_id_2));

        // Check split structure
        match &layout.root {
            PaneLayout::Split {
                direction,
                children,
                ratios,
            } => {
                assert_eq!(*direction, SplitDirection::Horizontal);
                assert_eq!(children.len(), 2);
                assert_eq!(ratios, &vec![0.5, 0.5]);
            }
            _ => panic!("Expected Split node"),
        }
    }

    #[test]
    fn test_split_pane_vertical() {
        let session_id_1 = test_session_id(1);
        let session_id_2 = test_session_id(2);

        let mut layout = LayoutManager::new(session_id_1);
        let new_pane_id = layout
            .split_pane("pane-0", SplitDirection::Vertical, session_id_2)
            .unwrap();

        assert_eq!(new_pane_id, "pane-1");

        match &layout.root {
            PaneLayout::Split { direction, .. } => {
                assert_eq!(*direction, SplitDirection::Vertical);
            }
            _ => panic!("Expected Split node"),
        }
    }

    #[test]
    fn test_close_pane() {
        let session_id_1 = test_session_id(1);
        let session_id_2 = test_session_id(2);

        let mut layout = LayoutManager::new(session_id_1);
        layout
            .split_pane("pane-0", SplitDirection::Horizontal, session_id_2)
            .unwrap();

        assert_eq!(layout.root.count_panes(), 2);

        // Close pane-1 → should collapse back to single pane
        layout.close_pane("pane-1").unwrap();

        assert_eq!(layout.root.count_panes(), 1);
        assert_eq!(layout.get_session_id("pane-1"), None);

        // Root should be terminal again (collapsed)
        match &layout.root {
            PaneLayout::Terminal { pane_id, .. } => {
                assert_eq!(pane_id, "pane-0");
            }
            _ => panic!("Expected Terminal node after collapse"),
        }
    }

    #[test]
    fn test_cannot_close_last_pane() {
        let session_id = test_session_id(1);
        let mut layout = LayoutManager::new(session_id);

        let result = layout.close_pane("pane-0");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CANNOT_CLOSE_LAST_PANE"));
    }

    #[test]
    fn test_invalid_pane_id() {
        let session_id = test_session_id(1);
        let mut layout = LayoutManager::new(session_id);

        let result = layout.split_pane("pane-999", SplitDirection::Horizontal, test_session_id(2));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("INVALID_PANE_ID"));
    }

    #[test]
    fn test_max_panes_reached() {
        let mut layout = LayoutManager::new(test_session_id(0));

        // Split 15 times to reach 16 panes
        for i in 0..15 {
            let pane_id = format!("pane-{}", i);
            layout
                .split_pane(&pane_id, SplitDirection::Horizontal, test_session_id(i + 1))
                .unwrap();
        }

        assert_eq!(layout.root.count_panes(), 16);

        // 17th split should fail
        let result = layout.split_pane("pane-0", SplitDirection::Horizontal, test_session_id(100));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("MAX_PANES_REACHED"));
    }

    #[test]
    fn test_focus_pane() {
        let session_id_1 = test_session_id(1);
        let session_id_2 = test_session_id(2);

        let mut layout = LayoutManager::new(session_id_1);
        layout
            .split_pane("pane-0", SplitDirection::Horizontal, session_id_2)
            .unwrap();

        assert_eq!(layout.get_focused_pane_id(), "pane-0");

        layout.focus_pane("pane-1").unwrap();
        assert_eq!(layout.get_focused_pane_id(), "pane-1");
    }

    #[test]
    fn test_focus_pane_invalid_id() {
        let session_id = test_session_id(1);
        let mut layout = LayoutManager::new(session_id);

        let result = layout.focus_pane("pane-999");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("INVALID_PANE_ID"));
    }

    #[test]
    fn test_nested_splits() {
        let mut layout = LayoutManager::new(test_session_id(0));

        // Split pane-0 horizontally → [pane-0, pane-1]
        layout
            .split_pane("pane-0", SplitDirection::Horizontal, test_session_id(1))
            .unwrap();

        // Split pane-1 vertically → [pane-0, [pane-1, pane-2]]
        layout
            .split_pane("pane-1", SplitDirection::Vertical, test_session_id(2))
            .unwrap();

        assert_eq!(layout.root.count_panes(), 3);

        // Close pane-2 → should collapse to [pane-0, pane-1]
        layout.close_pane("pane-2").unwrap();
        assert_eq!(layout.root.count_panes(), 2);
    }

    #[test]
    fn test_to_proto_single_pane() {
        let session_id = test_session_id(1);
        let layout = LayoutManager::new(session_id);

        let proto = layout.to_proto();
        assert_eq!(proto.focused_pane_id, "pane-0");

        let root = proto.root.unwrap();
        match root.pane.unwrap() {
            monoterminal_protocol::pane_layout::Pane::Terminal(term) => {
                assert_eq!(term.pane_id, "pane-0");
                assert_eq!(term.session_id, session_id.to_string());
                assert!(term.focused);
            }
            _ => panic!("Expected Terminal pane"),
        }
    }

    #[test]
    fn test_to_proto_split_layout() {
        let mut layout = LayoutManager::new(test_session_id(0));
        layout
            .split_pane("pane-0", SplitDirection::Horizontal, test_session_id(1))
            .unwrap();
        layout.focus_pane("pane-1").unwrap();

        let proto = layout.to_proto();
        assert_eq!(proto.focused_pane_id, "pane-1");

        let root = proto.root.unwrap();
        match root.pane.unwrap() {
            monoterminal_protocol::pane_layout::Pane::Split(split) => {
                assert_eq!(split.direction, i32::from(ProtoDirection::Horizontal));
                assert_eq!(split.children.len(), 2);
                assert_eq!(split.ratios, vec![0.5, 0.5]);

                // Check that pane-1 is focused
                match &split.children[1].pane {
                    Some(monoterminal_protocol::pane_layout::Pane::Terminal(term)) => {
                        assert_eq!(term.pane_id, "pane-1");
                        assert!(term.focused);
                    }
                    _ => panic!("Expected Terminal pane"),
                }
            }
            _ => panic!("Expected Split pane"),
        }
    }

    #[test]
    fn test_close_pane_updates_focus() {
        let mut layout = LayoutManager::new(test_session_id(0));
        layout
            .split_pane("pane-0", SplitDirection::Horizontal, test_session_id(1))
            .unwrap();
        layout.focus_pane("pane-1").unwrap();

        // Close focused pane → should auto-focus pane-0
        layout.close_pane("pane-1").unwrap();
        assert_eq!(layout.get_focused_pane_id(), "pane-0");
    }
}
