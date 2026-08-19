# GPU Rendering Pipeline Implementation Plan

**Owner:** rust-backend-lead  
**Target:** Criterion #1 - 60 FPS Rendering  
**Status:** Planning (awaiting environment fix + timeline decision)  
**Date:** 2026-08-16

---

## Objective

Implement the complete GPU rendering pipeline for terminal text rendering in `renderer.rs`, achieving 60 FPS (16.67ms frame budget) with 8ms GPU render time per SRS §2.1.1.

---

## Current State Assessment

### ✅ Foundation Complete (75%)

```
crates/master/src/ui/
├── fonts.rs         ✅ FontManager with fontdue, Consolas fallback
├── glyph_cache.rs   ✅ Guillotine bin-packing, LRU eviction, 4096×4096 atlas
├── terminal_grid.rs ✅ Grid data structure with dirty tracking
├── vt_parser.rs     ✅ VT sequence parsing
├── window.rs        ✅ winit event loop, 60 FPS target
├── performance.rs   ✅ Frame timing monitor
└── renderer.rs      🚧 INCOMPLETE - GPU pipeline needed
```

### ❌ Missing Components (25% - CRITICAL PATH)

From `renderer.rs` lines 221-225:

```rust
// TODO Day 2: Actual terminal rendering
// - Iterate dirty cells in terminal_grid
// - Lookup glyphs in glyph_cache
// - Draw to texture
// - egui UI overlay
```

---

## Implementation Plan

### Phase 1: Shader Implementation (4-6 hours)

#### 1.1 WGSL Vertex Shader

**File:** Create `crates/master/src/ui/shaders/text.wgsl`

```wgsl
// Vertex shader for textured glyph quads
struct VertexInput {
    @location(0) position: vec2<f32>,    // Screen position (NDC)
    @location(1) tex_coords: vec2<f32>,  // Texture coordinates (0-1)
    @location(2) fg_color: vec4<f32>,    // Foreground color (RGBA)
    @location(3) bg_color: vec4<f32>,    // Background color (RGBA)
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.tex_coords = in.tex_coords;
    out.fg_color = in.fg_color;
    out.bg_color = in.bg_color;
    return out;
}
```

#### 1.2 WGSL Fragment Shader

```wgsl
@group(0) @binding(0) var atlas_texture: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample glyph from atlas (grayscale)
    let alpha = textureSample(atlas_texture, atlas_sampler, in.tex_coords).r;
    
    // Mix foreground/background based on alpha
    return mix(in.bg_color, in.fg_color, alpha);
}
```

**Budget:** 2 hours (shader code + syntax validation)

#### 1.3 Vertex Format Definition

In `renderer.rs`, add:

```rust
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphVertex {
    position: [f32; 2],    // NDC coordinates (-1.0 to 1.0)
    tex_coords: [f32; 2],  // Texture coordinates (0.0 to 1.0)
    fg_color: [f32; 4],    // RGBA (0.0 to 1.0)
    bg_color: [f32; 4],    // RGBA (0.0 to 1.0)
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
```

**Budget:** 1 hour

---

### Phase 2: Pipeline Setup (2-3 hours)

#### 2.1 Texture Atlas Creation

In `renderer.rs`, add field:

```rust
struct Renderer {
    // ... existing fields ...
    
    // GPU rendering pipeline
    text_pipeline: Option<wgpu::RenderPipeline>,
    atlas_texture: Option<wgpu::Texture>,
    atlas_texture_view: Option<wgpu::TextureView>,
    atlas_sampler: Option<wgpu::Sampler>,
    bind_group: Option<wgpu::BindGroup>,
    vertex_buffer: Option<wgpu::Buffer>,
}
```

Implement `init_text_pipeline()`:

```rust
pub fn init_text_pipeline(&mut self) -> Result<()> {
    let device = self.device.as_ref().context("Device not initialized")?;
    
    // 1. Create texture atlas (4096×4096, R8Unorm)
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
        format: wgpu::TextureFormat::R8Unorm, // Grayscale
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    
    let atlas_texture_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
    
    // 2. Create sampler (linear filtering for smooth text)
    let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Glyph Atlas Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    
    // ... (continue with bind group layout, pipeline creation)
    
    Ok(())
}
```

**Budget:** 1.5 hours

#### 2.2 Render Pipeline Creation

```rust
// 3. Load shader
let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("Text Shader"),
    source: wgpu::ShaderSource::Wgsl(include_str!("shaders/text.wgsl").into()),
});

// 4. Create bind group layout
let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    label: Some("Text Bind Group Layout"),
    entries: &[
        // Texture
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
        // Sampler
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
        module: &shader,
        entry_point: "vs_main",
        buffers: &[GlyphVertex::desc()],
    },
    fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: "fs_main",
        targets: &[Some(wgpu::ColorTargetState {
            format: self.surface_config.as_ref().unwrap().format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })],
    }),
    primitive: wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        ..Default::default()
    },
    depth_stencil: None,
    multisample: wgpu::MultisampleState::default(),
    multiview: None,
});
```

**Budget:** 1.5 hours

---

### Phase 3: Integration (2-3 hours)

#### 3.1 Vertex Buffer Generation

Add method to `Renderer`:

```rust
fn build_vertex_buffer(&mut self, dirty_cells: &[(u16, u16)]) -> Vec<GlyphVertex> {
    let mut vertices = Vec::with_capacity(dirty_cells.len() * 6); // 2 triangles per cell
    
    for &(row, col) in dirty_cells {
        let cell = self.terminal_grid.get_cell(row, col);
        let key = GlyphKey::new(cell.ch, &cell.style);
        
        // Lookup or insert glyph in cache
        let glyph_info = if let Some(info) = self.glyph_cache.lookup(key) {
            info
        } else {
            // Rasterize and cache
            let rasterized = self.font_manager.rasterize_glyph(
                cell.ch, 
                cell.style.bold, 
                cell.style.italic
            )?;
            
            // Upload to atlas texture (queue.write_texture)
            self.upload_glyph_to_atlas(&rasterized)?;
            
            self.glyph_cache.insert(key, rasterized.width, rasterized.height)
                .context("Failed to insert glyph into cache")?
        };
        
        // Calculate NDC coordinates for this cell
        let cell_width = 2.0 / self.terminal_grid.cols() as f32;
        let cell_height = 2.0 / self.terminal_grid.rows() as f32;
        let x = -1.0 + col as f32 * cell_width;
        let y = 1.0 - row as f32 * cell_height;
        
        // Build quad (2 triangles = 6 vertices)
        let fg = cell.style.fg_color.to_rgba_f32();
        let bg = cell.style.bg_color.to_rgba_f32();
        
        vertices.extend_from_slice(&[
            // Triangle 1
            GlyphVertex { position: [x, y - cell_height], tex_coords: [glyph_info.tex_x, glyph_info.tex_y + glyph_info.tex_height], fg_color: fg, bg_color: bg },
            GlyphVertex { position: [x + cell_width, y - cell_height], tex_coords: [glyph_info.tex_x + glyph_info.tex_width, glyph_info.tex_y + glyph_info.tex_height], fg_color: fg, bg_color: bg },
            GlyphVertex { position: [x, y], tex_coords: [glyph_info.tex_x, glyph_info.tex_y], fg_color: fg, bg_color: bg },
            
            // Triangle 2
            GlyphVertex { position: [x, y], tex_coords: [glyph_info.tex_x, glyph_info.tex_y], fg_color: fg, bg_color: bg },
            GlyphVertex { position: [x + cell_width, y - cell_height], tex_coords: [glyph_info.tex_x + glyph_info.tex_width, glyph_info.tex_y + glyph_info.tex_height], fg_color: fg, bg_color: bg },
            GlyphVertex { position: [x + cell_width, y], tex_coords: [glyph_info.tex_x + glyph_info.tex_width, glyph_info.tex_y], fg_color: fg, bg_color: bg },
        ]);
    }
    
    Ok(vertices)
}
```

**Budget:** 2 hours

#### 3.2 Update render() Method

Replace TODO section in `render()`:

```rust
pub fn render(&mut self, window: &Window, perf: &mut PerformanceMonitor) -> Result<()> {
    // ... (existing PTY processing + texture acquisition) ...
    
    {
        // Begin render pass
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0, g: 0.0, b: 0.0, a: 1.0, // Black background
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        
        perf.mark("render_pass_begin");
        
        // Set pipeline and bind group
        render_pass.set_pipeline(self.text_pipeline.as_ref().unwrap());
        render_pass.set_bind_group(0, self.bind_group.as_ref().unwrap(), &[]);
        
        // Get dirty cells (optimization: only render changed cells)
        let dirty_cells = self.terminal_grid.get_dirty_cells();
        
        if !dirty_cells.is_empty() {
            // Build vertex buffer from dirty cells
            let vertices = self.build_vertex_buffer(&dirty_cells)?;
            
            // Upload vertices to GPU
            let vertex_buffer_data = bytemuck::cast_slice(&vertices);
            queue.write_buffer(
                self.vertex_buffer.as_ref().unwrap(),
                0,
                vertex_buffer_data,
            );
            
            // Draw
            render_pass.set_vertex_buffer(0, self.vertex_buffer.as_ref().unwrap().slice(..));
            render_pass.draw(0..vertices.len() as u32, 0..1);
        }
        
        perf.mark("render_draw");
    }
    
    // ... (existing submit + present) ...
}
```

**Budget:** 1 hour

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_vertex_buffer_generation() {
        // Test quad generation for sample cells
    }
    
    #[test]
    fn test_glyph_cache_integration() {
        // Test glyph lookup/insert workflow
    }
}
```

### Integration Test

```bash
# Run UI test with mock PTY data
cargo run --example ui_test

# Expected: Window opens, shows "Hello, World!" at 60 FPS
```

### Performance Validation

Use `PerformanceMonitor` to verify budgets:

```
[INFO] Frame time: 14.2ms ✅
  - PTY read: 1.8ms
  - Dirty tracking: 0.4ms
  - Glyph lookup: 0.9ms
  - GPU render: 6.3ms
  - VSync: 4.8ms
```

---

## Dependencies

### New Cargo Dependency

```toml
# crates/master/Cargo.toml
bytemuck = { version = "1.14", features = ["derive"] }
```

### Existing Dependencies (Already in Workspace)

- `wgpu = "0.20"`
- `winit = "0.30"`
- `fontdue` (via fonts.rs)

---

## Risk Mitigation

### Risk 1: Shader Compilation Errors
- **Mitigation:** Use `wgpu`'s built-in shader validation
- **Fallback:** Test shaders with `naga` CLI before integration

### Risk 2: Performance Budget Violation (>8ms GPU render)
- **Mitigation:** Profile with RenderDoc to identify bottlenecks
- **Optimization:** Batch draw calls, use instancing if needed

### Risk 3: Glyph Atlas Overflow
- **Mitigation:** Guillotine allocator already handles LRU eviction
- **Validation:** Log atlas utilization, add test for 2048+ unique glyphs

---

## Success Criteria

- ✅ Window renders terminal text using wgpu
- ✅ 60 FPS sustained (16.67ms frame budget)
- ✅ GPU render time <8ms (per SRS §2.1.1)
- ✅ Mock PTY data displays correctly
- ✅ All tests pass (`cargo test ui::`)
- ✅ No clippy warnings (`cargo clippy -D warnings`)
- ✅ Code formatted (`cargo fmt`)

---

## Timeline Estimate

| Phase | Description | Hours | Confidence |
|-------|-------------|-------|------------|
| 1 | Shader implementation | 4-6 | High |
| 2 | Pipeline setup | 2-3 | Medium |
| 3 | Integration | 2-3 | Medium |
| **Total** | **End-to-end** | **8-12** | **85%** |

**Assumes:** Environment fixed, no major blockers, cargo toolchain operational.

**Delivery:**
- **Option A (recommended):** Monday AM (85% confidence, quality-first)
- **Option B (compressed):** Friday EOD (60% confidence, risky)

---

## Next Steps (Post-Approval)

1. **Environment Fix:** Wait for devops-lead to resolve cargo PATH issue
2. **Timeline Decision:** Wait for eng-director's Option A vs B decision
3. **Implementation:** Execute phases 1-3 sequentially
4. **Validation:** Run tests, verify 60 FPS target
5. **Commit:** Create commit with shader files + renderer.rs changes
6. **Handoff:** Notify eng-director with commit hash + verification proof

---

**Status:** Ready to Execute (Blocked on Environment + Timeline Decision)  
**Last Updated:** 2026-08-16  
**Owner:** rust-backend-lead
