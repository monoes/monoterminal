//! Cross-platform wgpu backend selection
//!
//! Selects the appropriate wgpu backend for each platform:
//! - Windows: DirectX 12
//! - Linux: Vulkan (preferred) or OpenGL (fallback)
//! - macOS: Metal
//!
//! Phase 3 Week 6 - Cross-platform rendering validation

use wgpu;

/// Select appropriate wgpu backend(s) for the current platform
///
/// Returns platform-specific backend(s) with fallback support where appropriate.
///
/// # Platform Behavior
///
/// **Windows:**
/// - Primary: DirectX 12
/// - No fallback (DirectX 12 is guaranteed on Windows 10+)
///
/// **Linux:**
/// - Primary: Vulkan
/// - Fallback: OpenGL (for headless or missing Vulkan drivers)
///
/// **macOS:**
/// - Primary: Metal (only option on macOS)
/// - No fallback (Metal is guaranteed on macOS 10.11+)
///
/// **Other:**
/// - All backends attempted (platform-agnostic fallback)
pub fn select_backend() -> wgpu::Backends {
    #[cfg(target_os = "windows")]
    {
        tracing::info!("Platform: Windows - selecting DirectX 12 backend");
        wgpu::Backends::DX12
    }

    #[cfg(target_os = "linux")]
    {
        tracing::info!("Platform: Linux - selecting Vulkan backend (OpenGL fallback)");
        wgpu::Backends::VULKAN | wgpu::Backends::GL
    }

    #[cfg(target_os = "macos")]
    {
        tracing::info!("Platform: macOS - selecting Metal backend");
        wgpu::Backends::METAL
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        tracing::warn!("Unknown platform - trying all backends");
        wgpu::Backends::all()
    }
}

/// Get expected backend for current platform
///
/// Used for validation to ensure the correct backend was selected.
#[cfg(test)]
pub fn expected_backend() -> wgpu::Backend {
    #[cfg(target_os = "windows")]
    return wgpu::Backend::Dx12;

    #[cfg(target_os = "linux")]
    return wgpu::Backend::Vulkan; // Primary (OpenGL is fallback)

    #[cfg(target_os = "macos")]
    return wgpu::Backend::Metal;

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    return wgpu::Backend::Empty; // Placeholder for unknown platforms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_selection_returns_platform_appropriate() {
        let backends = select_backend();

        #[cfg(target_os = "windows")]
        assert_eq!(backends, wgpu::Backends::DX12);

        #[cfg(target_os = "linux")]
        assert!(backends.contains(wgpu::Backends::VULKAN));

        #[cfg(target_os = "macos")]
        assert_eq!(backends, wgpu::Backends::METAL);
    }

    #[test]
    fn test_expected_backend_matches_platform() {
        let expected = expected_backend();

        #[cfg(target_os = "windows")]
        assert_eq!(expected, wgpu::Backend::Dx12);

        #[cfg(target_os = "linux")]
        assert_eq!(expected, wgpu::Backend::Vulkan);

        #[cfg(target_os = "macos")]
        assert_eq!(expected, wgpu::Backend::Metal);
    }
}
