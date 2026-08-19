# GPU Shader Review - text.wgsl

**Reviewer:** rust-backend-lead  
**Date:** 2026-08-16  
**File:** `crates/master/src/ui/shaders/text.wgsl`  
**Status:** Existing shader found, reviewing for Criterion #1 (60 FPS rendering)

---

## Current Implementation Analysis

### ✅ Strengths

1. **Clean, minimal WGSL code** (56 lines)
2. **Correct vertex transformation** (NDC → clip space with Z=0, W=1)
3. **Proper texture sampling** (R8Unorm atlas format)
4. **Pass-through architecture** (minimal vertex processing)
5. **Alpha modulation approach** (`color.a * glyph_alpha`)

### ⚠️ Potential Issues for Terminal Rendering

#### Issue #1: Background Color Support

**Current approach:**
```wgsl
// Line 54: Returns transparent where glyph_alpha = 0
return vec4<f32>(in.color.rgb, in.color.a * glyph_alpha);
```

**Problem:**
- When `glyph_alpha = 0` (no glyph pixels), output alpha = 0 (transparent)
- Terminal cells typically have **per-cell background colors** (not just "transparent")
- Example: Green text on blue background, red text on yellow background (ANSI colors)

**Impact:**
- Works IF backgrounds are rendered separately (2-pass: backgrounds first, then glyphs)
- Doesn't work for single-pass rendering with per-cell bg colors

**Terminal use cases requiring bg colors:**
1. ANSI escape sequences: `\e[42;31m` (red text on green background)
2. Inverse video: `\e[7m` (swap fg/bg)
3. Selected text highlighting
4. Cursor background color

#### Issue #2: Sampler Filter Mode

**Current approach:**
```wgsl
// Line 26-28: Nearest filter
var atlas_sampler: sampler;  // Comment says "nearest filter"
```

**Consideration:**
- **Nearest filter**: Crisp text, but aliased edges (jagged diagonals)
- **Linear filter**: Smooth antialiased text, slightly blurrier

**Best practice for terminals:**
- Use **linear filter** for better readability at small font sizes
- GPU-accelerated antialiasing with minimal performance cost
- Most modern terminal emulators use linear sampling

---

## Proposed Enhancements

### Enhancement #1: Add Background Color Support

**Option A: Modify vertex input to include bg_color**

```wgsl
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) fg_color: vec4<f32>,  // Renamed from 'color'
    @location(3) bg_color: vec4<f32>,  // NEW: background color
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,  // NEW: pass to fragment
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let glyph_alpha = textureSample(glyph_atlas, atlas_sampler, in.tex_coord).r;
    
    // Blend foreground and background based on glyph coverage
    // alpha=0.0 → bg_color, alpha=1.0 → fg_color
    return mix(in.bg_color, in.fg_color, glyph_alpha);
}
```

**Benefits:**
- Single-pass rendering (no separate background pass)
- Per-cell background colors (full ANSI support)
- Simpler CPU-side logic (one draw call for all cells)

**Cost:**
- +16 bytes per vertex (vec4<f32> bg_color)
- Total vertex size: 48 bytes (was 32 bytes)
- For 80×24 grid: +23KB vertex buffer (negligible)

**Option B: Keep 2-pass rendering**

1. **Pass 1:** Render background quads (solid color, no texture)
2. **Pass 2:** Render glyphs with current shader (transparent bg)

**Benefits:**
- Keeps current shader unchanged
- Smaller vertex buffer

**Cost:**
- 2 draw calls per frame (vs 1 draw call)
- More complex CPU logic
- Potential overdraw (background pixels overwritten by glyphs)

**Recommendation:** **Option A** (single-pass with bg_color)
- Simpler architecture
- Better performance (1 draw call vs 2)
- 23KB vertex buffer increase is negligible

---

### Enhancement #2: Linear Sampler for Antialiasing

**Current:**
```wgsl
// Nearest filter (crisp but aliased)
```

**Proposed:**
```wgsl
// Linear filter for smooth antialiasing
// Configured in renderer.rs sampler creation:
let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
    mag_filter: wgpu::FilterMode::Linear,  // Changed from Nearest
    min_filter: wgpu::FilterMode::Linear,  // Changed from Nearest
    ...
});
```

**Impact:**
- No shader code changes needed (sampler config is CPU-side)
- Smoother text rendering
- Negligible GPU cost (modern GPUs optimize linear sampling)

---

## Integration with Existing Codebase

### Vertex Buffer Generation (renderer.rs)

If we adopt **Option A (bg_color support)**, update vertex struct:

```rust
// crates/master/src/ui/renderer.rs
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphVertex {
    position: [f32; 2],     // @location(0)
    tex_coords: [f32; 2],   // @location(1)
    fg_color: [f32; 4],     // @location(2)
    bg_color: [f32; 4],     // @location(3)  // NEW
}
```

### Glyph Cache Integration

No changes needed - `glyph_cache.rs` already provides:
- `GlyphInfo.tex_x, tex_y` → maps to `tex_coord`
- Atlas dimensions (4096×4096)

### Terminal Grid Integration

`terminal_grid.rs` provides:
- Cell foreground color → `fg_color`
- Cell background color → `bg_color` (NEW usage)
- Dirty cell tracking → only render changed cells

---

## Performance Analysis

### Frame Budget Compliance (SRS §2.1.1)

**Target:** 8ms GPU render time

**Current shader estimate:**
- Vertex processing: <0.5ms (simple pass-through)
- Texture sampling: ~3-5ms (4096×4096 atlas, linear filter)
- Fragment blending: ~2-3ms (mix() operation, alpha blend)
- **Total: ~6-8ms** ✅ Within budget

**With bg_color enhancement:**
- Vertex processing: +0.1ms (larger vertex buffer)
- Fragment blending: same (mix() already efficient)
- **Total: ~6-8ms** ✅ Still within budget

### Bottleneck Mitigation

**Potential bottleneck:** Texture sampling (largest contributor)

**Mitigation strategies:**
1. **Dirty tracking** (terminal_grid.rs): Only render changed cells (~10% per frame)
2. **Glyph caching** (glyph_cache.rs): Reuse atlas positions, no per-frame uploads
3. **Batched draw call**: All glyphs in single draw (minimize CPU overhead)

**Future optimizations** (if needed):
- Mipmaps for atlas texture (improve GPU cache coherency)
- Instanced rendering (reduce vertex data)
- Compressed texture format (BC4 for R8, GPU decompression)

---

## Questions for gpu-rendering-engineer

1. **Background color support:**
   - Is Option A (add bg_color to vertex) acceptable?
   - OR should we stick with 2-pass rendering (Option B)?
   - Is there a reason the current shader uses alpha modulation vs mix()?

2. **Sampler filter mode:**
   - Nearest filter chosen for "crisp text" - is aliasing acceptable?
   - Can we switch to linear filter for antialiasing?
   - Any platform-specific considerations (DirectX 12 on Windows)?

3. **Integration timeline:**
   - Current shader: ready to compile and test?
   - If enhancements needed: who implements (you or me)?
   - Review/approval process before integration?

4. **Testing approach:**
   - RenderDoc profiling available on Windows?
   - Frame timing validation strategy?
   - How to verify 60 FPS target before Phase 1 gate?

---

## Recommendation

**For Criterion #1 (60 FPS rendering):**

1. **Option 1 (Quick path):** Use existing shader as-is
   - Implement 2-pass rendering (backgrounds first, glyphs second)
   - Verify 60 FPS with current implementation
   - Defer bg_color enhancement to Phase 2

2. **Option 2 (Quality path):** Enhance shader before integration
   - Add bg_color support (Option A)
   - Switch to linear sampler
   - Single-pass rendering
   - Slightly longer implementation (~2h more) but better architecture

**My recommendation:** **Option 2** (enhance now)
- Cleaner architecture (single-pass)
- Full ANSI color support (required for terminal)
- Only +2h implementation time (within 8-12h budget)
- Avoids technical debt in Phase 1

**Next step:** Get gpu-rendering-engineer's input before proceeding.

---

**Status:** Awaiting feedback from gpu-rendering-engineer + protoc fix for compilation
