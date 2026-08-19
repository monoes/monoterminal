//! Performance monitoring and frame timing
//!
//! Tracks frame budgets per SRS §2.1.1:
//!
//! - PTY read: 2ms
//! - Dirty tracking: 0.5ms
//! - Glyph lookup: 1ms
//! - GPU render: 8ms
//! - VSync: 5ms
//!
//! Target: 16.67ms (60 FPS)

use std::collections::VecDeque;
use std::time::{Duration, Instant};

const FPS_HISTORY_SIZE: usize = 60;

/// Performance monitor tracking frame timing
pub struct PerformanceMonitor {
    frame_start: Option<Instant>,
    last_mark: Option<Instant>,
    frame_times: VecDeque<f32>,
    fps: f32,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            frame_start: None,
            last_mark: None,
            frame_times: VecDeque::with_capacity(FPS_HISTORY_SIZE),
            fps: 0.0,
        }
    }

    /// Start timing a new frame
    pub fn start_frame(&mut self) {
        let now = Instant::now();
        self.frame_start = Some(now);
        self.last_mark = Some(now);
    }

    /// Mark a timing point within the frame
    pub fn mark(&mut self, label: &str) {
        if let Some(last) = self.last_mark {
            let elapsed = last.elapsed();
            let ms = elapsed.as_secs_f32() * 1000.0;

            // Log if phase exceeds expected budget
            let budget = match label {
                "dirty_tracking" => Some(0.5),
                "glyph_lookup" => Some(1.0),
                "render_pass" => Some(8.0),
                _ => None,
            };

            if let Some(budget) = budget {
                if ms > budget {
                    tracing::debug!(
                        "Phase '{}' exceeded budget: {:.2}ms (target: {:.2}ms)",
                        label,
                        ms,
                        budget
                    );
                }
            }

            self.last_mark = Some(Instant::now());
        }
    }

    /// End frame timing and return frame time in ms
    pub fn end_frame(&mut self) -> f32 {
        if let Some(start) = self.frame_start {
            let frame_time = start.elapsed().as_secs_f32() * 1000.0;

            // Update frame time history
            self.frame_times.push_back(frame_time);
            if self.frame_times.len() > FPS_HISTORY_SIZE {
                self.frame_times.pop_front();
            }

            // Calculate FPS (average over history)
            if !self.frame_times.is_empty() {
                let avg_frame_time: f32 =
                    self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
                self.fps = 1000.0 / avg_frame_time;
            }

            frame_time
        } else {
            0.0
        }
    }

    /// Get current FPS
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// Get average frame time in ms
    pub fn avg_frame_time(&self) -> f32 {
        if self.frame_times.is_empty() {
            0.0
        } else {
            self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32
        }
    }

    /// Check if currently meeting 60 FPS target
    pub fn meeting_target(&self) -> bool {
        self.avg_frame_time() <= 16.67
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_performance_monitor_new() {
        let monitor = PerformanceMonitor::new();
        assert_eq!(monitor.fps(), 0.0);
        assert_eq!(monitor.avg_frame_time(), 0.0);
    }

    #[test]
    fn test_performance_monitor_default() {
        let monitor = PerformanceMonitor::default();
        assert_eq!(monitor.fps(), 0.0);
    }

    #[test]
    fn test_start_frame() {
        let mut monitor = PerformanceMonitor::new();
        monitor.start_frame();

        assert!(monitor.frame_start.is_some());
        assert!(monitor.last_mark.is_some());
    }

    #[test]
    fn test_mark_timing() {
        let mut monitor = PerformanceMonitor::new();
        monitor.start_frame();

        // Simulate some work
        thread::sleep(Duration::from_millis(1));

        monitor.mark("test_phase");
        // Should not panic
    }

    #[test]
    fn test_end_frame() {
        let mut monitor = PerformanceMonitor::new();
        monitor.start_frame();

        // Simulate frame
        thread::sleep(Duration::from_millis(10));

        monitor.end_frame();

        // FPS should be calculated
        assert!(monitor.fps() > 0.0);
        assert!(monitor.avg_frame_time() > 0.0);
    }

    #[test]
    fn test_fps_calculation() {
        let mut monitor = PerformanceMonitor::new();

        // Simulate several frames at ~60 FPS (16.67ms each)
        for _ in 0..60 {
            monitor.start_frame();
            thread::sleep(Duration::from_millis(16));
            monitor.end_frame();
        }

        let fps = monitor.fps();
        println!("Measured FPS: {:.1}", fps);

        // FPS should be around 50-65 (sleep is not precise)
        assert!(
            fps > 40.0 && fps < 70.0,
            "FPS out of expected range: {:.1}",
            fps
        );
    }

    #[test]
    fn test_avg_frame_time() {
        let mut monitor = PerformanceMonitor::new();

        // Simulate frames
        for _ in 0..10 {
            monitor.start_frame();
            thread::sleep(Duration::from_millis(16));
            monitor.end_frame();
        }

        let avg = monitor.avg_frame_time();
        println!("Average frame time: {:.2}ms", avg);

        // Should be around 16ms (with some variance)
        assert!(
            avg > 10.0 && avg < 25.0,
            "Frame time out of expected range: {:.2}ms",
            avg
        );
    }

    #[test]
    fn test_frame_history_limit() {
        let mut monitor = PerformanceMonitor::new();

        // Add more frames than history size (60)
        for _ in 0..100 {
            monitor.start_frame();
            thread::sleep(Duration::from_millis(1));
            monitor.end_frame();
        }

        // History should be capped at 60
        assert_eq!(monitor.frame_times.len(), 60);
    }

    #[test]
    fn test_mark_budget_warning() {
        let mut monitor = PerformanceMonitor::new();
        monitor.start_frame();

        // Simulate exceeding budget
        thread::sleep(Duration::from_millis(2));
        monitor.mark("glyph_lookup"); // Budget: 1ms, actual: 2ms

        // Should log warning (no panic)
    }

    #[test]
    fn test_multiple_frames() {
        let mut monitor = PerformanceMonitor::new();

        for i in 0..10 {
            monitor.start_frame();
            monitor.mark("phase1");
            monitor.mark("phase2");
            monitor.end_frame();

            if i > 0 {
                assert!(monitor.fps() > 0.0);
            }
        }
    }

    #[test]
    fn test_fps_smoothing() {
        let mut monitor = PerformanceMonitor::new();

        // First frame: slow
        monitor.start_frame();
        thread::sleep(Duration::from_millis(50));
        monitor.end_frame();
        let fps1 = monitor.fps();

        // Next frames: fast
        for _ in 0..20 {
            monitor.start_frame();
            thread::sleep(Duration::from_millis(10));
            monitor.end_frame();
        }
        let fps2 = monitor.fps();

        // FPS should improve as more fast frames are added
        assert!(fps2 > fps1, "FPS should improve: {} -> {}", fps1, fps2);
    }
}
