//! wgpu Renderer - DirectX 12 Backend
//!
//! Manages GPU resources and rendering pipeline
//! Target: 8ms GPU render time (per SRS §2.1.1)

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::mpsc;
use bytes::Bytes;
use wgpu;
use winit::window::Window;

use super::performance::PerformanceMonitor;
use super::terminal_grid::TerminalGrid;
use super::vt_parser::VtParser;
use super::glyph_cache::GlyphCache;

/// Main renderer managing wgpu resources
pub struct Renderer {
    instance: wgpu::Instance,
    adapter: Option<wgpu::Adapter>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,

    // egui integration (TODO: Add egui_wgpu_renderer on Day 2)
    // For now, we prepare the terminal rendering pipeline

    // Terminal rendering state
    terminal_grid: TerminalGrid,
    vt_parser: VtParser,
    glyph_cache: GlyphCache,

    // Mock PTY output channel (Day 1 - will replace with RendererBridge on Day 2)
    mock_pty_rx: Option<mpsc::Receiver<Bytes>>,
}

impl Renderer {
    /// Create new renderer
    /// Initializes wgpu instance with DirectX 12 backend preference
    pub async fn new(_event_loop: &winit::event_loop::EventLoop<()>) -> Result<Self> {
        tracing::info!("Creating wgpu renderer (DirectX 12)");

        // Create wgpu instance with DirectX 12 backend on Windows
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12, // DirectX 12 for Windows (Phase 1)
            ..Default::default()
        });

        tracing::info!("wgpu instance created with DirectX 12 backend");

        Ok(Self {
            instance,
            adapter: None,
            device: None,
            queue: None,
            surface: None,
            surface_config: None,

            // Terminal rendering state (80x24 default)
            terminal_grid: TerminalGrid::new(24, 80),
            vt_parser: VtParser::new(),
            glyph_cache: GlyphCache::new(),

            // Mock channel (Day 1)
            mock_pty_rx: None,
        })
    }

    /// Set mock PTY channel for Day 1 testing
    /// Will be replaced with RendererBridge on Day 2
    pub fn set_mock_pty_channel(&mut self, rx: mpsc::Receiver<Bytes>) {
        self.mock_pty_rx = Some(rx);
    }

    /// Initialize surface and request adapter/device
    /// Called after window is created
    pub async fn init_surface(&mut self, window: Arc<Window>) -> Result<()> {
        tracing::info!("Initializing wgpu surface");

        // Create surface
        // SAFETY: window must outlive surface
        let surface = self.instance.create_surface(window.clone())?;

        // Request adapter
        let adapter = self
            .instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("Failed to find suitable GPU adapter")?;

        tracing::info!("Adapter: {:?}", adapter.get_info());

        // Request device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("MONOTERMINAL Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .context("Failed to create device")?;

        tracing::info!("Device and queue created");

        // Configure surface
        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo, // VSync enabled (5ms budget)
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        tracing::info!(
            "Surface configured: {}x{}, format: {:?}",
            config.width,
            config.height,
            config.format
        );

        self.adapter = Some(adapter);
        self.device = Some(device);
        self.queue = Some(queue);
        self.surface = Some(surface);
        self.surface_config = Some(config);

        Ok(())
    }

    /// Resize surface
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }

        if let (Some(surface), Some(device), Some(config)) = (
            &self.surface,
            &self.device,
            &mut self.surface_config,
        ) {
            config.width = width;
            config.height = height;
            surface.configure(device, config);

            tracing::debug!("Surface resized to {}x{}", width, height);
        }

        Ok(())
    }

    /// Render a frame
    /// Target: 8ms GPU render + 5ms VSync = 13ms total
    pub fn render(&mut self, _window: &Window, perf: &mut PerformanceMonitor) -> Result<()> {
        // Process PTY output FIRST (before borrowing self.surface immutably)
        // This allows mutable borrow of self before immutable borrows begin
        self.process_pty_output(perf)?;

        // Now borrow surface, device, queue immutably
        let surface = self.surface.as_ref().context("Surface not initialized")?;
        let device = self.device.as_ref().context("Device not initialized")?;
        let queue = self.queue.as_ref().context("Queue not initialized")?;

        // Get current surface texture
        let output = surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        perf.mark("acquire_texture");

        // Create command encoder
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            // Begin render pass with clear color
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            perf.mark("render_pass");

            // TODO Day 2: Actual terminal rendering
            // - Iterate dirty cells in terminal_grid
            // - Lookup glyphs in glyph_cache
            // - Draw to texture
            // - egui UI overlay
        }

        // Submit commands
        queue.submit(std::iter::once(encoder.finish()));
        perf.mark("submit_queue");

        // Present frame
        output.present();
        perf.mark("present");

        Ok(())
    }

    /// Process PTY output and update terminal grid
    /// Target: <2ms parse time per SRS §2.1.1
    fn process_pty_output(&mut self, perf: &mut PerformanceMonitor) -> Result<()> {
        // Try to receive PTY data (non-blocking)
        if let Some(rx) = &mut self.mock_pty_rx {
            while let Ok(data) = rx.try_recv() {
                // Parse VT sequences
                self.vt_parser.feed(&data);

                // Apply changes to terminal grid
                let changes = self.vt_parser.drain_changes();
                self.terminal_grid.apply_changes(changes);

                perf.mark("pty_parse");
            }
        }

        Ok(())
    }
}
