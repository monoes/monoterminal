//! Headless GPU rendering integration tests
//!
//! Tests the wgpu renderer without a window/display using offscreen rendering.
//! Improves coverage of UI/renderer modules that cargo-tarpaulin can't normally reach.

use super::test_support::HeadlessGpuContext;

// ===== GPU Context Tests =====

#[tokio::test]
async fn test_gpu_adapter_detection() {
    let ctx = HeadlessGpuContext::new().await.unwrap();

    let info = ctx.adapter_info();
    println!("GPU Adapter: {}", info.name);
    println!("Backend: {:?}", info.backend);
    println!("Device Type: {:?}", info.device_type);

    // Should detect some adapter (hardware or software)
    assert!(!info.name.is_empty());
}

#[tokio::test]
async fn test_gpu_device_limits() {
    let ctx = HeadlessGpuContext::new().await.unwrap();

    let limits = ctx.device.limits();

    // Verify minimum required limits for terminal rendering
    assert!(limits.max_texture_dimension_2d >= 4096); // For glyph atlas
    assert!(limits.max_bind_groups >= 4);
    assert!(limits.max_vertex_attributes >= 4);
}

// ===== Render Target Tests =====

#[tokio::test]
async fn test_create_render_target_fullhd() {
    let ctx = HeadlessGpuContext::new().await.unwrap();
    let target = ctx.create_render_target(1920, 1080);

    assert_eq!(target.width(), 1920);
    assert_eq!(target.height(), 1080);
    assert_eq!(target.format(), wgpu::TextureFormat::Rgba8UnormSrgb);
}

#[tokio::test]
async fn test_create_render_target_4k() {
    let ctx = HeadlessGpuContext::new().await.unwrap();
    let target = ctx.create_render_target(3840, 2160);

    assert_eq!(target.width(), 3840);
    assert_eq!(target.height(), 2160);
}

#[tokio::test]
async fn test_render_target_small() {
    let ctx = HeadlessGpuContext::new().await.unwrap();
    let target = ctx.create_render_target(256, 256);

    assert_eq!(target.width(), 256);
    assert_eq!(target.height(), 256);
}

// ===== Clear Color Rendering Tests =====

#[tokio::test]
async fn test_clear_red() {
    let ctx = HeadlessGpuContext::new().await.unwrap();
    let target = ctx.create_render_target(100, 100);

    // Clear to red
    clear_texture(
        &ctx,
        &target,
        wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );

    let pixels = ctx.read_pixels(&target, 100, 100).await.unwrap();

    // Verify first pixel is red
    assert_eq!(pixels[0], 255); // R
    assert_eq!(pixels[1], 0); // G
    assert_eq!(pixels[2], 0); // B
    assert_eq!(pixels[3], 255); // A
}

#[tokio::test]
async fn test_clear_green() {
    let ctx = HeadlessGpuContext::new().await.unwrap();
    let target = ctx.create_render_target(100, 100);

    clear_texture(
        &ctx,
        &target,
        wgpu::Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
    );

    let pixels = ctx.read_pixels(&target, 100, 100).await.unwrap();

    assert_eq!(pixels[0], 0); // R
    assert_eq!(pixels[1], 255); // G
    assert_eq!(pixels[2], 0); // B
    assert_eq!(pixels[3], 255); // A
}

#[tokio::test]
async fn test_clear_blue() {
    let ctx = HeadlessGpuContext::new().await.unwrap();
    let target = ctx.create_render_target(100, 100);

    clear_texture(
        &ctx,
        &target,
        wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
    );

    let pixels = ctx.read_pixels(&target, 100, 100).await.unwrap();

    assert_eq!(pixels[0], 0); // R
    assert_eq!(pixels[1], 0); // G
    assert_eq!(pixels[2], 255); // B
    assert_eq!(pixels[3], 255); // A
}

#[tokio::test]
async fn test_clear_black() {
    let ctx = HeadlessGpuContext::new().await.unwrap();
    let target = ctx.create_render_target(100, 100);

    clear_texture(&ctx, &target, wgpu::Color::BLACK);

    let pixels = ctx.read_pixels(&target, 100, 100).await.unwrap();

    // All pixels should be black (0,0,0,255)
    for i in (0..pixels.len()).step_by(4) {
        assert_eq!(pixels[i], 0); // R
        assert_eq!(pixels[i + 1], 0); // G
        assert_eq!(pixels[i + 2], 0); // B
        assert_eq!(pixels[i + 3], 255); // A
    }
}

#[tokio::test]
async fn test_clear_white() {
    let ctx = HeadlessGpuContext::new().await.unwrap();
    let target = ctx.create_render_target(50, 50);

    clear_texture(&ctx, &target, wgpu::Color::WHITE);

    let pixels = ctx.read_pixels(&target, 50, 50).await.unwrap();

    // All pixels should be white (255,255,255,255)
    for i in (0..pixels.len()).step_by(4) {
        assert_eq!(pixels[i], 255); // R
        assert_eq!(pixels[i + 1], 255); // G
        assert_eq!(pixels[i + 2], 255); // B
        assert_eq!(pixels[i + 3], 255); // A
    }
}

// ===== Pixel Readback Tests =====

#[tokio::test]
async fn test_read_pixels_size() {
    let ctx = HeadlessGpuContext::new().await.unwrap();

    // Test various sizes
    let sizes = [(64, 64), (128, 128), (256, 256), (512, 512), (1024, 768)];

    for (width, height) in sizes {
        let target = ctx.create_render_target(width, height);
        let pixels = ctx.read_pixels(&target, width, height).await.unwrap();

        assert_eq!(
            pixels.len() as u32,
            width * height * 4,
            "Wrong pixel count for {}x{}",
            width,
            height
        );
    }
}

#[tokio::test]
async fn test_read_pixels_non_square() {
    let ctx = HeadlessGpuContext::new().await.unwrap();

    // Test non-square dimensions
    let target = ctx.create_render_target(1920, 1080);
    let pixels = ctx.read_pixels(&target, 1920, 1080).await.unwrap();

    assert_eq!(pixels.len(), 1920 * 1080 * 4);
}

// ===== Buffer and Pipeline Tests =====

#[tokio::test]
async fn test_create_vertex_buffer() {
    let ctx = HeadlessGpuContext::new().await.unwrap();

    // Test vertex buffer creation for terminal grid (80x24)
    let cols = 80;
    let rows = 24;
    let vertices_per_cell = 6; // 2 triangles
    let vertex_size = std::mem::size_of::<f32>() * 12; // position(2) + tex(2) + fg(4) + bg(4)

    let buffer_size = (cols * rows * vertices_per_cell * vertex_size) as u64;

    let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test_vertex_buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    assert_eq!(buffer.size(), buffer_size);
}

#[tokio::test]
async fn test_create_glyph_atlas_texture() {
    let ctx = HeadlessGpuContext::new().await.unwrap();

    // Test glyph atlas creation (4096x4096 as per SRS)
    let atlas_size = 4096;

    let atlas = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test_glyph_atlas"),
        size: wgpu::Extent3d {
            width: atlas_size,
            height: atlas_size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm, // Grayscale for glyphs
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    assert_eq!(atlas.width(), atlas_size);
    assert_eq!(atlas.height(), atlas_size);
}

#[tokio::test]
async fn test_create_sampler() {
    let ctx = HeadlessGpuContext::new().await.unwrap();

    // Test sampler creation for glyph atlas
    let _sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("test_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    // Sampler created successfully
}

// ===== Backend-Specific Tests =====

#[cfg(target_os = "windows")]
#[tokio::test]
async fn test_dx12_backend() {
    use wgpu::Backends;

    let result = HeadlessGpuContext::new_with_backends(Backends::DX12).await;

    if let Ok(ctx) = result {
        let info = ctx.adapter_info();
        assert_eq!(info.backend, wgpu::Backend::Dx12);
        println!("DirectX 12 backend working: {}", info.name);
    } else {
        println!("DirectX 12 not available, skipping test");
    }
}

#[tokio::test]
async fn test_vulkan_backend() {
    use wgpu::Backends;

    let result = HeadlessGpuContext::new_with_backends(Backends::VULKAN).await;

    if let Ok(ctx) = result {
        let info = ctx.adapter_info();
        assert_eq!(info.backend, wgpu::Backend::Vulkan);
        println!("Vulkan backend working: {}", info.name);
    } else {
        println!("Vulkan not available, skipping test");
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_linux_vulkan_backend() {
    use wgpu::Backends;

    let result = HeadlessGpuContext::new_with_backends(Backends::VULKAN).await;

    if let Ok(ctx) = result {
        let info = ctx.adapter_info();
        assert_eq!(info.backend, wgpu::Backend::Vulkan);
        println!("✓ Linux Vulkan backend working: {}", info.name);
        println!("  Device type: {:?}", info.device_type);
    } else {
        println!("⚠ Vulkan not available on Linux, falling back to OpenGL test");
        test_linux_opengl_fallback().await;
    }
}

#[cfg(target_os = "linux")]
async fn test_linux_opengl_fallback() {
    use wgpu::Backends;

    let ctx = HeadlessGpuContext::new_with_backends(Backends::GL)
        .await
        .expect("OpenGL fallback should be available on Linux");

    let info = ctx.adapter_info();
    assert_eq!(info.backend, wgpu::Backend::Gl);
    println!("✓ Linux OpenGL fallback working: {}", info.name);
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_macos_metal_backend() {
    use wgpu::Backends;

    let ctx = HeadlessGpuContext::new_with_backends(Backends::METAL)
        .await
        .expect("Metal backend should be available on macOS");

    let info = ctx.adapter_info();
    assert_eq!(info.backend, wgpu::Backend::Metal);
    println!("✓ macOS Metal backend working: {}", info.name);
    println!("  Device type: {:?}", info.device_type);
}

// ===== Cross-Platform Backend Selection Test =====

#[tokio::test]
async fn test_platform_backend_selection() {
    use super::backend_selection;

    let backends = backend_selection::select_backend();
    let ctx = HeadlessGpuContext::new_with_backends(backends)
        .await
        .expect("Platform-specific backend should be available");

    let info = ctx.adapter_info();
    println!("✓ Platform backend detected: {:?}", info.backend);
    println!("  Adapter: {}", info.name);
    println!("  Device type: {:?}", info.device_type);

    #[cfg(target_os = "windows")]
    assert_eq!(info.backend, wgpu::Backend::Dx12);

    #[cfg(target_os = "linux")]
    assert!(
        info.backend == wgpu::Backend::Vulkan || info.backend == wgpu::Backend::Gl,
        "Linux should use Vulkan or OpenGL, got {:?}",
        info.backend
    );

    #[cfg(target_os = "macos")]
    assert_eq!(info.backend, wgpu::Backend::Metal);
}

// ===== Helper Functions =====

/// Clear texture to specified color (helper for tests)
fn clear_texture(ctx: &HeadlessGpuContext, texture: &wgpu::Texture, color: wgpu::Color) {
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("clear_encoder"),
        });

    {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }

    ctx.queue.submit(Some(encoder.finish()));
}

// ===== Performance Tests =====

#[tokio::test]
async fn test_render_performance_target_size() {
    use std::time::Instant;

    let ctx = HeadlessGpuContext::new().await.unwrap();
    let target = ctx.create_render_target(1920, 1080);

    // Time 100 clear operations
    let start = Instant::now();
    for _ in 0..100 {
        clear_texture(&ctx, &target, wgpu::Color::BLACK);
    }
    let elapsed = start.elapsed();

    let avg_ms = elapsed.as_secs_f64() * 1000.0 / 100.0;
    println!("Average clear time: {:.2}ms", avg_ms);

    // Should be very fast (<1ms per clear on modern GPU)
    assert!(avg_ms < 10.0, "Clear operation too slow: {:.2}ms", avg_ms);
}

#[tokio::test]
async fn test_texture_creation_performance() {
    use std::time::Instant;

    let ctx = HeadlessGpuContext::new().await.unwrap();

    // Time creating 100 textures
    let start = Instant::now();
    for _ in 0..100 {
        let _ = ctx.create_render_target(512, 512);
    }
    let elapsed = start.elapsed();

    let avg_ms = elapsed.as_secs_f64() * 1000.0 / 100.0;
    println!("Average texture creation: {:.2}ms", avg_ms);

    // Should be fast (<5ms per texture)
    assert!(avg_ms < 10.0, "Texture creation too slow: {:.2}ms", avg_ms);
}
