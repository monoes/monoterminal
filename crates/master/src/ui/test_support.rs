//! Test support utilities for headless GPU testing
//!
//! Provides GPU context creation without display/window for unit and integration tests.
//! Uses wgpu offscreen rendering to exercise real GPU code paths in `cargo test`.

use anyhow::{Context, Result};
use wgpu;

/// Headless GPU context for testing (no window/surface required)
///
/// Uses wgpu::RequestAdapterOptions with `compatible_surface: None` to create
/// a GPU context that can render to textures without a display.
///
/// Falls back to software rendering (WARP on Windows, llvmpipe on Linux) if no GPU.
pub struct HeadlessGpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl HeadlessGpuContext {
    /// Create new headless GPU context
    ///
    /// # Example
    /// ```no_run
    /// use monoterminal_master::ui::test_support::HeadlessGpuContext;
    ///
    /// #[tokio::test]
    /// async fn test_gpu_rendering() {
    ///     let ctx = HeadlessGpuContext::new().await.unwrap();
    ///     let target = ctx.create_render_target(1920, 1080);
    ///     // ... render to target ...
    /// }
    /// ```
    pub async fn new() -> Result<Self> {
        Self::new_with_backends(wgpu::Backends::all()).await
    }

    /// Create headless context with specific backend(s)
    ///
    /// Useful for testing specific backends:
    /// - `wgpu::Backends::DX12` - DirectX 12 (Windows)
    /// - `wgpu::Backends::VULKAN` - Vulkan (Linux/Windows)
    /// - `wgpu::Backends::METAL` - Metal (macOS)
    /// - `wgpu::Backends::GL` - OpenGL (fallback)
    ///
    /// # Software Rendering Fallback
    ///
    /// If no hardware adapter is found, automatically falls back to software rendering:
    /// - Windows: WARP (DirectX software rasterizer)
    /// - Linux: Mesa llvmpipe
    /// - macOS: Software renderer (if available)
    ///
    /// This ensures tests pass in CI environments without GPU hardware.
    pub async fn new_with_backends(backends: wgpu::Backends) -> Result<Self> {
        tracing::debug!(
            "Creating headless GPU context with backends: {:?}",
            backends
        );

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        // Try hardware adapter first (preferred for performance)
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None, // ← Headless mode (no window)
                force_fallback_adapter: false,
            })
            .await;

        // Fallback to software adapter if hardware unavailable (CI environments)
        let adapter = match adapter {
            Some(adapter) => {
                tracing::info!(
                    "Hardware GPU adapter found: {} ({:?})",
                    adapter.get_info().name,
                    adapter.get_info().backend
                );
                adapter
            }
            None => {
                tracing::warn!(
                    "No hardware GPU adapter available, falling back to software rendering"
                );
                tracing::debug!("Requesting software adapter with force_fallback_adapter: true");

                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface: None,
                        force_fallback_adapter: true, // ← Software rendering (WARP/llvmpipe)
                    })
                    .await
                    .context(
                        "Failed to find GPU adapter (no hardware or software rendering available)",
                    )?
            }
        };

        tracing::info!(
            "Using GPU adapter: {} ({:?}, device_type: {:?})",
            adapter.get_info().name,
            adapter.get_info().backend,
            adapter.get_info().device_type
        );

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("headless_test_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .context("Failed to create GPU device")?;

        tracing::debug!("Headless GPU context created successfully");

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    /// Create offscreen render target texture
    ///
    /// Returns a texture that can be used as a render attachment in tests.
    /// Supports reading pixels back via `read_pixels()`.
    pub fn create_render_target(&self, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test_render_target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// Read pixels from texture (for validation)
    ///
    /// Copies texture data to CPU-accessible buffer and returns RGBA8 bytes.
    ///
    /// # Example
    /// ```no_run
    /// # use monoterminal_master::ui::test_support::HeadlessGpuContext;
    /// # async fn example() {
    /// # let ctx = HeadlessGpuContext::new().await.unwrap();
    /// let target = ctx.create_render_target(800, 600);
    /// // ... render to target ...
    /// let pixels = ctx.read_pixels(&target, 800, 600).await.unwrap();
    /// assert_eq!(pixels.len(), 800 * 600 * 4); // RGBA
    /// # }
    /// ```
    pub async fn read_pixels(
        &self,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>> {
        let bytes_per_row = width * 4; // RGBA8
        let padded_bytes_per_row = Self::padded_bytes_per_row(bytes_per_row);
        let buffer_size = (padded_bytes_per_row * height) as u64;

        // Create staging buffer to copy texture data
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("read_pixels_encoder"),
            });

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        // Map buffer and read data
        let buffer_slice = staging_buffer.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, |_| {});

        self.device.poll(wgpu::Maintain::Wait);

        let data = buffer_slice.get_mapped_range();

        // Remove padding and copy to contiguous buffer
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + bytes_per_row as usize;
            pixels.extend_from_slice(&data[start..end]);
        }

        drop(data);
        staging_buffer.unmap();

        Ok(pixels)
    }

    /// Calculate padded bytes per row (wgpu requires 256-byte alignment)
    fn padded_bytes_per_row(bytes_per_row: u32) -> u32 {
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

        (bytes_per_row + align - 1) / align * align
    }

    /// Get adapter info (for debugging)
    pub fn adapter_info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_headless_context_creation() {
        let ctx = HeadlessGpuContext::new().await.unwrap();

        let info = ctx.adapter_info();
        println!("GPU Adapter: {} ({:?})", info.name, info.backend);

        // Verify we got a valid device
        assert!(!info.name.is_empty());
    }

    #[tokio::test]
    async fn test_create_render_target() {
        let ctx = HeadlessGpuContext::new().await.unwrap();
        let target = ctx.create_render_target(1920, 1080);

        // Verify texture properties
        assert_eq!(target.width(), 1920);
        assert_eq!(target.height(), 1080);
        assert_eq!(target.format(), wgpu::TextureFormat::Rgba8UnormSrgb);
    }

    #[tokio::test]
    async fn test_read_pixels_blank() {
        let ctx = HeadlessGpuContext::new().await.unwrap();
        let target = ctx.create_render_target(100, 100);

        // Read pixels from blank texture
        let pixels = ctx.read_pixels(&target, 100, 100).await.unwrap();

        // Should get 100x100 RGBA pixels
        assert_eq!(pixels.len(), 100 * 100 * 4);
    }

    #[tokio::test]
    async fn test_render_clear_color() {
        let ctx = HeadlessGpuContext::new().await.unwrap();
        let target = ctx.create_render_target(64, 64);

        // Clear texture to red
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test_encoder"),
            });

        {
            let view = target.create_view(&wgpu::TextureViewDescriptor::default());
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("test_clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        ctx.queue.submit(Some(encoder.finish()));

        // Read pixels and verify red color
        let pixels = ctx.read_pixels(&target, 64, 64).await.unwrap();

        // Check first pixel (should be red: 255, 0, 0, 255)
        assert_eq!(pixels[0], 255); // R
        assert_eq!(pixels[1], 0); // G
        assert_eq!(pixels[2], 0); // B
        assert_eq!(pixels[3], 255); // A
    }

    #[tokio::test]
    async fn test_backends_dx12() {
        // Windows-specific: test DirectX 12 backend
        #[cfg(target_os = "windows")]
        {
            let ctx = HeadlessGpuContext::new_with_backends(wgpu::Backends::DX12)
                .await
                .unwrap();

            let info = ctx.adapter_info();
            assert_eq!(info.backend, wgpu::Backend::Dx12);
        }
    }

    #[tokio::test]
    async fn test_backends_vulkan() {
        // Try Vulkan backend (available on Windows/Linux)
        if let Ok(ctx) = HeadlessGpuContext::new_with_backends(wgpu::Backends::VULKAN).await {
            let info = ctx.adapter_info();
            assert_eq!(info.backend, wgpu::Backend::Vulkan);
        }
    }
}
