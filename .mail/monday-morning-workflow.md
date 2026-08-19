# Monday Morning Execution Workflow
## rust-backend-lead - Criterion #1 (60 FPS)

**Date**: 2026-08-18 (Monday)  
**Owner**: rust-backend-lead  
**Timeline**: 8:00 AM (wait) → 9:00 AM (start) → 16:00 (checkpoint) → 20:00 (EOD)

---

## ⏸️ Phase 0: Wait for Environment (8:00-9:00 AM)

**DO NOT START until "environment ready" signal**

**Waiting for**: devops-lead Rust toolchain setup (30-60 min)

**Actions while waiting**:
- Review .mail/60fps-texture-upload-plan.md
- Review renderer.rs current state (lines 170-340, 365-431)
- Review glyph_cache.rs implementation
- No code changes, no cargo commands

**Trigger to proceed**: Message from devops-lead or eng-director with "environment ready"

---

## ✅ Phase 1: Verify monomind-bridge (9:00-9:15 AM)

**Goal**: Ensure monomind-bridge compiles and tests pass

```powershell
# Build verification
cargo build -p monoterminal-monomind-bridge

# Test verification (38 unit tests expected)
cargo test -p monoterminal-monomind-bridge --lib

# Expected output:
# running 38 tests
# test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured
```

**If build fails**: Report immediately to eng-director (blocks E2E tests)  
**If tests fail**: Report immediately to eng-director  
**If success**: Proceed to Phase 2

---

## 🚧 Phase 2: fontdue Integration (9:15-11:15 AM - 2h)

### Step 1: Add fontdue dependency
```powershell
# Edit Cargo.toml (workspace.dependencies section)
# Add: fontdue = "0.9"
```

### Step 2: Modify FontManager (crates/master/src/ui/fonts.rs)
- Change `default_font: Vec<u8>` → `default_font: Font`
- Add `use fontdue::Font;`
- Implement `rasterize_glyph(ch: char, size: f32) -> (Vec<u8>, Metrics)`
- Load Consolas from `C:\Windows\Fonts\consola.ttf` as fallback

### Step 3: Connect fontdue metrics to glyph_cache.rs
- Fix TODOs at lines 97-99 (bearing_x, bearing_y, advance)
- Use fontdue::Metrics from rasterization

### Step 4: Verify compilation
```powershell
cargo build -p monoterminal-master
```

**Checkpoint**: 11:15 AM - fontdue compiles, no errors

---

## 🚧 Phase 3: Texture Upload Batching (11:15-13:15 PM - 2h)

### Step 1: Add upload staging to Renderer struct
- Add `pending_uploads: Vec<GlyphUpload>` field
- Add `GlyphUpload` struct (data, atlas_x, atlas_y, width, height)

### Step 2: Implement queue_glyph_upload()
- Rasterize glyph with fontdue
- Allocate atlas space via glyph_cache.insert()
- Queue upload (don't execute immediately)

### Step 3: Implement flush_glyph_uploads()
- Called once per frame BEFORE surface.get_current_texture()
- Batch upload all pending glyphs with queue.write_texture()
- Clear pending_uploads after flush

### Step 4: Call flush in render()
- Insert `self.flush_glyph_uploads()` after process_pty_output()
- Add perf.mark("flush_uploads")

**Checkpoint**: 13:15 PM - Texture upload path working

---

## 🚧 Phase 4: Actual Rendering (13:15-15:15 PM - 2h)

### Step 1: Implement build_vertex_buffer()
- Iterate terminal_grid cells
- Lookup glyph in cache (or queue upload if miss)
- Calculate NDC screen position
- Build 6 vertices per glyph quad (2 triangles)

### Step 2: Add build_glyph_quad() helper
- Generate 6 vertices for glyph rectangle
- Use GlyphInfo for texture coordinates
- Use CellStyle for foreground color

### Step 3: Update render() method
- Replace TODO at line 415-419
- Set pipeline and bind group
- Build vertex buffer
- Create dynamic buffer with vertices
- Call render_pass.draw()

**Checkpoint**: 15:15 PM - "Hello, MONOTERMINAL!" visible on screen

---

## 📊 Phase 5: FPS Testing & 16:00 Checkpoint (15:15-16:00 PM)

### Step 1: Build release binary
```powershell
cargo build --release -p monoterminal-master
```

### Step 2: Run FPS test with scrolling
```powershell
./target/release/monoterminal-master --fps-counter

# In terminal, run scrolling workload:
# - cat large file (10,000 lines)
# - Observe FPS counter during rapid scroll
```

### Step 3: Collect metrics
- p50 FPS (target: ≥ 60)
- p95 FPS (target: ≥ 58)
- Frame drops count (target: 0)
- GPU render time per frame (target: < 8ms)

### Step 4: Report to eng-director at 16:00
**Report includes**:
- FPS metrics (p50, p95)
- Pass/Fail against SRS §7.1 criteria
- Bottlenecks identified (if any)
- Monday evening work plan

---

## 🔧 Monday Evening Work (16:00-20:00 PM)

### If p50 ≥ 60 FPS (SUCCESS):
- ✅ Add dirty cell tracking to terminal_grid
- ✅ Pre-warm glyph cache with ASCII 32-126
- ✅ Shader optimization (if needed)
- ✅ Prepare Tuesday handoff to gpu-rendering-engineer

### If p50 < 60 FPS (TUNING NEEDED):
- 🔧 GPU profiler analysis (find bottleneck)
- 🔧 Throttle uploads (max 100 glyphs/frame)
- 🔧 Add staging buffer for async upload
- 🔧 Reduce atlas size to 2048×2048 if needed
- 🔧 Report slip to eng-director

---

## Emergency Contacts

**Blockers**: Report immediately to eng-director  
**Technical questions**: Coordinate with gpu-rendering-engineer  
**Toolchain issues**: Escalate to devops-lead

---

## Key Files Reference

```
crates/master/src/ui/
├── renderer.rs          # Main rendering loop (lines 170-431)
├── fonts.rs             # FontManager (modify for fontdue)
├── glyph_cache.rs       # Guillotine allocator (TODOs at 97-99)
├── terminal_grid.rs     # Terminal state (iterate cells)
├── vt_parser.rs         # VT sequence parser
└── shaders/
    └── text.wgsl        # Complete shader (ready to use)

Cargo.toml               # Add fontdue dependency
```

---

**Status**: READY FOR MONDAY 9 AM  
**Next Action**: Wait for "environment ready" signal  
**Owner**: rust-backend-lead
