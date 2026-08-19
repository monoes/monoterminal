//! wgpu Renderer - DirectX 12 Backend
//!
//! Manages GPU resources and rendering pipeline
//! Target: 8ms GPU render time (per SRS Â§2.1.1)

use anyhow::{Context, Result};
use bytes::Bytes;
use std::sync::Arc;
use wgpu;
use winit::window::Window;

use super::glyph_cache::{GlyphCache, GlyphKey};
use super::performance::PerformanceMonitor;
use super::terminal_grid::TerminalGrid;
use super::vt_parser::VtParser;

/// Vertex data for a single glyph quad corner
/// Layout matches WGSL VertexInput in shaders/text.wgsl
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphVertex {
    /// NDC position (-1.0 to 1.0)
    position: [f32; 2],
    /// Atlas texture coordinates (0.0 to 1.0)
    tex_coords: [f32; 2],
    /// Foreground color (RGBA, 0.0 to 1.0)
    fg_color: [f32; 4],
    /// Background color (RGBA, 0.0 to 1.0)
    bg_color: [f32; 4],
}

impl GlyphVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x2,  // tex_coords
        2 => Float32x4,  // fg_color
        3 => Float32x4,  // bg_color
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GlyphVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Main renderer managing wgpu resources
pub struct Renderer {
    instance: wgpu::Instance,
    adapter: Option<wgpu::Adapter>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,

    // Text rendering pipeline (Option A: single-pass with bg_color)
    text_pipeline: Option<wgpu::RenderPipeline>,
    atlas_texture: Option<wgpu::Texture>,
    atlas_texture_view: Option<wgpu::TextureView>,
    atlas_sampler: Option<wgpu::Sampler>,
    bind_group: Option<wgpu::BindGroup>,
    vertex_buffer: Option<wgpu::Buffer>,

    // egui integration (TODO: Add egui_wgpu_renderer on Day 2)
    // For now, we prepare the terminal rendering pipeline

    // Terminal rendering state
    terminal_grid: TerminalGrid,
    vt_parser: VtParser,
    glyph_cache: GlyphCache,
    font_manager: super::fonts::FontManager,

    // Cell dimensions (pixels)
    cell_width: u32,
    cell_height: u32,

    // RendererBridge - connects SessionManager PTY output to GPU rendering
    renderer_bridge: Option<super::renderer_bridge::RendererBridge>,
}

impl Renderer {
    /// Create new renderer
    /// Initializes wgpu instance with platform-appropriate backend
    pub async fn new(_event_loop: &winit::event_loop::EventLoop<()>) -> Result<Self> {
        tracing::info!("Creating wgpu renderer");

        // Select backend appropriate for the platform (Phase 3 Week 6 - cross-platform support)
        let backends = super::backend_selection::select_backend();

        // Create wgpu instance with platform-specific backend
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        tracing::info!("wgpu instance created with {:?} backend", backends);

        // Initialize font manager (16px Consolas for Windows Phase 1)
        let font_manager =
            super::fonts::FontManager::new(16.0).context("Failed to create FontManager")?;
        let (cell_width, cell_height) = font_manager.cell_dimensions();

        tracing::info!(
            "Font loaded: cell size {}x{} pixels",
            cell_width,
            cell_height
        );

        Ok(Self {
            instance,
            adapter: None,
            device: None,
            queue: None,
            surface: None,
            surface_config: None,

            // Text rendering pipeline (initialized in init_text_pipeline())
            text_pipeline: None,
            atlas_texture: None,
            atlas_texture_view: None,
            atlas_sampler: None,
            bind_group: None,
            vertex_buffer: None,

            // Terminal rendering state (80x24 default)
            terminal_grid: TerminalGrid::new(24, 80),
            vt_parser: VtParser::new(),
            glyph_cache: GlyphCache::new(),
            font_manager,
            cell_width,
            cell_height,

            // RendererBridge (attached after session creation)
            renderer_bridge: None,
        })
    }

    /// Attach RendererBridge for PTY output streaming
    /// Connects SessionManager PTY output to GPU rendering pipeline
    ///
    /// # Arguments
    /// - `bridge`: RendererBridge instance (from SessionManager attachment)
    ///
    /// # Integration Point
    /// Called after SessionManager.attach_client() establishes PTY stream
    pub fn attach_renderer_bridge(&mut self, bridge: super::renderer_bridge::RendererBridge) {
        tracing::info!("Attaching RendererBridge to GPU rendering pipeline");
        self.renderer_bridge = Some(bridge);
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

        if let (Some(surface), Some(device), Some(config)) =
            (&self.surface, &self.device, &mut self.surface_config)
        {
            config.width = width;
            config.height = height;
            surface.configure(device, config);

            tracing::debug!("Surface resized to {}x{}", width, height);
        }

        Ok(())
    }

    /// Initialize text rendering pipeline
    /// Creates shaders, atlas texture, sampler, and render pipeline
    /// Called after surface initialization (window.rs:91-97)
    pub fn init_text_pipeline(&mut self) -> Result<()> {
        tracing::info!(
            "Initializing text rendering pipeline (Option A: single-pass with bg_color)"
        );

        let device = self.device.as_ref().context("Device not initialized")?;
        let config = self
            .surface_config
            .as_ref()
            .context("Surface config not initialized")?;

        // 1. Create glyph atlas texture (4096Ã—4096 R8Unorm, per SRS Â§2.1.1)
        let (atlas_width, atlas_height) = self.glyph_cache.atlas_size();
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas Texture"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm, // Grayscale (8-bit)
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let atlas_texture_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        tracing::info!(
            "Glyph atlas created: {}x{} R8Unorm",
            atlas_width,
            atlas_height
        );

        // 2. Create sampler (LINEAR filter for antialiasing, per gpu-rendering-engineer recommendation)
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Glyph Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear, // â† Linear for antialiasing
            min_filter: wgpu::FilterMode::Linear, // â† Linear for antialiasing
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        tracing::info!("Atlas sampler created with linear filtering");

        // 3. Load shader from shaders/text.wgsl
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/text.wgsl").into()),
        });

        tracing::info!("Shader module loaded (vertex + fragment)");

        // 4. Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text Bind Group Layout"),
            entries: &[
                // @group(0) @binding(0) - atlas texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // @group(0) @binding(1) - atlas sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // 5. Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // 6. Create render pipeline
        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                module: &shader,
                entry_point: "vs_main",
                buffers: &[GlyphVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for 2D quads
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None, // No depth testing for 2D
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        tracing::info!("Render pipeline created");

        // 7. Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        // 8. Create vertex buffer (sized for full-screen update: 80Ã—24 = 1920 cells Ã— 6 vertices)
        let max_vertices = 1920 * 6; // 80Ã—24 grid, 6 vertices per cell (2 triangles)
        let vertex_buffer_size = (max_vertices * std::mem::size_of::<GlyphVertex>()) as u64;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Glyph Vertex Buffer"),
            size: vertex_buffer_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        tracing::info!(
            "Vertex buffer created: {} vertices ({}KB)",
            max_vertices,
            vertex_buffer_size / 1024
        );

        // Store pipeline resources
        self.text_pipeline = Some(text_pipeline);
        self.atlas_texture = Some(atlas_texture);
        self.atlas_texture_view = Some(atlas_texture_view);
        self.atlas_sampler = Some(atlas_sampler);
        self.bind_group = Some(bind_group);
        self.vertex_buffer = Some(vertex_buffer);

        tracing::info!("âœ… Text rendering pipeline initialized successfully");

        Ok(())
    }

    /// Render a frame
    /// Target: 8ms GPU render + 5ms VSync = 13ms total
    pub fn render(&mut self, _window: &Window, perf: &mut PerformanceMonitor) -> Result<()> {
        // Process PTY output FIRST (mutable borrow)
        self.process_pty_output(perf)?;

        // Build vertex data from dirty cells SECOND (mutable borrow for glyph cache)
        // Target: <1ms glyph lookup + 0.5ms dirty tracking
        perf.mark("dirty_tracking_start");
        let vertices = self.build_vertices(perf)?;
        perf.mark("glyph_lookup");

        // NOW borrow resources immutably (all mutable work done)
        let surface = self.surface.as_ref().context("Surface not initialized")?;
        let device = self.device.as_ref().context("Device not initialized")?;
        let queue = self.queue.as_ref().context("Queue not initialized")?;

        // Get current surface texture
        let output = surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        perf.mark("acquire_texture");

        // Create command encoder
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Upload vertices to GPU
        if !vertices.is_empty() {
            queue.write_buffer(
                self.vertex_buffer
                    .as_ref()
                    .context("Vertex buffer not initialized")?,
                0,
                bytemuck::cast_slice(&vertices),
            );
        }
        perf.mark("upload_vertices");

        {
            // Begin render pass with clear color (terminal background)
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

            perf.mark("render_pass_begin");

            // Draw glyphs if we have vertices
            if !vertices.is_empty() {
                render_pass.set_pipeline(
                    self.text_pipeline
                        .as_ref()
                        .context("Pipeline not initialized")?,
                );
                render_pass.set_bind_group(
                    0,
                    self.bind_group
                        .as_ref()
                        .context("Bind group not initialized")?,
                    &[],
                );
                render_pass.set_vertex_buffer(
                    0,
                    self.vertex_buffer
                        .as_ref()
                        .context("Vertex buffer not initialized")?
                        .slice(..),
                );
                render_pass.draw(0..vertices.len() as u32, 0..1);
            }

            perf.mark("render_pass");
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
    /// Target: <2ms parse time per SRS Â§2.1.1
    fn process_pty_output(&mut self, perf: &mut PerformanceMonitor) -> Result<()> {
        // Try to receive PTY data (non-blocking) via RendererBridge
        if let Some(bridge) = &mut self.renderer_bridge {
            while let Some(data) = bridge.try_recv() {
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

    /// Build vertex data from dirty terminal cells
    /// Target: <0.5ms dirty tracking + <1ms glyph lookup = <1.5ms total
    /// Returns vertex array ready for GPU upload
    fn build_vertices(&mut self, perf: &mut PerformanceMonitor) -> Result<Vec<GlyphVertex>> {
        let mut vertices = Vec::new();

        let surface_config = self
            .surface_config
            .as_ref()
            .context("Surface not configured")?;
        let (rows, cols) = self.terminal_grid.dimensions();

        // Calculate NDC (Normalized Device Coordinates) scale
        // NDC: -1.0 (left/bottom) to +1.0 (right/top)
        let viewport_width = surface_config.width as f32;
        let viewport_height = surface_config.height as f32;
        let cell_width_ndc = (self.cell_width as f32 / viewport_width) * 2.0;
        let cell_height_ndc = (self.cell_height as f32 / viewport_height) * 2.0;

        perf.mark("dirty_tracking");

        // Collect dirty cells into Vec to release immutable borrow before calling ensure_glyph_cached
        // (ensure_glyph_cached needs mutable borrow of self)
        let dirty_cells: Vec<_> = self
            .terminal_grid
            .dirty_region()
            .iter_dirty()
            .filter_map(|(row, col)| {
                self.terminal_grid
                    .get_cell(row, col)
                    .map(|cell| (row, col, *cell))
            })
            .collect();

        // Now process dirty cells (mutable borrow allowed)
        for (row, col, cell) in dirty_cells {
            // Ensure glyph is in cache (rasterize + upload if needed)
            let glyph_key = GlyphKey::new(cell.ch, &cell.style);
            let glyph_info = self.ensure_glyph_cached(glyph_key)?;

            // Calculate NDC position for this cell
            // Origin is top-left, NDC origin is center
            let x_ndc = -1.0 + (col as f32 * cell_width_ndc);
            let y_ndc = 1.0 - (row as f32 * cell_height_ndc);

            // Convert colors from terminal style to RGBA floats
            let fg_color = color_to_rgba(&cell.style.fg);
            let bg_color = color_to_rgba(&cell.style.bg);

            // Build 6 vertices for quad (2 triangles)
            // Triangle 1: top-left, top-right, bottom-left
            // Triangle 2: top-right, bottom-right, bottom-left
            let (tex_x, tex_y, tex_w, tex_h) = (
                glyph_info.tex_x,
                glyph_info.tex_y,
                glyph_info.tex_width,
                glyph_info.tex_height,
            );

            // Top-left
            vertices.push(GlyphVertex {
                position: [x_ndc, y_ndc],
                tex_coords: [tex_x, tex_y],
                fg_color,
                bg_color,
            });

            // Top-right
            vertices.push(GlyphVertex {
                position: [x_ndc + cell_width_ndc, y_ndc],
                tex_coords: [tex_x + tex_w, tex_y],
                fg_color,
                bg_color,
            });

            // Bottom-left
            vertices.push(GlyphVertex {
                position: [x_ndc, y_ndc - cell_height_ndc],
                tex_coords: [tex_x, tex_y + tex_h],
                fg_color,
                bg_color,
            });

            // Triangle 2
            // Top-right (duplicate)
            vertices.push(GlyphVertex {
                position: [x_ndc + cell_width_ndc, y_ndc],
                tex_coords: [tex_x + tex_w, tex_y],
                fg_color,
                bg_color,
            });

            // Bottom-right
            vertices.push(GlyphVertex {
                position: [x_ndc + cell_width_ndc, y_ndc - cell_height_ndc],
                tex_coords: [tex_x + tex_w, tex_y + tex_h],
                fg_color,
                bg_color,
            });

            // Bottom-left (duplicate)
            vertices.push(GlyphVertex {
                position: [x_ndc, y_ndc - cell_height_ndc],
                tex_coords: [tex_x, tex_y + tex_h],
                fg_color,
                bg_color,
            });
        }

        // Clear dirty region after rendering
        self.terminal_grid.clear_dirty();

        Ok(vertices)
    }

    /// Ensure glyph is cached and uploaded to GPU atlas
    /// Target: <1ms glyph lookup per SRS §2.1.1
    /// Returns GlyphInfo for rendering
    fn ensure_glyph_cached(&mut self, key: GlyphKey) -> Result<super::glyph_cache::GlyphInfo> {
        // Fast path: glyph already in cache
        if let Some(info) = self.glyph_cache.lookup(key) {
            return Ok(info);
        }

        // Slow path: rasterize glyph and upload to GPU
        let rasterized = self
            .font_manager
            .rasterize_glyph(key.ch, key.bold, key.italic)?;

        // Allocate space in atlas
        let glyph_info = self
            .glyph_cache
            .insert(key, rasterized.width as u32, rasterized.height as u32)
            .context("Failed to allocate space in glyph atlas")?;

        // Upload glyph bitmap to GPU texture atlas
        let queue = self.queue.as_ref().context("Queue not initialized")?;
        let atlas_texture = self
            .atlas_texture
            .as_ref()
            .context("Atlas texture not initialized")?;

        // Calculate atlas pixel coordinates from normalized UV
        let (atlas_width, atlas_height) = self.glyph_cache.atlas_size();
        let atlas_x = (glyph_info.tex_x * atlas_width as f32) as u32;
        let atlas_y = (glyph_info.tex_y * atlas_height as f32) as u32;

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: atlas_x,
                    y: atlas_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &rasterized.bitmap,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(rasterized.width as u32),
                rows_per_image: Some(rasterized.height as u32),
            },
            wgpu::Extent3d {
                width: rasterized.width as u32,
                height: rasterized.height as u32,
                depth_or_array_layers: 1,
            },
        );

        Ok(glyph_info)
    }
}

/// Convert terminal Color (24-bit RGB) to RGBA float array (0.0-1.0)
/// Supports true-color per SRS §2.1.1
fn color_to_rgba(color: &super::vt_parser::Color) -> [f32; 4] {
    [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        1.0,
    ]
}
