# Phase 3 Week 6: Cross-Platform Rendering Validation Plan

**Task:** task-57  
**Engineer:** gpu-rendering-engineer  
**Date:** 2026-08-19  
**Timeline:** 3-4 days

## Objective

Validate existing wgpu renderer works correctly on Linux (Vulkan/OpenGL) and macOS (Metal), ensuring 60 FPS target across all platforms.

## Current State Analysis

### Renderer Implementation (`renderer.rs`)
**Issue:** Hardcoded to DirectX 12 backend
```rust
let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
    backends: wgpu::Backends::DX12, // DirectX 12 for Windows (Phase 1)
    ..Default::default()
});
```

**Required Change:** Platform-aware backend selection

### Test Infrastructure (`test_support.rs`)
**Status:** ✅ Already cross-platform capable
- `new_with_backends(backends: wgpu::Backends)` supports all backends
- Tested: DirectX 12 ✅, Vulkan ✅ (partial)
- Ready: Metal, OpenGL

## Implementation Plan

### Phase 1: Update Renderer for Cross-Platform Support

**1.1 Backend Selection Logic**
```rust
pub fn select_backend() -> wgpu::Backends {
    #[cfg(target_os = "windows")]
    return wgpu::Backends::DX12;
    
    #[cfg(target_os = "linux")]
    return wgpu::Backends::VULKAN | wgpu::Backends::GL; // Vulkan preferred, GL fallback
    
    #[cfg(target_os = "macos")]
    return wgpu::Backends::METAL;
    
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    return wgpu::Backends::all();
}
```

**1.2 Font Manager Platform Support**
- Windows: Consolas (fontdue) ✅ Implemented
- Linux: FreeType integration (Phase 3)
- macOS: CoreText integration (Phase 3)

### Phase 2: Cross-Platform Test Suite

**2.1 Extend Headless GPU Tests**
```rust
#[tokio::test]
async fn test_linux_vulkan_backend() {
    #[cfg(target_os = "linux")]
    {
        let ctx = HeadlessGpuContext::new_with_backends(wgpu::Backends::VULKAN)
            .await
            .unwrap();
        assert_eq!(ctx.adapter_info().backend, wgpu::Backend::Vulkan);
        
        // Run 60 FPS benchmark
        validate_60fps_target(&ctx).await;
    }
}

#[tokio::test]
async fn test_macos_metal_backend() {
    #[cfg(target_os = "macos")]
    {
        let ctx = HeadlessGpuContext::new_with_backends(wgpu::Backends::METAL)
            .await
            .unwrap();
        assert_eq!(ctx.adapter_info().backend, wgpu::Backend::Metal);
        
        // Run 60 FPS benchmark
        validate_60fps_target(&ctx).await;
    }
}
```

**2.2 Visual Consistency Tests**
- Render test pattern on all platforms
- Compare pixel output for color accuracy
- Validate font rendering consistency

### Phase 3: Platform-Specific Validation

**3.1 Linux (Ubuntu 22.04) Validation Checklist**
- [ ] Build succeeds (`cargo build --release`)
- [ ] Vulkan backend detected
- [ ] OpenGL fallback works if Vulkan unavailable
- [ ] 60 FPS achieved (headless benchmark)
- [ ] Font rendering correct (FreeType)
- [ ] Window resize works correctly
- [ ] All 61 GPU tests pass

**3.2 macOS (13+) Validation Checklist**
- [ ] Build succeeds (`cargo build --release`)
- [ ] Metal backend detected
- [ ] 60 FPS achieved (headless benchmark)
- [ ] Retina/HiDPI support validated
- [ ] Font rendering correct (CoreText)
- [ ] Window resize works correctly
- [ ] All 61 GPU tests pass

**3.3 Cross-Platform Consistency**
- [ ] Color rendering identical across platforms
- [ ] Font metrics consistent (cell dimensions)
- [ ] Performance parity (all ≥60 FPS)
- [ ] Terminal output visually identical

### Phase 4: CI Integration

**4.1 GitHub Actions Workflow**
```yaml
name: Cross-Platform Rendering Tests

on: [push, pull_request]

jobs:
  test-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - run: cargo test --lib ui::test_support_integration
      - run: cargo bench --bench rendering_performance
  
  test-linux:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v3
      - run: sudo apt-get install -y libvulkan-dev mesa-vulkan-drivers
      - uses: actions-rs/toolchain@v1
      - run: cargo test --lib ui::test_support_integration
      - run: cargo bench --bench rendering_performance
  
  test-macos:
    runs-on: macos-13
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - run: cargo test --lib ui::test_support_integration
      - run: cargo bench --bench rendering_performance
```

### Phase 5: Documentation Updates

**5.1 Platform-Specific Documentation**
- `docs/linux-setup.md` - Linux build/run instructions
- `docs/macos-setup.md` - macOS build/run instructions  
- `docs/cross-platform-rendering.md` - Backend selection guide

**5.2 Troubleshooting Guide**
- Vulkan driver issues (Linux)
- Mesa/llvmpipe fallback (Linux headless)
- Metal shader compilation (macOS)
- HiDPI scaling (macOS Retina)

## Deliverables

### Deliverable 1: Updated Renderer
- [ ] Platform-aware backend selection in `renderer.rs`
- [ ] Compile-time backend configuration
- [ ] Runtime backend detection and fallback

### Deliverable 2: Linux Validation Report
- [ ] Build verification (Ubuntu 22.04)
- [ ] Vulkan backend validation
- [ ] OpenGL fallback validation
- [ ] 60 FPS benchmark results
- [ ] Font rendering screenshots
- [ ] Test results (61 GPU tests)

### Deliverable 3: macOS Validation Report
- [ ] Build verification (macOS 13+)
- [ ] Metal backend validation
- [ ] 60 FPS benchmark results
- [ ] Retina display validation
- [ ] Font rendering screenshots
- [ ] Test results (61 GPU tests)

### Deliverable 4: Cross-Platform Consistency Report
- [ ] Color rendering comparison (screenshots)
- [ ] Font rendering comparison
- [ ] Performance comparison table
- [ ] Platform-specific quirks documented

### Deliverable 5: CI Integration
- [ ] GitHub Actions workflow created
- [ ] Linux runner configured
- [ ] macOS runner configured
- [ ] All platform tests passing in CI

## Acceptance Criteria

✅ **60 FPS on Linux (Vulkan/OpenGL)**
- Headless benchmark: ≥60 FPS average
- Frame time: ≤16.67ms p95

✅ **60 FPS on macOS (Metal)**
- Headless benchmark: ≥60 FPS average
- Frame time: ≤16.67ms p95
- Retina display: ≥60 FPS

✅ **Visual Consistency**
- Color values match across platforms (±1% tolerance)
- Font rendering visually identical
- Terminal output pixel-perfect match

✅ **Font Rendering Correct**
- Windows: Consolas rendering correct
- Linux: Monospace font rendering correct (FreeType)
- macOS: Menlo/SF Mono rendering correct (CoreText)

## Timeline

**Day 1: Implementation**
- Update renderer.rs for cross-platform support
- Extend test suite with platform-specific tests
- Create validation scripts

**Day 2: Linux Validation**
- Set up Ubuntu 22.04 environment
- Run validation checklist
- Generate Linux validation report
- Fix any Linux-specific issues

**Day 3: macOS Validation**
- Set up macOS 13+ environment
- Run validation checklist  
- Generate macOS validation report
- Fix any macOS-specific issues

**Day 4: CI Integration + Documentation**
- Create GitHub Actions workflow
- Update documentation
- Generate cross-platform consistency report
- Final verification

## Risk Mitigation

**Risk:** No access to Linux/macOS hardware for validation
- **Mitigation:** Use GitHub Actions runners for testing
- **Fallback:** Document validation procedures for manual testing

**Risk:** Platform-specific rendering bugs
- **Mitigation:** Headless testing infrastructure isolates GPU issues
- **Fallback:** Software rendering (Mesa/llvmpipe) as backup

**Risk:** Font rendering differences
- **Mitigation:** Font manager abstraction already in place
- **Fallback:** Document platform-specific font requirements

## Notes

- Reusing task-21 headless GPU test infrastructure
- No new test code needed, just platform validation
- Focus on documentation and CI integration
- Leverage existing 61 GPU tests for validation
