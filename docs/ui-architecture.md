# MONOTERMINAL UI Architecture

## Status: Foundation Ready (Pending Integration)

Implementation of wgpu + egui rendering system per SRS §2.1.1, §4.2.1.

**Blocked on:** task-5 (session-manager-runtime) for live session data integration

---

## Module Structure

```
crates/master/src/ui/
├── mod.rs           # Main UI entry point
├── window.rs        # Window management & event loop (winit)
├── renderer.rs      # wgpu renderer (DirectX 12 backend)
├── performance.rs   # Frame timing & FPS monitoring
├── fonts.rs         # Font loading & glyph cache
└── layout.rs        # UI layout components
```

---

## Frame Budget (60 FPS = 16.67ms target)

Per SRS §2.1.1:

| Phase           | Budget | Owner                    |
|-----------------|--------|--------------------------|
| PTY read        | 2ms    | Backend (pty module)     |
| Dirty tracking  | 0.5ms  | performance.rs           |
| Glyph lookup    | 1ms    | fonts.rs (GlyphCache)    |
| GPU render      | 8ms    | renderer.rs              |
| VSync           | 5ms    | wgpu present mode        |
| **Total**       | **16.5ms** | ✅ Within budget     |

---

## Architecture Decisions

### Threading Model

- **Main thread:** UI event loop (winit + egui) - owns the window
- **PTY thread:** tokio async (managed by backend, separate)
- **Render thread:** wgpu command submission

### Graphics Backend

- **Phase 1 (Windows):** DirectX 12 via wgpu
- **Phase 3 (Linux):** Vulkan via wgpu
- **Phase 3 (macOS):** Metal via wgpu

### UI Layout (SRS §4.2.1)

```
┌────────────────────────────────────┐
│  Menu Bar (File, Session, Help)   │  ← 30px height
├────────────────────────────────────┤
│  Session │ Terminal Canvas        │  ← Main area
│  List    │ (wgpu rendered)        │
│  200px   │                         │
├────────────────────────────────────┤
│  Status Bar (FPS, latency)         │  ← 25px height
└────────────────────────────────────┘
```

### Performance Monitoring

`PerformanceMonitor` tracks:
- Frame time (target: ≤16.67ms)
- FPS (rolling 60-frame average)
- Per-phase timing marks
- Budget violation logging

---

## Implementation Status

### ✅ Completed (Prep Work)

- [x] wgpu instance creation (DirectX 12 backend)
- [x] Surface/device initialization
- [x] Window scaffolding (winit 0.30)
- [x] Event loop handling
- [x] Performance monitoring framework
- [x] Frame timing infrastructure
- [x] Layout calculation (menu, sidebar, canvas, status)
- [x] Font manager skeleton
- [x] Basic render pass (clear color)

### 🚧 Pending Integration (waiting on task-5)

- [ ] Live session data binding
- [ ] Terminal canvas rendering (actual text)
- [ ] Session list population
- [ ] egui integration (UI widgets)
- [ ] Glyph cache implementation (Guillotine bin-packing)
- [ ] HarfBuzz text shaping integration

### 📋 Future Work (Phase 2+)

- [ ] Cairo CPU fallback renderer
- [ ] Sixel graphics compositing (Phase 4)
- [ ] True-color RGB support (OSC SGR)
- [ ] Hyperlink support (OSC 8)

---

## Dependencies Added

```toml
# crates/master/Cargo.toml
winit = "0.30"      # Window management
pollster = "0.3"    # Async blocker for wgpu init
```

Workspace dependencies already configured:
- `wgpu = "0.20"`
- `egui = "0.28"`

---

## Testing

**Example binary:** `examples/ui_test.rs`

```bash
# Test UI rendering independently
cargo run --example ui_test
```

**Note:** Currently placeholder until library exports are configured.

---

## Integration Plan (Post Task-5)

Once session-manager-runtime (task-5) completes:

1. **Wire session data:**
   - Connect `SessionList` to live `SessionManager`
   - Display active sessions in sidebar

2. **Terminal rendering:**
   - Bind PTY output to terminal canvas
   - Implement glyph rendering pipeline
   - Add scrollback buffer integration

3. **egui integration:**
   - Menu bar (File, Session, Help)
   - Session sidebar (clickable list)
   - Status bar (FPS, latency, session count)

4. **Dual-mode operation:**
   - Run UI on main thread
   - Spawn WebSocket server on tokio runtime
   - Support headless mode (server-only)

---

## Performance Validation

Once integrated, validate frame budgets with:

- **RenderDoc** - GPU profiling
- **cargo-flamegraph** - CPU profiling
- Built-in `PerformanceMonitor` - frame timing logs

Target validation:
- [ ] 60 FPS sustained under normal load
- [ ] <16.67ms p95 frame time
- [ ] Individual phase budgets met
- [ ] No frame drops during PTY burst

---

## Files Created

```
crates/master/src/ui/mod.rs           # Main UI module
crates/master/src/ui/window.rs        # Window & event loop
crates/master/src/ui/renderer.rs      # wgpu renderer
crates/master/src/ui/performance.rs   # Performance monitoring
crates/master/src/ui/fonts.rs         # Font management
crates/master/src/ui/layout.rs        # UI layout
crates/master/examples/ui_test.rs     # Test binary
docs/ui-architecture.md               # This document
```

---

## Next Steps

**Immediate (when task-5 completes):**
1. Notification from eng-director
2. Review session manager API
3. Integrate live session data
4. Implement terminal canvas rendering
5. Add egui UI widgets

**Testing:**
1. Validate 60 FPS target
2. Profile with RenderDoc
3. Test DirectX 12 on Windows 10/11
4. Verify frame budget compliance

---

**Owner:** gpu-rendering-engineer  
**Last Updated:** 2026-08-15  
**Status:** ✅ Foundation ready, ⏸️ blocked on task-5
