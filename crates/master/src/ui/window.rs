//! Window management and event loop
//!
//! Handles the main window creation and event processing
//! Per SRS §4.2.1 - Desktop App Architecture

use anyhow::Result;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window as WinitWindow, WindowAttributes},
};

use super::{performance::PerformanceMonitor, Renderer};

/// Window wrapper managing the native window
pub struct Window {
    event_loop: EventLoop<()>,
    window_attributes: WindowAttributes,
}

impl Window {
    /// Create new window with default size
    pub fn new() -> Result<Self> {
        let event_loop = EventLoop::new()?;

        // Default window size: 1280x720 (720p)
        let window_attributes = WinitWindow::default_attributes()
            .with_title("MONOTERMINAL")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0));

        Ok(Self {
            event_loop,
            window_attributes,
        })
    }

    /// Get reference to inner window for renderer initialization
    pub fn inner(&self) -> &EventLoop<()> {
        &self.event_loop
    }

    /// Run the event loop with the renderer
    pub fn run(self, renderer: Renderer) -> Result<()> {
        let mut app = App::new(self.window_attributes, renderer);

        self.event_loop.run_app(&mut app)?;

        Ok(())
    }
}

/// Application handler for event loop
struct App {
    window_attributes: WindowAttributes,
    window: Option<Arc<WinitWindow>>,
    renderer: Renderer,
    perf_monitor: PerformanceMonitor,
}

impl App {
    fn new(window_attributes: WindowAttributes, renderer: Renderer) -> Self {
        Self {
            window_attributes,
            window: None,
            renderer,
            perf_monitor: PerformanceMonitor::new(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window = Arc::new(
                event_loop
                    .create_window(self.window_attributes.clone())
                    .expect("Failed to create window"),
            );

            // Initialize renderer surface
            if let Err(e) = pollster::block_on(self.renderer.init_surface(window.clone())) {
                tracing::error!("Failed to initialize renderer surface: {}", e);
                event_loop.exit();
                return;
            }

            // Initialize text rendering pipeline (shaders, atlas, sampler)
            if let Err(e) = self.renderer.init_text_pipeline() {
                tracing::error!("Failed to initialize text rendering pipeline: {}", e);
                event_loop.exit();
                return;
            }

            self.window = Some(window);
            tracing::info!("Window created and renderer initialized with text pipeline");
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                tracing::info!("Close requested");
                event_loop.exit();
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                tracing::info!("Escape pressed, exiting");
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                if let Err(e) = self.renderer.resize(new_size.width, new_size.height) {
                    tracing::error!("Failed to resize: {}", e);
                }
            }

            WindowEvent::RedrawRequested => {
                // Start frame timing
                self.perf_monitor.start_frame();

                // Render frame
                if let Some(window) = &self.window {
                    match self.renderer.render(window, &mut self.perf_monitor) {
                        Ok(_) => {
                            // End frame timing and check budget
                            let frame_time = self.perf_monitor.end_frame();

                            // Log if we exceed 16.67ms budget
                            if frame_time > 16.67 {
                                tracing::warn!(
                                    "Frame time exceeded budget: {:.2}ms (target: 16.67ms)",
                                    frame_time
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!("Render error: {}", e);
                        }
                    }
                }

                // Request next frame
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
