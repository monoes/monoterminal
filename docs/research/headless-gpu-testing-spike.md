# Headless GPU Testing Research Spike

**Date:** 2026-08-18  
**Engineer:** gpu-rendering-engineer  
**Task:** Implement headless GPU test infrastructure for Criterion #6 coverage improvement

## Problem Statement

Current test coverage: **41.03%** (target: ≥70%)  
Gap: UI/renderer modules have **ZERO coverage** (507 uncovered lines)  
Root cause: `cargo-tarpaulin` cannot initialize GPU context for wgpu/egui tests

## Research: Headless wgpu Backend Options

### Option 1: wgpu NULL Backend (Empty)
```rust
wgpu::Instance::new(wgpu::InstanceDescriptor {
    backends: wgpu::Backends::EMPTY,
    ..Default::default()
})
```
**Pros:**
- No GPU required
- Fast, always works
- Good for API contract testing

**Cons:**
- No actual rendering
- Can't validate visual output
- Doesn't exercise real GPU code paths
- **Won't catch GPU-specific bugs**

**Verdict:** ❌ Not suitable for real coverage

---

### Option 2: Software Rendering (SwiftShader/llvmpipe)
```rust
// Request GL backend with software rendering fallback
wgpu::Instance::new(wgpu::InstanceDescriptor {
    backends: wgpu::Backends::GL,
    ..Default::default()
})
```
**Pros:**
- Real rendering without display
- Works in CI environments
- Can validate pixel output
- Cross-platform

**Cons:**
- Requires software rasterizer installation (swiftshader, mesa llvmpipe)
- Slower than hardware rendering
- Different code path than production (GL vs DX12/Vulkan)
- Setup complexity on Windows

**Verdict:** ⚠️ Possible but complex setup

---

### Option 3: Offscreen Rendering with Real GPU ✅ SELECTED
```rust
// Use real GPU adapter, render to texture instead of surface
let adapter = instance
    .request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None, // ← No surface = headless
        force_fallback_adapter: false,
    })
    .await
    .expect("Failed to find adapter");

let (device, queue) = adapter
    .request_device(&wgpu::DeviceDescriptor::default(), None)
    .await
    .expect("Failed to create device");

// Render to texture
let texture = device.create_texture(&wgpu::TextureDescriptor {
    size: wgpu::Extent3d { width: 1920, height: 1080, depth_or_array_layers: 1 },
    format: wgpu::TextureFormat::Rgba8UnormSrgb,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    ..Default::default()
});
```

**Pros:**
- ✅ Real GPU execution (same code path as production)
- ✅ No display required (`compatible_surface: None`)
- ✅ Can validate rendering output (read pixels from texture)
- ✅ Works with DirectX 12 (Windows), Vulkan (Linux), Metal (macOS)
- ✅ Fast (hardware accelerated)
- ✅ Simple integration - no external dependencies

**Cons:**
- Requires GPU on test machine (most CI has GPU or software fallback)
- May fall back to software if no GPU (WARP on Windows, llvmpipe on Linux)

**Verdict:** ✅ **BEST OPTION** - Real rendering, real GPU, no display

---

## Implementation Plan

### Phase 1: Test Infrastructure (2-4 hours)
Create `crates/master/src/ui/test_support.rs`:

```rust
/// Headless GPU context for testing
pub struct HeadlessGpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
}

impl HeadlessGpuContext {
    /// Create headless GPU context (no window/surface)
    pub async fn new() -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(), // Try all backends
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None, // ← Headless
                force_fallback_adapter: false,
            })
            .await
            .expect("No GPU adapter found");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .expect("Failed to create device");

        Self { device, queue, adapter }
    }

    /// Create offscreen render target
    pub fn create_render_target(&self, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test_render_target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT 
                 | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// Read pixels from texture (for validation)
    pub async fn read_pixels(&self, texture: &wgpu::Texture) -> Vec<u8> {
        // Implementation: copy texture to buffer, map buffer, read bytes
        todo!()
    }
}
```

### Phase 2: UI Module Tests (6-8 hours)
Target modules and coverage gains:

1. **renderer.rs** (+333 lines)
   - Test: GPU initialization
   - Test: Pipeline creation
   - Test: Vertex buffer allocation
   - Test: Texture atlas creation
   - Test: Bind group setup

2. **fonts.rs** (+41 lines)
   - Test: Font loading (Consolas fallback)
   - Test: Glyph rasterization
   - Test: Metrics calculation

3. **layout.rs** (+39 lines)
   - Test: Cell grid calculations
   - Test: Viewport sizing

4. **performance.rs** (+40 lines)
   - Test: FPS tracking
   - Test: Budget monitoring

5. **window.rs** (+54 lines)
   - Test: Window creation helpers
   - Test: Event handling

### Phase 3: Integration Tests (4-6 hours)
Create `crates/master/tests/ui_renderer_headless.rs`:

```rust
#[tokio::test]
async fn test_renderer_initialization_headless() {
    let ctx = HeadlessGpuContext::new().await;
    
    // Test actual renderer initialization
    let renderer = Renderer::new_headless(&ctx).await.unwrap();
    
    assert!(renderer.device.is_some());
    assert!(renderer.pipeline.is_some());
}

#[tokio::test]
async fn test_render_single_frame() {
    let ctx = HeadlessGpuContext::new().await;
    let mut renderer = Renderer::new_headless(&ctx).await.unwrap();
    
    // Render frame to offscreen texture
    let target = ctx.create_render_target(1920, 1080);
    renderer.render_to_texture(&target).unwrap();
    
    // Validate (can check for non-zero pixels, specific colors, etc.)
    let pixels = ctx.read_pixels(&target).await;
    assert!(!pixels.is_empty());
}
```

### Phase 4: main.rs Coverage (2 hours)
Quick win: +92 lines

```rust
#[test]
fn test_main_startup_shutdown() {
    // Test config loading
    // Test logger initialization
    // Test graceful shutdown path
}
```

## Expected Coverage Improvements

| Module | Current | Lines | Target | Improvement |
|--------|---------|-------|--------|-------------|
| renderer.rs | 0% | 333 | 80% | +266 lines |
| fonts.rs | 0% | 41 | 90% | +37 lines |
| layout.rs | 0% | 39 | 90% | +35 lines |
| performance.rs | 0% | 40 | 90% | +36 lines |
| window.rs | 0% | 54 | 70% | +38 lines |
| main.rs | 0% | 92 | 60% | +55 lines |
| **TOTAL** | **41.03%** | **+467** | **59.4%** | **+18.37%** |

**Quick wins target:** 41% → 56-61% (✅ achievable)  
**Full implementation:** 41% → 66-76% (stretch goal)

## Implementation Timeline

- ✅ Research spike: 1 hour (DONE)
- 🔄 Test infrastructure: 2-4 hours (NEXT)
- 🔄 UI module tests: 6-8 hours
- 🔄 Integration tests: 4-6 hours
- 🔄 main.rs coverage: 2 hours
- 🔄 Coverage validation: 1 hour

**Total: 16-22 hours (2-3 days)**

## Decision: Proceed with Option 3 (Offscreen Rendering)

**Rationale:**
- Real GPU execution = real coverage
- No external dependencies (pure wgpu)
- Cross-platform (DX12/Vulkan/Metal)
- Can validate actual rendering output
- Supports CI environments (GPU or software fallback)

**Next step:** Implement `HeadlessGpuContext` in `src/ui/test_support.rs`
