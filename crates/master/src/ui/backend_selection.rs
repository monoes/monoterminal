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
/// **CI Environment (GitHub Actions, etc.):**
/// - Detected via CI or GITHUB_ACTIONS environment variables
/// - Uses PRIMARY backends + OpenGL for software rendering fallback
/// - Ensures tests pass even without hardware GPU acceleration
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
    // CI environment detection - force software-compatible backends
    // PRIMARY includes Vulkan/Metal/DX12, GL provides software fallback (Mesa/llvmpipe)
    if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
        tracing::info!("CI environment detected - using software-compatible backends (PRIMARY | GL)");
        tracing::debug!("CI env vars: CI={:?}, GITHUB_ACTIONS={:?}",
            std::env::var("CI"),
            std::env::var("GITHUB_ACTIONS"));
        return wgpu::Backends::PRIMARY | wgpu::Backends::GL;
    }

    // Production platform-specific backend selection
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

        // CI environment: should return PRIMARY | GL for software fallback
        if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
            assert!(backends.contains(wgpu::Backends::GL),
                "CI environment should include GL backend for software rendering");
            assert!(backends.contains(wgpu::Backends::PRIMARY),
                "CI environment should include PRIMARY backends");
        } else {
            // Production environment: platform-specific backends
            #[cfg(target_os = "windows")]
            assert_eq!(backends, wgpu::Backends::DX12);

            #[cfg(target_os = "linux")]
            assert!(backends.contains(wgpu::Backends::VULKAN));

            #[cfg(target_os = "macos")]
            assert_eq!(backends, wgpu::Backends::METAL);
        }
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

    #[test]
    fn test_ci_environment_detection() {
        // This test verifies CI detection logic without setting env vars
        // (actual CI behavior tested by test_backend_selection_returns_platform_appropriate)

        // Save current env state
        let ci_was_set = std::env::var("CI").is_ok();
        let gh_was_set = std::env::var("GITHUB_ACTIONS").is_ok();

        // Test CI detection
        std::env::set_var("CI", "true");
        let backends = select_backend();
        assert!(backends.contains(wgpu::Backends::GL),
            "CI=true should trigger GL backend");

        // Cleanup
        std::env::remove_var("CI");
        if !ci_was_set {
            std::env::remove_var("CI");
        }
        if gh_was_set {
            std::env::set_var("GITHUB_ACTIONS", "true");
        }
    }
}
