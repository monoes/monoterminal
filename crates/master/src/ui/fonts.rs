//! Font loading and rasterization
//!
//! Phase 1: fontdue (pure Rust, Windows)
//! Phase 3: HarfBuzz integration for complex shaping (Linux/macOS)
//!
//! Target: <1ms glyph lookup (per SRS §2.1.1)

use anyhow::{Context, Result};
use fontdue::{Font, FontSettings};
use std::path::PathBuf;

/// Rasterized glyph bitmap
pub struct RasterizedGlyph {
    /// Grayscale bitmap data (row-major, top-to-bottom)
    pub bitmap: Vec<u8>,
    /// Width in pixels
    pub width: usize,
    /// Height in pixels
    pub height: usize,
    /// Metrics
    pub metrics: GlyphMetrics,
}

/// Glyph metrics for layout
#[derive(Debug, Clone, Copy)]
pub struct GlyphMetrics {
    /// Advance width (pixels to next glyph)
    pub advance_width: f32,
    /// Left bearing (horizontal offset from origin)
    pub bearing_x: f32,
    /// Top bearing (vertical offset from baseline)
    pub bearing_y: f32,
}

/// Font manager for terminal rendering
pub struct FontManager {
    /// Loaded font (fontdue)
    font: Font,
    /// Font size in pixels
    font_size: f32,
}

impl FontManager {
    /// Create new font manager with system font fallback
    /// Windows: Consolas → Courier New → fallback
    pub fn new(font_size: f32) -> Result<Self> {
        let font_data = Self::load_system_font()?;
        let font = Font::from_bytes(font_data, FontSettings::default())
            .map_err(|e| anyhow::anyhow!("Failed to parse font: {}", e))?;

        tracing::info!("Font loaded successfully (size: {}px)", font_size);

        Ok(Self { font, font_size })
    }

    /// Load system monospace font with fallback chain
    /// Windows: C:\Windows\Fonts\consola.ttf (Consolas)
    fn load_system_font() -> Result<Vec<u8>> {
        #[cfg(target_os = "windows")]
        {
            // Try Consolas first (best Windows monospace font)
            let consolas_path = PathBuf::from(r"C:\Windows\Fonts\consola.ttf");
            if consolas_path.exists() {
                tracing::info!("Loading Consolas font");
                return std::fs::read(&consolas_path).context("Failed to read Consolas font");
            }

            // Fallback: Courier New
            let courier_path = PathBuf::from(r"C:\Windows\Fonts\cour.ttf");
            if courier_path.exists() {
                tracing::warn!("Consolas not found, falling back to Courier New");
                return std::fs::read(&courier_path).context("Failed to read Courier New font");
            }

            anyhow::bail!("No suitable monospace font found in C:\\Windows\\Fonts");
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Phase 3: Linux/macOS font loading
            anyhow::bail!("Non-Windows font loading not yet implemented (Phase 3)")
        }
    }

    /// Rasterize a glyph to bitmap
    /// Returns grayscale bitmap + metrics
    /// Target: <1ms per glyph
    pub fn rasterize_glyph(&self, ch: char, _bold: bool, _italic: bool) -> Result<RasterizedGlyph> {
        // Get glyph index for character
        let glyph_index = self.font.lookup_glyph_index(ch);

        // Rasterize at configured font size
        let (metrics, bitmap) = self.font.rasterize_indexed(glyph_index, self.font_size);

        // Convert fontdue metrics to our format
        let glyph_metrics = GlyphMetrics {
            advance_width: metrics.advance_width,
            bearing_x: metrics.xmin as f32,
            bearing_y: metrics.ymin as f32,
        };

        Ok(RasterizedGlyph {
            bitmap,
            width: metrics.width,
            height: metrics.height,
            metrics: glyph_metrics,
        })
    }

    /// Get font metrics (cell dimensions for terminal grid)
    pub fn cell_dimensions(&self) -> (u32, u32) {
        // Measure 'M' (widest monospace char) for cell width
        let (metrics, _) = self.font.rasterize('M', self.font_size);
        let width = metrics.advance_width.ceil() as u32;

        // Use font line height for cell height
        let height = self
            .font
            .horizontal_line_metrics(self.font_size)
            .map(|m| (m.ascent - m.descent + m.line_gap).ceil() as u32)
            .unwrap_or(self.font_size.ceil() as u32);

        (width, height)
    }
}

impl Default for FontManager {
    fn default() -> Self {
        // Default: 16px font size (good for 1280x720 window)
        Self::new(16.0).expect("Failed to create FontManager")
    }
}

// Re-export GlyphCache from glyph_cache module
pub use super::glyph_cache::{GlyphCache, GlyphInfo, GlyphKey};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_manager_new() {
        let manager = FontManager::new(16.0).unwrap();
        assert_eq!(manager.font_size, 16.0);
    }

    #[test]
    fn test_font_manager_default() {
        let manager = FontManager::default();
        assert_eq!(manager.font_size, 16.0);
    }

    #[test]
    fn test_font_manager_various_sizes() {
        let sizes = [8.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0];

        for size in sizes {
            let manager = FontManager::new(size).unwrap();
            assert_eq!(manager.font_size, size);
        }
    }

    #[test]
    fn test_cell_dimensions() {
        let manager = FontManager::new(16.0).unwrap();
        let (width, height) = manager.cell_dimensions();

        // Monospace font should have reasonable cell dimensions
        assert!(width > 0);
        assert!(height > 0);
        assert!(width <= 20); // Typical monospace at 16px
        assert!(height <= 25);

        println!("Cell dimensions at 16px: {}x{}", width, height);
    }

    #[test]
    fn test_cell_dimensions_scaling() {
        let manager_small = FontManager::new(12.0).unwrap();
        let manager_large = FontManager::new(24.0).unwrap();

        let (width_small, height_small) = manager_small.cell_dimensions();
        let (width_large, height_large) = manager_large.cell_dimensions();

        // Larger font should have larger cell dimensions
        assert!(width_large > width_small);
        assert!(height_large > height_small);
    }

    #[test]
    fn test_rasterize_ascii() {
        let manager = FontManager::new(16.0).unwrap();

        // Test ASCII characters
        let chars = ['A', 'a', '0', ' ', '#', '@'];

        for ch in chars {
            let glyph = manager.rasterize_glyph(ch, false, false).unwrap();

            assert!(glyph.width > 0 || ch == ' '); // Space may be empty
            assert!(glyph.height > 0 || ch == ' ');
            assert!(!glyph.bitmap.is_empty() || ch == ' ');

            println!(
                "Glyph '{}': {}x{}, {} bytes",
                ch,
                glyph.width,
                glyph.height,
                glyph.bitmap.len()
            );
        }
    }

    #[test]
    fn test_rasterize_unicode() {
        let manager = FontManager::new(16.0).unwrap();

        // Test some Unicode characters (font may not support all)
        let chars = ['→', '★', '♥'];

        for ch in chars {
            let result = manager.rasterize_glyph(ch, false, false);

            if let Ok(glyph) = result {
                println!("Unicode '{}': {}x{}", ch, glyph.width, glyph.height);
            } else {
                println!("Unicode '{}': not supported in font", ch);
            }
        }
    }

    #[test]
    fn test_rasterize_metrics() {
        let manager = FontManager::new(16.0).unwrap();
        let glyph = manager.rasterize_glyph('M', false, false).unwrap();

        // Check metrics are reasonable
        assert!(glyph.metrics.advance_width > 0.0);

        println!(
            "Metrics for 'M': advance={}, bearing_x={}, bearing_y={}",
            glyph.metrics.advance_width, glyph.metrics.bearing_x, glyph.metrics.bearing_y
        );
    }

    #[test]
    fn test_bitmap_size_matches_dimensions() {
        let manager = FontManager::new(16.0).unwrap();
        let glyph = manager.rasterize_glyph('A', false, false).unwrap();

        // Bitmap size should match width * height
        assert_eq!(glyph.bitmap.len(), glyph.width * glyph.height);
    }

    #[test]
    fn test_monospace_width_consistency() {
        let manager = FontManager::new(16.0).unwrap();

        // Monospace chars should have same advance width
        let chars = ['A', 'B', 'i', 'w', '0', '1'];
        let mut widths = Vec::new();

        for ch in chars {
            let glyph = manager.rasterize_glyph(ch, false, false).unwrap();
            widths.push(glyph.metrics.advance_width);
        }

        // All widths should be equal (or very close for monospace font)
        let first_width = widths[0];
        for (i, width) in widths.iter().enumerate() {
            assert!(
                (width - first_width).abs() < 0.1,
                "Char '{}' has different width: {} vs {}",
                chars[i],
                width,
                first_width
            );
        }
    }

    #[test]
    fn test_glyph_metrics_copy() {
        let metrics = GlyphMetrics {
            advance_width: 10.0,
            bearing_x: 1.0,
            bearing_y: 12.0,
        };

        let metrics2 = metrics;
        assert_eq!(metrics.advance_width, metrics2.advance_width);
    }

    #[test]
    fn test_rasterized_glyph_data() {
        let manager = FontManager::new(16.0).unwrap();
        let glyph = manager.rasterize_glyph('#', false, false).unwrap();

        // '#' should have significant pixel data
        assert!(glyph.bitmap.len() > 10);

        // Count non-zero pixels
        let non_zero = glyph.bitmap.iter().filter(|&&b| b > 0).count();
        assert!(non_zero > 0, "Glyph bitmap should have rendered pixels");

        println!(
            "'#' has {} non-zero pixels out of {}",
            non_zero,
            glyph.bitmap.len()
        );
    }
}
