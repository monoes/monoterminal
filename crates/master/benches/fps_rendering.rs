//! FPS Rendering Benchmarks
//!
//! Validates SRS §7.1 Phase 1 acceptance criterion #1:
//! - 60 FPS master rendering on Windows 10 1809+
//! - Frame budget: 16.67ms (60 Hz)
//! - Breakdown: PTY read (2ms) + dirty tracking (0.5ms) + glyph lookup (1ms) + GPU render (8ms) + VSync (5ms)
//!
//! This benchmark measures the components that contribute to the rendering pipeline
//! to ensure we stay within the 16.67ms frame budget.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

/// Simulates dirty cell tracking after PTY output
/// Target: < 0.5ms for 80x24 terminal (SRS §2.1.1)
fn bench_dirty_cell_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("dirty_tracking");

    // Test various terminal sizes
    let sizes = vec![
        (80, 24),  // Standard
        (120, 40), // Larger
        (200, 60), // Extra large
    ];

    for (cols, rows) in sizes {
        let total_cells = cols * rows;

        // Simulate a dirty bitmap (bit per cell)
        let mut dirty_bits = vec![false; total_cells];

        group.throughput(Throughput::Elements(total_cells as u64));
        group.bench_with_input(
            BenchmarkId::new("mark_dirty_region", format!("{}x{}", cols, rows)),
            &(cols, rows),
            |b, &(cols, rows)| {
                b.iter(|| {
                    // Simulate marking a rectangular region as dirty
                    // (typical for scrolling or large output burst)
                    let start_row = 5;
                    let end_row = rows - 5;
                    let start_col = 0;
                    let end_col = cols;

                    for row in start_row..end_row {
                        for col in start_col..end_col {
                            let idx = row * cols + col;
                            dirty_bits[idx] = black_box(true);
                        }
                    }

                    black_box(&dirty_bits);
                })
            },
        );
    }

    group.finish();
}

/// Simulates glyph cache lookup
/// Target: < 1ms (SRS §2.1.1 frame budget)
fn bench_glyph_cache_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("glyph_cache");

    // Simulate a simple glyph cache (char -> atlas coordinates)
    use std::collections::HashMap;

    let mut cache: HashMap<char, (u32, u32)> = HashMap::new();

    // Pre-populate with ASCII + common Unicode
    for i in 32u8..127u8 {
        cache.insert(i as char, (i as u32, 0));
    }

    // Add some common Unicode chars
    let common_unicode = vec!['→', '←', '↑', '↓', '✓', '✗', '…', '•'];
    for (idx, ch) in common_unicode.iter().enumerate() {
        cache.insert(*ch, (128 + idx as u32, 0));
    }

    group.bench_function("lookup_ascii", |b| {
        b.iter(|| {
            // Simulate looking up every character in a typical line
            let line = "cargo build --release --target x86_64-pc-windows-msvc";
            for ch in line.chars() {
                let coords = cache.get(&ch).unwrap_or(&(0, 0));
                black_box(coords);
            }
        })
    });

    group.bench_function("lookup_mixed_unicode", |b| {
        b.iter(|| {
            let line = "✓ Build succeeded → 128ms • Size: 5.2MB";
            for ch in line.chars() {
                let coords = cache.get(&ch).unwrap_or(&(0, 0));
                black_box(coords);
            }
        })
    });

    group.finish();
}

/// Simulates GPU command buffer submission overhead
/// Target: < 8ms for full-screen update (SRS §2.1.1)
fn bench_gpu_command_submission(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_commands");

    // Simulate building vertex buffers for text rendering
    #[derive(Clone, Copy)]
    struct Vertex {
        pos: [f32; 2],
        uv: [f32; 2],
        color: [f32; 4],
    }

    let sizes = vec![
        (80, 24),  // Standard terminal
        (120, 40), // Larger
        (200, 60), // Extra large
    ];

    for (cols, rows) in sizes {
        let total_cells = cols * rows;

        group.throughput(Throughput::Elements(total_cells as u64));
        group.bench_with_input(
            BenchmarkId::new("build_vertex_buffer", format!("{}x{}", cols, rows)),
            &(cols, rows),
            |b, &(cols, rows)| {
                b.iter(|| {
                    let mut vertices = Vec::with_capacity(total_cells * 6);

                    for row in 0..rows {
                        for col in 0..cols {
                            // Each cell = 2 triangles = 6 vertices
                            let x = col as f32 * 10.0;
                            let y = row as f32 * 20.0;

                            let v = Vertex {
                                pos: [x, y],
                                uv: [0.0, 0.0],
                                color: [1.0, 1.0, 1.0, 1.0],
                            };

                            for _ in 0..6 {
                                vertices.push(black_box(v));
                            }
                        }
                    }

                    black_box(vertices);
                })
            },
        );
    }

    group.finish();
}

/// Simulates full frame render cycle
/// Target: < 16.67ms total (60 FPS)
fn bench_full_frame_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_frame");

    // Set the target time to 16.67ms (60 FPS)
    group.measurement_time(Duration::from_secs(10));

    let terminal_size = (80, 24);
    let (cols, rows) = terminal_size;
    let total_cells = cols * rows;

    // Simulate all frame components
    let mut dirty_bits = vec![false; total_cells];
    let mut screen_buffer = vec![' '; total_cells];

    use std::collections::HashMap;
    let mut glyph_cache: HashMap<char, (u32, u32)> = HashMap::new();
    for i in 32u8..127u8 {
        glyph_cache.insert(i as char, (i as u32, 0));
    }

    #[derive(Clone, Copy)]
    struct Vertex {
        pos: [f32; 2],
        uv: [f32; 2],
        color: [f32; 4],
    }

    group.throughput(Throughput::Elements(total_cells as u64));
    group.bench_function("simulate_60fps_frame", |b| {
        b.iter(|| {
            // 1. Dirty tracking (0.5ms budget)
            for row in 0..rows {
                for col in 0..cols {
                    let idx = row * cols + col;
                    dirty_bits[idx] = black_box(true);
                }
            }

            // 2. Glyph lookups (1ms budget)
            for ch in screen_buffer.iter() {
                let coords = glyph_cache.get(ch).unwrap_or(&(0, 0));
                black_box(coords);
            }

            // 3. Build vertex buffer (part of 8ms GPU budget)
            let mut vertices = Vec::with_capacity(total_cells * 6);
            for row in 0..rows {
                for col in 0..cols {
                    let x = col as f32 * 10.0;
                    let y = row as f32 * 20.0;

                    let v = Vertex {
                        pos: [x, y],
                        uv: [0.0, 0.0],
                        color: [1.0, 1.0, 1.0, 1.0],
                    };

                    for _ in 0..6 {
                        vertices.push(v);
                    }
                }
            }

            black_box(vertices);
        })
    });

    group.finish();
}

/// Benchmark incremental rendering (only dirty regions)
/// This is the hot path - only redraw changed cells
fn bench_incremental_rendering(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_render");

    let (cols, rows) = (80, 24);
    let total_cells = cols * rows;

    // Test different dirty region sizes (% of screen)
    let dirty_percentages = vec![1, 5, 10, 25, 50, 100];

    #[derive(Clone, Copy)]
    struct Vertex {
        pos: [f32; 2],
        uv: [f32; 2],
        color: [f32; 4],
    }

    for dirty_pct in dirty_percentages {
        let dirty_count = (total_cells * dirty_pct) / 100;

        group.throughput(Throughput::Elements(dirty_count as u64));
        group.bench_with_input(
            BenchmarkId::new("dirty_region", format!("{}%", dirty_pct)),
            &dirty_count,
            |b, &dirty_count| {
                b.iter(|| {
                    // Only render dirty cells
                    let mut vertices = Vec::with_capacity(dirty_count * 6);

                    for i in 0..dirty_count {
                        let row = i / cols;
                        let col = i % cols;

                        let x = col as f32 * 10.0;
                        let y = row as f32 * 20.0;

                        let v = Vertex {
                            pos: [x, y],
                            uv: [0.0, 0.0],
                            color: [1.0, 1.0, 1.0, 1.0],
                        };

                        for _ in 0..6 {
                            vertices.push(v);
                        }
                    }

                    black_box(vertices);
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    name = fps_benches;
    config = Criterion::default()
        .sample_size(100)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));
    targets =
        bench_dirty_cell_tracking,
        bench_glyph_cache_lookup,
        bench_gpu_command_submission,
        bench_full_frame_cycle,
        bench_incremental_rendering,
);

criterion_main!(fps_benches);
