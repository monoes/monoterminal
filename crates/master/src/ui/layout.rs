//! UI Layout management
//!
//! Implements the layout per SRS §4.2.1:
//! ┌────────────────────────────────────┐
//! │  Menu Bar (File, Session, Help)   │
//! ├────────────────────────────────────┤
//! │  Session List │ Terminal Canvas   │
//! │  (sidebar)    │ (wgpu rendered)   │
//! │               │                    │
//! ├────────────────────────────────────┤
//! │  Status Bar (FPS, latency)         │
//! └────────────────────────────────────┘

use super::performance::PerformanceMonitor;

/// UI layout manager
pub struct Layout {
    /// Sidebar width in pixels
    sidebar_width: f32,

    /// Menu bar height in pixels
    menu_height: f32,

    /// Status bar height in pixels
    status_height: f32,
}

impl Layout {
    pub fn new() -> Self {
        Self {
            sidebar_width: 200.0,
            menu_height: 30.0,
            status_height: 25.0,
        }
    }

    /// Calculate terminal canvas area
    pub fn terminal_area(&self, window_width: f32, window_height: f32) -> Rect {
        Rect {
            x: self.sidebar_width,
            y: self.menu_height,
            width: window_width - self.sidebar_width,
            height: window_height - self.menu_height - self.status_height,
        }
    }

    /// Calculate sidebar area
    pub fn sidebar_area(&self, window_height: f32) -> Rect {
        Rect {
            x: 0.0,
            y: self.menu_height,
            width: self.sidebar_width,
            height: window_height - self.menu_height - self.status_height,
        }
    }

    /// Calculate menu bar area
    pub fn menu_area(&self, window_width: f32) -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: window_width,
            height: self.menu_height,
        }
    }

    /// Calculate status bar area
    pub fn status_area(&self, window_width: f32, window_height: f32) -> Rect {
        Rect {
            x: 0.0,
            y: window_height - self.status_height,
            width: window_width,
            height: self.status_height,
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

/// Rectangle area
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Session list for sidebar
pub struct SessionList {
    sessions: Vec<SessionItem>,
    selected: Option<usize>,
}

impl SessionList {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected: None,
        }
    }

    /// Add session to list
    pub fn add_session(&mut self, name: String) {
        self.sessions.push(SessionItem { name });
    }

    /// Get sessions
    pub fn sessions(&self) -> &[SessionItem] {
        &self.sessions
    }

    /// Get selected session index
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Set selected session
    pub fn set_selected(&mut self, index: Option<usize>) {
        self.selected = index;
    }
}

impl Default for SessionList {
    fn default() -> Self {
        Self::new()
    }
}

/// Session item in the list
#[derive(Debug, Clone)]
pub struct SessionItem {
    pub name: String,
}

/// Status bar data
pub struct StatusBar {
    fps: f32,
    latency: f32,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            fps: 0.0,
            latency: 0.0,
        }
    }

    /// Update from performance monitor
    pub fn update_from_perf(&mut self, perf: &PerformanceMonitor) {
        self.fps = perf.fps();
        self.latency = perf.avg_frame_time();
    }

    /// Get formatted FPS string
    pub fn fps_string(&self) -> String {
        format!("FPS: {:.1}", self.fps)
    }

    /// Get formatted latency string
    pub fn latency_string(&self) -> String {
        format!("Frame: {:.2}ms", self.latency)
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_new() {
        let layout = Layout::new();
        assert_eq!(layout.sidebar_width, 200.0);
        assert_eq!(layout.menu_height, 30.0);
        assert_eq!(layout.status_height, 25.0);
    }

    #[test]
    fn test_layout_default() {
        let layout = Layout::default();
        assert_eq!(layout.sidebar_width, 200.0);
    }

    #[test]
    fn test_terminal_area() {
        let layout = Layout::new();
        let area = layout.terminal_area(1920.0, 1080.0);

        assert_eq!(area.x, 200.0); // After sidebar
        assert_eq!(area.y, 30.0); // After menu
        assert_eq!(area.width, 1720.0); // 1920 - 200
        assert_eq!(area.height, 1025.0); // 1080 - 30 - 25
    }

    #[test]
    fn test_terminal_area_small_window() {
        let layout = Layout::new();
        let area = layout.terminal_area(800.0, 600.0);

        assert_eq!(area.x, 200.0);
        assert_eq!(area.y, 30.0);
        assert_eq!(area.width, 600.0);
        assert_eq!(area.height, 545.0);
    }

    #[test]
    fn test_sidebar_area() {
        let layout = Layout::new();
        let area = layout.sidebar_area(1080.0);

        assert_eq!(area.x, 0.0);
        assert_eq!(area.y, 30.0);
        assert_eq!(area.width, 200.0);
        assert_eq!(area.height, 1025.0);
    }

    #[test]
    fn test_menu_area() {
        let layout = Layout::new();
        let area = layout.menu_area(1920.0);

        assert_eq!(area.x, 0.0);
        assert_eq!(area.y, 0.0);
        assert_eq!(area.width, 1920.0);
        assert_eq!(area.height, 30.0);
    }

    #[test]
    fn test_status_area() {
        let layout = Layout::new();
        let area = layout.status_area(1920.0, 1080.0);

        assert_eq!(area.x, 0.0);
        assert_eq!(area.y, 1055.0); // 1080 - 25
        assert_eq!(area.width, 1920.0);
        assert_eq!(area.height, 25.0);
    }

    #[test]
    fn test_session_list_new() {
        let list = SessionList::new();
        assert_eq!(list.sessions().len(), 0);
        assert_eq!(list.selected(), None);
    }

    #[test]
    fn test_session_list_add() {
        let mut list = SessionList::new();
        list.add_session("Session 1".to_string());
        list.add_session("Session 2".to_string());

        assert_eq!(list.sessions().len(), 2);
        assert_eq!(list.sessions()[0].name, "Session 1");
        assert_eq!(list.sessions()[1].name, "Session 2");
    }

    #[test]
    fn test_session_list_select() {
        let mut list = SessionList::new();
        list.add_session("Session 1".to_string());
        list.add_session("Session 2".to_string());

        list.set_selected(Some(0));
        assert_eq!(list.selected(), Some(0));

        list.set_selected(Some(1));
        assert_eq!(list.selected(), Some(1));

        list.set_selected(None);
        assert_eq!(list.selected(), None);
    }

    #[test]
    fn test_status_bar_new() {
        let status = StatusBar::new();
        assert_eq!(status.fps, 0.0);
        assert_eq!(status.latency, 0.0);
    }

    #[test]
    fn test_status_bar_fps_string() {
        let mut status = StatusBar::new();
        status.fps = 60.5;
        assert_eq!(status.fps_string(), "FPS: 60.5");
    }

    #[test]
    fn test_status_bar_latency_string() {
        let mut status = StatusBar::new();
        status.latency = 16.67;
        assert_eq!(status.latency_string(), "Frame: 16.67ms");
    }

    #[test]
    fn test_rect_clone() {
        let rect = Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 200.0,
        };

        let rect2 = rect;
        assert_eq!(rect.x, rect2.x);
        assert_eq!(rect.y, rect2.y);
        assert_eq!(rect.width, rect2.width);
        assert_eq!(rect.height, rect2.height);
    }
}
