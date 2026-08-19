//! PTY throughput benchmarks
//!
//! Validates SRS §6.1 PTY read performance:
//! - Raw read throughput (MB/s)
//! - UTF-8 validation overhead
//! - Ring buffer append performance

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::VecDeque;

/// Simulates ring buffer operations for scrollback
fn bench_ring_buffer_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer");

    const CAPACITY: usize = 10_000;
    let mut buffer: VecDeque<String> = VecDeque::with_capacity(CAPACITY);

    group.bench_function("append_with_eviction", |b| {
        b.iter(|| {
            let line = "This is a typical terminal line with some ANSI codes \x1b[32mgreen\x1b[0m";

            if buffer.len() >= CAPACITY {
                buffer.pop_front();
            }
            buffer.push_back(black_box(line.to_string()));
        })
    });

    group.finish();
}

/// Simulates UTF-8 validation of PTY output
fn bench_utf8_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("utf8_validation");

    // Various sizes of UTF-8 data
    for size in [256, 1024, 4096, 16384].iter() {
        let data = "Hello, 世界! 🚀 Terminal output\n".repeat(*size / 32);
        let bytes = data.as_bytes();

        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &bytes, |b, bytes| {
            b.iter(|| {
                let validated = std::str::from_utf8(black_box(bytes)).unwrap();
                black_box(validated);
            })
        });
    }

    group.finish();
}

/// Simulates VT sequence parsing (simplified)
fn bench_ansi_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("ansi_parsing");

    let input = "\x1b[32mGreen text\x1b[0m Normal \x1b[1;31mBold Red\x1b[0m\n".repeat(100);

    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("strip_ansi", |b| {
        b.iter(|| {
            // Simplified ANSI stripping - just count escapes
            let count = input.matches('\x1b').count();
            black_box(count);
        })
    });

    group.finish();
}

/// Simulates scrollback retrieval (lines/sec)
fn bench_scrollback_retrieval(c: &mut Criterion) {
    let mut group = c.benchmark_group("scrollback_retrieval");

    // Prepare scrollback buffer
    const LINES: usize = 10_000;
    let scrollback: Vec<String> = (0..LINES)
        .map(|i| {
            format!(
                "Line {:05} - typical terminal output with some content here",
                i
            )
        })
        .collect();

    // Benchmark retrieving chunks
    for chunk_size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*chunk_size as u64));
        group.bench_with_input(
            BenchmarkId::new("retrieve_lines", chunk_size),
            chunk_size,
            |b, &size| {
                b.iter(|| {
                    let start = 0;
                    let end = size.min(scrollback.len());
                    let chunk = &scrollback[start..end];
                    black_box(chunk);
                })
            },
        );
    }

    group.finish();
}

/// Simulates compression of scrollback lines (zstd level 3)
fn bench_scrollback_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("scrollback_compression");

    // 1000 lines to compress (batch)
    let lines: Vec<String> = (0..1000)
        .map(|i| format!("Line {:05} - terminal output content here\n", i))
        .collect();
    let joined = lines.join("");
    let bytes = joined.as_bytes();

    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function("compress_1000_lines", |b| {
        b.iter(|| {
            let compressed = zstd::bulk::compress(black_box(bytes), 3).unwrap();
            black_box(compressed);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_ring_buffer_append,
    bench_utf8_validation,
    bench_ansi_parsing,
    bench_scrollback_retrieval,
    bench_scrollback_compression,
);

criterion_main!(benches);
