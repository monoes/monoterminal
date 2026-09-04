# Week 8 Day 3-4: Optimization Implementation Plan

**Phase:** 3 Week 8 Day 3-4  
**Task:** task-64  
**Engineer:** performance-engineer  
**Date:** 2026-08-20  
**Status:** 📋 PLANNED (awaiting Day 1-2 completion)

---

## Objective

Implement high-priority optimizations identified in Week 7 analysis based on Windows baseline profiling results.

---

## Optimization Priorities

Based on Week 7 `OPTIMIZATION-RECOMMENDATIONS.md`:

### High Priority (Day 3-4 Implementation)

1. ✅ **Platform Parity Tuning**
   - **Goal:** <10% GPU variance (DX12 vs Vulkan vs Metal)
   - **Current:** DX12 30.17 µs (baseline)
   - **Expected:** Vulkan ~35 µs, Metal ~28 µs
   - **Status:** Within <20% threshold, but can optimize to <10%

2. ✅ **Mobile Battery Optimization (Frame Rate Throttling)**
   - **Goal:** Comparable battery drain to native apps
   - **Approach:** Dynamic frame rate (30 FPS idle, 60 FPS active)
   - **Expected:** 40-50% battery savings on mobile browsers

3. ✅ **Vertex Buffer Reuse**
   - **Goal:** Reduce CPU overhead by 50%
   - **Current:** 38.0 µs vertex buffer build
   - **Expected:** ~20 µs (15-20 µs savings)

### Medium Priority (Defer to Phase 4)

4. ⏸️ **SIMD UTF-8 Validation**
   - Already 21x faster than target (not worth effort in Phase 3)
   
5. ⏸️ **Memory Pooling (>1000 sessions)**
   - Enterprise-scale optimization (defer to Phase 4)

---

## Implementation Details

### 1. Platform Parity Tuning (GPU)

**Objective:** Ensure DX12, Vulkan, and Metal backends have <10% performance variance

**Current Status:**
- Windows (DX12): 30.17 µs ✅
- Linux (Vulkan): ~35 µs (predicted, +16%)
- macOS (Metal): ~28 µs (predicted, -7%)
- **Variance:** 16% (within <20% SRS threshold, but can optimize)

**Optimization Approach:**

**Step 1: Profile all backends**
```bash
# Windows (DX12) - already profiled
cargo bench --bench fps_rendering

# Linux (Vulkan) - via CI or VM
cargo bench --bench fps_rendering

# macOS (Metal) - via CI or VM
cargo bench --bench fps_rendering
```

**Step 2: Identify slowest backend**
- If Vulkan slowest (~35 µs): Optimize Vulkan pipeline
- If Metal fastest (~28 µs): Analyze Metal optimizations, apply to others

**Step 3: Backend-specific tuning**

**Vulkan Optimization (if needed):**
```rust
// crates/master/src/ui/renderer.rs (example)
// Optimize Vulkan descriptor set updates
// Current: Per-frame descriptor updates
// Optimized: Cached descriptor sets + minimal updates

impl Renderer {
    fn optimize_vulkan_pipeline(&mut self) {
        // 1. Pre-allocate descriptor sets
        // 2. Use push constants for small uniform data
        // 3. Reduce pipeline barrier overhead
        // 4. Batch draw calls more aggressively
    }
}
```

**Metal Optimization (if already fastest):**
```rust
// If Metal is fastest (~28 µs):
// 1. Analyze Metal-specific optimizations (e.g., tile shaders)
// 2. Apply similar patterns to DX12/Vulkan (e.g., better batching)
// 3. Ensure all backends use optimal draw call patterns
```

**Expected Result:**
- All backends: 28-30 µs range
- Variance: <10%
- Status: ✅ Within optimized target

**Effort:** 2-3 hours (pending CI results for Linux/macOS)

**Dependencies:**
- CI execution for Linux/macOS results
- Or manual VM testing

---

### 2. Mobile Battery Optimization (Frame Rate Throttling)

**Objective:** Reduce battery drain on mobile browsers by 40-50% through adaptive frame rate

**Current Status:**
- Frame rate: Fixed 60 FPS (wasteful on idle screens)
- Mobile battery impact: Unknown (not yet tested)
- SRS criterion: Comparative to native apps (not absolute)

**Optimization Approach:**

**Step 1: Activity detection**
```rust
// crates/master/src/ui/renderer.rs
use std::time::{Duration, Instant};

pub struct ActivityTracker {
    last_input: Instant,
    idle_threshold: Duration,
}

impl ActivityTracker {
    pub fn new() -> Self {
        Self {
            last_input: Instant::now(),
            idle_threshold: Duration::from_secs(5), // 5 sec idle → 30 FPS
        }
    }

    pub fn on_input(&mut self) {
        self.last_input = Instant::now();
    }

    pub fn is_idle(&self) -> bool {
        self.last_input.elapsed() > self.idle_threshold
    }

    pub fn target_fps(&self) -> u32 {
        if self.is_idle() {
            30 // Idle: 30 FPS (battery savings)
        } else {
            60 // Active: 60 FPS (smooth interaction)
        }
    }
}
```

**Step 2: Adaptive frame pacing**
```rust
// crates/master/src/ui/mod.rs (example)
impl App {
    fn run_frame_loop(&mut self) {
        let mut activity_tracker = ActivityTracker::new();

        loop {
            let target_fps = activity_tracker.target_fps();
            let frame_budget = Duration::from_micros(1_000_000 / target_fps as u64);

            let frame_start = Instant::now();

            // Render frame
            self.render_frame();

            // Handle input (updates activity tracker)
            if self.handle_input() {
                activity_tracker.on_input();
            }

            // Sleep for remaining frame budget
            let elapsed = frame_start.elapsed();
            if elapsed < frame_budget {
                std::thread::sleep(frame_budget - elapsed);
            }
        }
    }
}
```

**Step 3: Mobile detection (optional)**
```rust
// Detect mobile browsers via User-Agent (web client reports this)
impl ActivityTracker {
    pub fn is_mobile(&self) -> bool {
        // Set by web client during connection
        self.is_mobile_device
    }

    pub fn target_fps(&self) -> u32 {
        if self.is_mobile() {
            // More aggressive throttling on mobile
            if self.is_idle() {
                15 // Mobile idle: 15 FPS (max battery savings)
            } else {
                30 // Mobile active: 30 FPS (balance)
            }
        } else {
            // Desktop
            if self.is_idle() {
                30 // Desktop idle: 30 FPS
            } else {
                60 // Desktop active: 60 FPS
            }
        }
    }
}
```

**Expected Result:**
- Idle (5+ sec no input): 30 FPS (50% less GPU work)
- Active (recent input): 60 FPS (smooth interaction)
- Mobile idle: 15 FPS (75% less GPU work)
- Mobile active: 30 FPS (50% less GPU work)
- **Battery savings:** 40-50% on mobile (estimated)

**Effort:** 2-3 hours

**Dependencies:** None (can implement immediately)

---

### 3. Vertex Buffer Reuse

**Objective:** Reduce CPU overhead by 50% through in-place vertex buffer updates

**Current Status:**
- Vertex buffer build: 38.0 µs (80x24 grid)
- Allocates new buffer every frame
- Unnecessary allocations for mostly-static content

**Optimization Approach:**

**Step 1: Analyze current implementation**
```rust
// Current (example - actual code may differ)
fn build_vertex_buffer(&self, grid: &Grid) -> Vec<Vertex> {
    let mut vertices = Vec::new(); // ❌ New allocation every frame

    for (row, col, cell) in grid.iter() {
        // Build vertex data for each cell
        vertices.extend(cell_to_vertices(row, col, cell));
    }

    vertices
}
```

**Step 2: Implement buffer reuse**
```rust
// Optimized: Reuse buffer + in-place updates
pub struct VertexBufferCache {
    buffer: Vec<Vertex>,
    capacity: usize,
}

impl VertexBufferCache {
    pub fn new(grid_size: (usize, usize)) -> Self {
        let capacity = grid_size.0 * grid_size.1 * 6; // 6 vertices per cell (2 triangles)
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn update_from_grid(&mut self, grid: &Grid, dirty_cells: &[CellPos]) -> &[Vertex] {
        // Only update dirty cells (incremental rendering)
        for &(row, col) in dirty_cells {
            let cell = &grid[row][col];
            let offset = (row * grid.cols + col) * 6; // 6 vertices per cell

            // In-place update (no allocation)
            for (i, vertex) in cell_to_vertices(row, col, cell).iter().enumerate() {
                self.buffer[offset + i] = *vertex;
            }
        }

        &self.buffer[..]
    }
}
```

**Step 3: Integrate with dirty tracking**
```rust
// Leverage existing dirty tracking (already 1.40 µs)
impl Renderer {
    fn render_frame(&mut self, grid: &Grid, dirty_tracker: &DirtyTracker) {
        // Get dirty cells (already tracked, 1.40 µs)
        let dirty_cells = dirty_tracker.get_dirty();

        // Update only dirty cells (no full rebuild)
        let vertices = self.vertex_cache.update_from_grid(grid, &dirty_cells);

        // Upload to GPU (unchanged)
        self.upload_vertices(vertices);
    }
}
```

**Expected Result:**
- Current: 38.0 µs (full rebuild)
- Optimized: ~20 µs (50% reduction)
- Incremental (1% dirty): ~2 µs (95% reduction)
- **CPU overhead:** 15-20 µs savings per frame

**Effort:** 2-3 hours

**Dependencies:** None (dirty tracking already implemented)

---

## Implementation Schedule

### Day 3 (2-3 hours)

**Morning:**
1. Implement mobile battery optimization (frame rate throttling) - 2 hours
2. Test and validate (manual testing)

**Afternoon:**
3. Implement vertex buffer reuse - 2 hours
4. Benchmark and validate improvements

### Day 4 (1-2 hours)

**Morning:**
5. Platform parity tuning (if CI results available) - 2 hours
6. Or defer to Week 9 if CI pending

**Afternoon:**
7. Re-run benchmarks (validate no regressions)
8. Document optimization results

---

## Validation Criteria

### Per-Optimization Success Criteria

**1. Platform Parity:**
- ✅ DX12/Vulkan/Metal variance <10%
- ✅ All backends <35 µs (within 60 FPS budget)
- ✅ No regressions on any platform

**2. Mobile Battery:**
- ✅ Idle frame rate: 30 FPS (or 15 FPS mobile)
- ✅ Active frame rate: 60 FPS (or 30 FPS mobile)
- ✅ Transitions smooth (<1 sec lag)

**3. Vertex Buffer Reuse:**
- ✅ CPU overhead: <25 µs (from 38 µs)
- ✅ Incremental (1% dirty): <5 µs
- ✅ No visual regressions

---

## Risk Assessment

### Risks

**1. Platform Parity - CI Dependency**
- **Likelihood:** Medium (CI requires manual trigger)
- **Impact:** Low (Windows baseline sufficient, can defer to Week 9)
- **Mitigation:** Implement other optimizations first, revisit when CI available

**2. Mobile Battery - Testing Difficulty**
- **Likelihood:** Medium (requires mobile device testing)
- **Impact:** Low (SRS criterion is comparative, not absolute)
- **Mitigation:** Test on desktop browser first, mobile validation deferred

**3. Vertex Buffer - Rendering Regressions**
- **Likelihood:** Low (incremental rendering already validated)
- **Impact:** Medium (visual glitches)
- **Mitigation:** Comprehensive visual testing, easy rollback

---

## Deliverables

### Code Changes

**Files to Modify:**
1. `crates/master/src/ui/renderer.rs` (all 3 optimizations)
2. `crates/master/src/ui/mod.rs` (frame pacing loop)
3. `crates/master/src/ui/dirty.rs` (if vertex buffer integration needed)

### Documentation

**Files to Create:**
1. `tests/evidence/phase3/OPTIMIZATION-IMPLEMENTATION-RESULTS.md`
2. `tests/evidence/phase3/WEEK-8-DAY-3-4-SUMMARY.md`

**Files to Update:**
1. `tests/evidence/phase3/PLATFORM-COMPARISON-MATRIX.md` (post-optimization results)
2. `tests/evidence/phase3/OPTIMIZATION-RECOMMENDATIONS.md` (mark implemented)

### Benchmarks

**Re-run after optimization:**
1. `cargo bench --bench fps_rendering` (validate improvements)
2. `cargo bench --bench pty_throughput` (validate no regressions)
3. Manual visual testing (check for rendering glitches)

---

## Next Steps (After Day 3-4)

### Day 5-6: Validation

1. Re-run all benchmarks
2. Validate improvements match predictions
3. Confirm no regressions
4. Generate final performance reports

### Day 7: Week 8 Summary

1. Week 8 summary report
2. Final SRS compliance matrix
3. Phase 3 performance validation complete

---

**Status:** Planned (awaiting Day 1-2 completion)

**Dependencies:** Day 1-2 profiling complete

**Updated:** 2026-08-20  
**Engineer:** performance-engineer
