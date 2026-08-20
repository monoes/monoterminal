///! Activity tracking and adaptive frame rate control
///!
///! Implements Week 8 Day 3-4 mobile battery optimization:
///! - 60 FPS active (recent user input)
///! - 30 FPS idle (5+ seconds no input)
///! - Optional: 30/15 FPS for mobile devices
///!
///! Expected battery savings: 40-50% on mobile browsers

use std::time::{Duration, Instant};

/// Activity tracking for adaptive frame rate
pub struct ActivityTracker {
    last_input: Instant,
    idle_threshold: Duration,
    is_mobile: bool,
}

impl ActivityTracker {
    /// Create new activity tracker
    ///
    /// Default: 5-second idle threshold (desktop)
    pub fn new() -> Self {
        Self {
            last_input: Instant::now(),
            idle_threshold: Duration::from_secs(5),
            is_mobile: false,
        }
    }

    /// Create mobile-optimized activity tracker
    ///
    /// More aggressive throttling for battery savings
    pub fn new_mobile() -> Self {
        Self {
            last_input: Instant::now(),
            idle_threshold: Duration::from_secs(3), // Faster idle detection on mobile
            is_mobile: true,
        }
    }

    /// Record user input activity
    ///
    /// Call this on keyboard, mouse, touch events
    pub fn on_input(&mut self) {
        self.last_input = Instant::now();
    }

    /// Check if user is idle
    ///
    /// Returns true if no input for idle_threshold duration
    pub fn is_idle(&self) -> bool {
        self.last_input.elapsed() > self.idle_threshold
    }

    /// Get target FPS based on activity and platform
    ///
    /// Desktop:
    /// - Active (recent input): 60 FPS (smooth interaction)
    /// - Idle (5+ sec no input): 30 FPS (battery savings)
    ///
    /// Mobile:
    /// - Active (recent input): 30 FPS (balance)
    /// - Idle (3+ sec no input): 15 FPS (max battery savings)
    pub fn target_fps(&self) -> u32 {
        if self.is_mobile {
            // Mobile: More aggressive throttling
            if self.is_idle() {
                15 // Mobile idle: 15 FPS (75% less GPU work)
            } else {
                30 // Mobile active: 30 FPS (50% less GPU work)
            }
        } else {
            // Desktop: Smooth when active, throttle when idle
            if self.is_idle() {
                30 // Desktop idle: 30 FPS (50% less GPU work)
            } else {
                60 // Desktop active: 60 FPS (smooth interaction)
            }
        }
    }

    /// Get target frame duration for current activity
    ///
    /// Returns Duration to sleep between frames
    pub fn frame_duration(&self) -> Duration {
        let target_fps = self.target_fps();
        Duration::from_micros(1_000_000 / target_fps as u64)
    }

    /// Get time since last input
    ///
    /// Useful for debugging or UI feedback
    pub fn idle_duration(&self) -> Duration {
        self.last_input.elapsed()
    }
}

impl Default for ActivityTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desktop_active_fps() {
        let tracker = ActivityTracker::new();
        assert_eq!(tracker.target_fps(), 60, "Desktop active should be 60 FPS");
    }

    #[test]
    fn test_desktop_idle_fps() {
        let mut tracker = ActivityTracker::new();
        tracker.last_input = Instant::now() - Duration::from_secs(10); // Force idle

        assert!(tracker.is_idle(), "Should be idle after 10 seconds");
        assert_eq!(tracker.target_fps(), 30, "Desktop idle should be 30 FPS");
    }

    #[test]
    fn test_mobile_active_fps() {
        let tracker = ActivityTracker::new_mobile();
        assert_eq!(tracker.target_fps(), 30, "Mobile active should be 30 FPS");
    }

    #[test]
    fn test_mobile_idle_fps() {
        let mut tracker = ActivityTracker::new_mobile();
        tracker.last_input = Instant::now() - Duration::from_secs(5); // Force idle

        assert!(tracker.is_idle(), "Should be idle after 5 seconds");
        assert_eq!(tracker.target_fps(), 15, "Mobile idle should be 15 FPS");
    }

    #[test]
    fn test_on_input_resets_idle() {
        let mut tracker = ActivityTracker::new();
        tracker.last_input = Instant::now() - Duration::from_secs(10); // Force idle

        assert!(tracker.is_idle(), "Should be idle initially");

        tracker.on_input(); // Simulate user input

        assert!(!tracker.is_idle(), "Should not be idle after input");
        assert_eq!(tracker.target_fps(), 60, "Should return to 60 FPS");
    }

    #[test]
    fn test_frame_duration_calculation() {
        let tracker = ActivityTracker::new();
        let frame_duration = tracker.frame_duration();

        // 60 FPS = 16.67ms per frame
        assert!(
            frame_duration.as_micros() >= 16_600 && frame_duration.as_micros() <= 16_700,
            "60 FPS should be ~16.67ms per frame"
        );
    }

    #[test]
    fn test_activity_transition() {
        let mut tracker = ActivityTracker::new();

        // Start active
        assert!(!tracker.is_idle());
        assert_eq!(tracker.target_fps(), 60);

        // Simulate time passing (mock idle)
        tracker.last_input = Instant::now() - Duration::from_secs(6);
        assert!(tracker.is_idle());
        assert_eq!(tracker.target_fps(), 30);

        // User input brings back to active
        tracker.on_input();
        assert!(!tracker.is_idle());
        assert_eq!(tracker.target_fps(), 60);
    }
}
