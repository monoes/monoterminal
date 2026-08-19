//! MONOTERMINAL Master Local UI
//!
//! wgpu + egui rendering system targeting 60 FPS (16.67ms frame budget)
//! Per SRS §2.1.1, §4.2.1
//!
//! Frame Budget Breakdown:
//! - PTY read: 2ms (handled by backend)
//! - Dirty tracking: 0.5ms
//! - Glyph lookup: 1ms
//! - GPU render: 8ms
//! - VSync: 5ms
//! Total: 16.5ms ✅

pub mod backend_selection;
pub mod fonts;
pub mod glyph_cache;
pub mod layout;
pub mod performance;
pub mod renderer;
pub mod renderer_bridge;
pub mod terminal_grid;
pub mod vt_parser;
pub mod window;

// Test support (for tests and integration tests)
#[cfg(any(test, feature = "test_support"))]
pub mod test_support;

pub use renderer::Renderer;
pub use renderer_bridge::RendererBridge;
pub use window::Window;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_support_integration;

use anyhow::Result;

/// Main UI entry point
/// Initializes window, wgpu, and egui
pub struct TerminalUI {
    window: Window,
    renderer: Renderer,
}

impl TerminalUI {
    /// Create new Terminal UI
    /// Sets up DirectX 12 backend on Windows
    pub async fn new() -> Result<Self> {
        tracing::info!("Initializing Terminal UI (wgpu + egui)");

        let window = Window::new()?;
        let renderer = Renderer::new(window.inner()).await?;

        Ok(Self { window, renderer })
    }

    /// Run the event loop
    /// Target: 60 FPS (16.67ms frame budget)
    pub fn run(self) -> Result<()> {
        self.window.run(self.renderer)
    }
}
