# 60 FPS Texture Upload Optimization Plan
## Monday Aug 18 Execution Plan - rust-backend-lead

**Date**: 2026-08-15 (Saturday Evening Prep)  
**Target**: Criterion #1 (60 FPS Rendering) - SRS §7.1  
**Timeline**: Monday 9 AM → Thursday EOD (4-day, per ADR-010)  
**Monday Focus**: Texture upload + fontdue integration

---

## Current State Analysis

### ✅ Already Complete
- **Shader pipeline**: text.wgsl (vertex + fragment) - lines 1-56
- **Texture atlas**: 4096×4096 R8Unorm (16 MB) created in `init_text_pipeline()`
- **Bind group + sampler**: Nearest filter for crisp text (renderer.rs:218-257)
- **Render pipeline**: wgpu pipeline with alpha blending (renderer.rs:293-328)
- **Glyph cache**: Guillotine bin-packing + LRU eviction (glyph_cache.rs)
- **Borrow checker pattern**: Split borrows from d1f6abf (process_pty_output)

### ⏸️ Monday Work Needed
- **fontdue integration**: Add dependency, integrate Font::from_bytes/rasterize
- **Texture upload path**: queue.write_texture() with batching
- **Font metrics**: Connect fontdue metrics to glyph_cache (bearing_x, bearing_y, advance)
- **Actual rendering**: Render loop in render() (currently just clear color)

---

## Bottleneck Identification

### Frame Budget Breakdown (SRS §2.1.1)
- **Total**: 16.67ms (60 FPS)
- **GPU render**: 8ms target
- **VSync**: 5ms
- **Headroom**: ~3ms

### Likely Bottlenecks
1. **Texture upload synchronicity** (CRITICAL)
   - Current: No upload path yet
   - Risk: Synchronous `queue.write_texture()` blocks GPU
   - Impact: 1 glyph upload = ~0.1-0.5ms (rough estimate)
   - At 80×24 = 1920 cells, full screen = 192-960ms (WAY over budget)

2. **Per-glyph upload overhead** (MEDIUM)
   - Risk: Uploading 1 glyph at a time has high call overhead
   - Solution: Batch uploads per frame

3. **Glyph cache misses** (LOW - already mitigated)
   - Guillotine allocator handles packing efficiently
   - LRU eviction prevents unbounded growth
   - Risk is low, but monitor cache hit rate

---

## Monday 9 AM → 16:00 Optimization Strategy

### Phase 1: fontdue Integration (2h - 9:00 AM → 11:00 AM)

**Goal**: Rasterize glyphs with fontdue and get font metrics

**Tasks**:
1. Add `fontdue = "0.9"` to `Cargo.toml` (workspace.dependencies)
2. Modify `FontManager::new()` in fonts.rs:
   ```rust
   use fontdue::Font;
   
   pub struct FontManager {
       default_font: Font,  // Change from Vec<u8> to Font
   }
   
   impl FontManager {
       pub fn new() -> Result<Self> {
           // Load Consolas from Windows system fonts as fallback
           let font_path = r"C:\Windows\Fonts\consola.ttf";
           let font_data = std::fs::read(font_path)?;
           let font = Font::from_bytes(font_data, fontdue::FontSettings::default())?;
           Ok(Self { default_font: font })
       }
       
       pub fn rasterize_glyph(&self, ch: char, size: f32) -> (Vec<u8>, fontdue::Metrics) {
           self.default_font.rasterize(ch, size)
       }
   }
   ```

3. Connect metrics to glyph_cache.rs (fix TODOs at lines 97-99):
   ```rust
   let (raster_data, metrics) = font_manager.rasterize_glyph(key.ch, 16.0);
   let info = GlyphInfo {
       tex_x: rect.x as f32 / self.atlas_width as f32,
       tex_y: rect.y as f32 / self.atlas_height as f32,
       tex_width: metrics.width as f32 / self.atlas_width as f32,
       tex_height: metrics.height as f32 / self.atlas_height as f32,
       bearing_x: metrics.xmin,
       bearing_y: metrics.ymin,
       advance: metrics.advance_width as u32,
   };
   ```

**Success Criteria**:
- Compile succeeds with fontdue
- Rasterize test glyph 'A' and verify metrics non-zero
- No panics on font loading

---

### Phase 2: Batched Texture Upload (2h - 11:00 AM → 13:00 PM)

**Goal**: Upload glyphs to atlas WITHOUT blocking GPU (batch uploads)

**Strategy**: Collect dirty glyphs per frame, upload in ONE queue.write_texture() call

**Approach**:
```rust
// In Renderer struct, add:
pub struct Renderer {
    // ... existing fields ...
    
    // Upload staging (batches glyphs for upload)
    pending_uploads: Vec<GlyphUpload>,
}

struct GlyphUpload {
    data: Vec<u8>,       // Rasterized glyph bitmap (R8)
    atlas_x: u32,        // Target position in atlas
    atlas_y: u32,
    width: u32,
    height: u32,
}

impl Renderer {
    /// Queue glyph for upload (don't upload immediately)
    pub fn queue_glyph_upload(&mut self, key: GlyphKey, font_manager: &FontManager) {
        let (raster_data, metrics) = font_manager.rasterize_glyph(key.ch, 16.0);
        
        // Allocate space in atlas via glyph_cache
        let info = self.glyph_cache.insert(key, metrics.width, metrics.height)?;
        
        // Queue upload (don't execute yet)
        self.pending_uploads.push(GlyphUpload {
            data: raster_data,
            atlas_x: (info.tex_x * self.atlas_width as f32) as u32,
            atlas_y: (info.tex_y * self.atlas_height as f32) as u32,
            width: metrics.width,
            height: metrics.height,
        });
    }
    
    /// Execute batched uploads (called once per frame BEFORE render)
    fn flush_glyph_uploads(&mut self) {
        if self.pending_uploads.is_empty() {
            return;
        }
        
        let queue = self.queue.as_ref().unwrap();
        let texture = self.glyph_atlas_texture.as_ref().unwrap();
        
        // Upload each glyph in batch (TODO: Consider staging buffer for multi-upload)
        for upload in self.pending_uploads.drain(..) {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: upload.atlas_x,
                        y: upload.atlas_y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &upload.data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(upload.width),  // R8 = 1 byte per pixel
                    rows_per_image: Some(upload.height),
                },
                wgpu::Extent3d {
                    width: upload.width,
                    height: upload.height,
                    depth_or_array_layers: 1,
                },
            );
        }
        
        tracing::debug!("Flushed {} glyph uploads to atlas", self.pending_uploads.len());
    }
}
```

**Call site in render()**:
```rust
pub fn render(&mut self, window: &Window, perf: &mut PerformanceMonitor) -> Result<()> {
    // Process PTY (split borrows pattern from d1f6abf)
    Self::process_pty_output(...)?;
    
    // Flush glyph uploads BEFORE acquiring surface texture
    self.flush_glyph_uploads();
    perf.mark("flush_uploads");
    
    // Acquire surface and render...
    let output = surface.get_current_texture()?;
    // ... rest of render logic
}
```

**Success Criteria**:
- Batch 10 glyphs, verify only 10 queue.write_texture() calls (not 10×N)
- No GPU stalls (use GPU profiler if time permits)
- Frame time stays under 16ms for 80×24 screen

---

### Phase 3: Actual Rendering (2h - 13:00 PM → 15:00 PM)

**Goal**: Render terminal grid using uploaded glyphs

**Approach**: Build vertex buffer from dirty cells, draw with pipeline

```rust
// In render() method, replace TODO at line 415-419
{
    let mut render_pass = encoder.begin_render_pass(...);
    
    // Set pipeline and bind group
    render_pass.set_pipeline(self.render_pipeline.as_ref().unwrap());
    render_pass.set_bind_group(0, self.atlas_bind_group.as_ref().unwrap(), &[]);
    
    // Build vertex buffer from dirty cells in terminal_grid
    let vertices = self.build_vertex_buffer(&self.terminal_grid)?;
    
    if !vertices.is_empty() {
        // Create dynamic vertex buffer
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..vertices.len() as u32, 0..1);
    }
    
    perf.mark("render_pass");
}

fn build_vertex_buffer(&mut self, grid: &TerminalGrid) -> Result<Vec<Vertex>> {
    let mut vertices = Vec::new();
    
    // Iterate dirty cells (TODO: Add dirty tracking to TerminalGrid)
    for row in 0..grid.rows() {
        for col in 0..grid.cols() {
            let cell = grid.get_cell(row, col);
            
            // Lookup glyph in cache (or queue upload if miss)
            let key = GlyphKey::new(cell.ch, &cell.style);
            let info = if let Some(info) = self.glyph_cache.lookup(key) {
                info
            } else {
                // Cache miss - queue upload, skip rendering this frame
                self.queue_glyph_upload(key, &self.font_manager)?;
                continue;
            };
            
            // Calculate screen position (NDC: -1 to +1)
            let screen_x = (col as f32 / grid.cols() as f32) * 2.0 - 1.0;
            let screen_y = (row as f32 / grid.rows() as f32) * 2.0 - 1.0;
            
            // Build 2 triangles (6 vertices) for glyph quad
            vertices.extend_from_slice(&self.build_glyph_quad(
                screen_x, screen_y,
                info,
                cell.style.fg_color,
            ));
        }
    }
    
    Ok(vertices)
}
```

**Success Criteria**:
- Render "Hello, MONOTERMINAL!" on screen
- Text is crisp (nearest filter works)
- No visual artifacts or texture corruption

---

### Phase 4: FPS Testing & Checkpoint (1h - 15:00 PM → 16:00 PM)

**Goal**: Verify 60 FPS with scrolling workload

**Test Workload**:
```powershell
# Run master daemon with FPS counter
./target/release/monoterminal-master --fps-counter

# In terminal, run rapid scrolling test:
# - cat large file (10,000 lines)
# - Observe FPS counter during scroll
```

**Metrics to Collect**:
- p50 FPS (target: ≥ 60)
- p95 FPS (target: ≥ 58)
- Frame drops during scroll (target: 0)
- GPU render time (target: < 8ms per frame)

**If FPS < 60**:
1. Check GPU profiler for bottleneck (upload vs render vs present)
2. Reduce glyph uploads per frame (throttle to 100 glyphs/frame?)
3. Add dirty cell tracking to skip unchanged cells

**16:00 Checkpoint**: Report to eng-director:
- FPS metrics (p50, p95)
- Bottlenecks identified
- Monday evening work plan (shaders if ahead, or continue FPS tuning)

---

## Risks & Mitigations

### Risk 1: fontdue dependency conflicts
- **Mitigation**: Use fontdue 0.9 (stable, widely used)
- **Fallback**: If conflicts, use rusttype or ab_glyph

### Risk 2: Texture upload still too slow even with batching
- **Mitigation**: Add staging buffer (wgpu::BufferUsages::COPY_SRC)
- **Mitigation**: Async texture upload (queue.write_buffer_with then copy)
- **Fallback**: Reduce atlas size to 2048×2048 (4 MB) if 16 MB is bottleneck

### Risk 3: Glyph cache thrashing (high miss rate)
- **Mitigation**: Increase max_cache_size from 2048 to 4096 glyphs
- **Mitigation**: Pre-warm cache with ASCII 32-126 on startup

### Risk 4: Split borrow pattern breaks with new fields
- **Mitigation**: Use same pattern from d1f6abf (static methods with explicit borrows)
- **Mitigation**: Keep surface/device/queue separate from mutable state

---

## Monday Evening Contingency (if ahead of schedule)

**If 16:00 checkpoint shows p50 ≥ 60 FPS:**
- ✅ Move to shader optimization (if shaders need tuning)
- ✅ Add dirty cell tracking to terminal_grid (skip unchanged cells)
- ✅ Pre-warm glyph cache with ASCII printables

**If 16:00 checkpoint shows p50 < 60 FPS:**
- 🔧 Debug with GPU profiler
- 🔧 Add performance instrumentation (perf.mark at each stage)
- 🔧 Throttle uploads (max 100 glyphs per frame)

---

## Success Definition

**Monday EOD (20:00) Target**:
- [x] fontdue integrated, Consolas loaded
- [x] Texture upload batching working
- [x] Text rendering visible on screen
- [x] FPS ≥ 58 (p95) during scrolling test
- [x] No GPU stalls or frame drops

**If all checkboxes hit**: Monday = SUCCESS, move to Tuesday work  
**If FPS < 58**: Continue tuning Tuesday AM, notify eng-director of slip

---

## References

- **SRS §7.1**: 60 FPS acceptance criteria (p50 ≥ 60, p95 ≥ 58)
- **SRS §2.1.1**: Frame budget (16.67ms total, 8ms GPU, 5ms VSync)
- **ADR-010**: 4-day timeline (Mon-Thu) with 70% confidence
- **Commit d1f6abf**: Split borrow pattern for mutable + immutable borrows
- **renderer.rs**: Existing text pipeline (lines 170-340)
- **glyph_cache.rs**: Guillotine bin-packing + LRU (lines 1-286)
- **text.wgsl**: Complete shader (lines 1-56)

---

**Plan Status**: DRAFT - Saturday Evening Prep  
**Next Action**: Wait for Monday 9 AM "environment ready" signal from devops-lead  
**Owner**: rust-backend-lead
