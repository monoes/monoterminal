//! Glyph Cache with Guillotine Bin-Packing
//!
//! Implements 4096×4096 atlas with Guillotine bin-packing and LRU eviction
//! Target: <1ms glyph lookup per SRS §2.1.1

use super::vt_parser::CellStyle;
use std::collections::{HashMap, VecDeque};

/// Glyph cache key (character + style)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub ch: char,
    pub bold: bool,
    pub italic: bool,
}

impl GlyphKey {
    pub fn new(ch: char, style: &CellStyle) -> Self {
        Self {
            ch,
            bold: style.bold,
            italic: style.italic,
        }
    }
}

/// Glyph information for rendering
#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    /// Atlas texture coordinates (normalized 0.0-1.0)
    pub tex_x: f32,
    pub tex_y: f32,
    pub tex_width: f32,
    pub tex_height: f32,

    /// Glyph metrics
    pub bearing_x: i32,
    pub bearing_y: i32,
    pub advance: u32,
}

/// Glyph cache with Guillotine bin-packing and LRU eviction
pub struct GlyphCache {
    /// Atlas size (4096×4096 per SRS)
    atlas_width: u32,
    atlas_height: u32,

    /// Glyph lookup cache
    cache: HashMap<GlyphKey, GlyphInfo>,

    /// LRU tracking
    lru: VecDeque<GlyphKey>,

    /// Guillotine allocator
    allocator: GuillotineAllocator,

    /// Maximum cache size (LRU eviction threshold)
    max_cache_size: usize,
}

impl GlyphCache {
    /// Create new glyph cache with 4096×4096 atlas
    pub fn new() -> Self {
        Self {
            atlas_width: 4096,
            atlas_height: 4096,
            cache: HashMap::new(),
            lru: VecDeque::new(),
            allocator: GuillotineAllocator::new(4096, 4096),
            max_cache_size: 2048, // Evict after 2048 glyphs (conservative)
        }
    }

    /// Lookup glyph in cache
    /// Target: <1ms
    pub fn lookup(&mut self, key: GlyphKey) -> Option<GlyphInfo> {
        if let Some(info) = self.cache.get(&key) {
            // Update LRU (move to back)
            self.lru.retain(|k| k != &key);
            self.lru.push_back(key);
            return Some(*info);
        }
        None
    }

    /// Insert glyph into cache with LRU eviction
    pub fn insert(&mut self, key: GlyphKey, width: u32, height: u32) -> Option<GlyphInfo> {
        // Try to allocate space in atlas
        let rect = self.allocator.allocate(width, height)?;

        // Normalize texture coordinates to 0.0-1.0
        let info = GlyphInfo {
            tex_x: rect.x as f32 / self.atlas_width as f32,
            tex_y: rect.y as f32 / self.atlas_height as f32,
            tex_width: rect.width as f32 / self.atlas_width as f32,
            tex_height: rect.height as f32 / self.atlas_height as f32,
            bearing_x: 0, // TODO: Get from font metrics
            bearing_y: 0,
            advance: width, // TODO: Get from font metrics
        };

        // Insert into cache
        self.cache.insert(key, info);
        self.lru.push_back(key);

        // LRU eviction if cache too large
        if self.lru.len() > self.max_cache_size {
            if let Some(evict_key) = self.lru.pop_front() {
                self.cache.remove(&evict_key);
                // Note: We don't free atlas space on eviction (atlas is persistent)
                // Full atlas defrag happens on reset/resize
            }
        }

        Some(info)
    }

    /// Clear cache and reset atlas
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru.clear();
        self.allocator = GuillotineAllocator::new(self.atlas_width, self.atlas_height);
    }

    /// Get atlas dimensions
    pub fn atlas_size(&self) -> (u32, u32) {
        (self.atlas_width, self.atlas_height)
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            total_glyphs: self.cache.len(),
            atlas_utilization: self.allocator.utilization(),
        }
    }
}

impl Default for GlyphCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics
pub struct CacheStats {
    pub total_glyphs: usize,
    pub atlas_utilization: f32,
}

/// Guillotine bin-packing allocator
/// Implements the Guillotine algorithm for efficient rectangle packing
struct GuillotineAllocator {
    width: u32,
    height: u32,
    /// Free rectangles
    free_rects: Vec<Rect>,
}

impl GuillotineAllocator {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            free_rects: vec![Rect {
                x: 0,
                y: 0,
                width,
                height,
            }],
        }
    }

    /// Allocate rectangle using best-area-fit
    fn allocate(&mut self, width: u32, height: u32) -> Option<Rect> {
        // Find best-fitting free rectangle
        let mut best_idx = None;
        let mut best_area = u32::MAX;

        for (i, rect) in self.free_rects.iter().enumerate() {
            if rect.width >= width && rect.height >= height {
                let area = rect.width * rect.height;
                if area < best_area {
                    best_area = area;
                    best_idx = Some(i);
                }
            }
        }

        let best_idx = best_idx?;
        let free_rect = self.free_rects.remove(best_idx);

        // Allocated rectangle
        let allocated = Rect {
            x: free_rect.x,
            y: free_rect.y,
            width,
            height,
        };

        // Split remaining space (Guillotine algorithm)
        // Create two new free rectangles from the remaining space

        // Right split (if width < free_rect.width)
        if free_rect.width > width {
            self.free_rects.push(Rect {
                x: free_rect.x + width,
                y: free_rect.y,
                width: free_rect.width - width,
                height,
            });
        }

        // Bottom split (if height < free_rect.height)
        if free_rect.height > height {
            self.free_rects.push(Rect {
                x: free_rect.x,
                y: free_rect.y + height,
                width: free_rect.width,
                height: free_rect.height - height,
            });
        }

        Some(allocated)
    }

    /// Calculate atlas utilization (0.0-1.0)
    fn utilization(&self) -> f32 {
        let total_area = (self.width * self.height) as f32;
        let free_area: u32 = self.free_rects.iter().map(|r| r.width * r.height).sum();
        1.0 - (free_area as f32 / total_area)
    }
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guillotine_allocate() {
        let mut allocator = GuillotineAllocator::new(256, 256);

        let rect1 = allocator.allocate(64, 64).unwrap();
        assert_eq!(rect1.x, 0);
        assert_eq!(rect1.y, 0);

        let rect2 = allocator.allocate(64, 64).unwrap();
        // Should allocate next to first rect
        assert!(rect2.x == 64 || rect2.y == 64);
    }

    #[test]
    fn test_glyph_cache_insert() {
        let mut cache = GlyphCache::new();
        let key = GlyphKey {
            ch: 'A',
            bold: false,
            italic: false,
        };

        // Insert glyph
        let info = cache.insert(key, 16, 16).unwrap();
        assert!(info.tex_width > 0.0);

        // Lookup should succeed
        assert!(cache.lookup(key).is_some());
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = GlyphCache::new();
        cache.max_cache_size = 2; // Small cache for testing

        let key1 = GlyphKey {
            ch: 'A',
            bold: false,
            italic: false,
        };
        let key2 = GlyphKey {
            ch: 'B',
            bold: false,
            italic: false,
        };
        let key3 = GlyphKey {
            ch: 'C',
            bold: false,
            italic: false,
        };

        cache.insert(key1, 16, 16);
        cache.insert(key2, 16, 16);
        cache.insert(key3, 16, 16); // Should evict key1

        assert!(cache.lookup(key1).is_none()); // Evicted
        assert!(cache.lookup(key2).is_some());
        assert!(cache.lookup(key3).is_some());
    }
}
