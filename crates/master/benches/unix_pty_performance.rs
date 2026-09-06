// Unix PTY Backend Performance Benchmarks
// Phase 3 Week 1 Day 3
//
// Validates SRS performance targets:
// - PTY creation time: <100ms (SRS §6.1)
// - Read/write throughput with 4KB buffer (SRS §3.1.4)
// - Resize latency: <10ms
//
// Compares Unix PTY vs Windows ConPTY performance

#![cfg(unix)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use monoterminal_master::pty::{PtyBackend, PtyConfig, UnixPtyBackend};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Puts the shell's controlling terminal into raw mode with echo disabled.
///
/// Required before benchmarking raw `write()` throughput: in the default
/// cooked/echo mode, the kernel tty driver echoes every input byte back into
/// the PTY's output buffer. Nobody drains that buffer during a write
/// benchmark, so it fills and subsequent writes block indefinitely. Raw mode
/// with echo off avoids generating that output in the first place.
async fn disable_echo(pty: &mut UnixPtyBackend) {
    pty.write(b"stty raw -echo\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
}

/// Benchmark PTY creation time
/// Target: <100ms per SRS §6.1
fn bench_pty_creation(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("unix_pty_creation");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50); // Fewer samples for slow operations

    group.bench_function("create_pty", |b| {
        b.to_async(&runtime).iter(|| async {
            let config = PtyConfig {
                rows: 24,
                cols: 80,
                shell: "/bin/sh".to_string(),
                working_dir: PathBuf::from("/tmp"),
                environment: HashMap::new(),
            };

            let pty = UnixPtyBackend::create(config).await.unwrap();
            black_box(pty)
        })
    });

    group.finish();
}

/// Benchmark read throughput with 4KB buffer (SRS §3.1.4)
fn bench_read_throughput(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("unix_pty_read");
    group.measurement_time(Duration::from_secs(10));

    // Create PTY once for all read benchmarks
    let config = PtyConfig {
        rows: 24,
        cols: 80,
        shell: "/bin/sh".to_string(),
        working_dir: PathBuf::from("/tmp"),
        environment: HashMap::new(),
    };

    let pty = runtime.block_on(async {
        let mut p = UnixPtyBackend::create(config).await.unwrap();

        // Write command that generates continuous output (never runs dry mid-benchmark)
        p.write(b"yes\n").await.unwrap();

        // Wait for command to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        p
    });
    let pty = Arc::new(Mutex::new(pty));

    // Benchmark reading with 4KB buffer (SRS §3.1.4)
    group.throughput(Throughput::Bytes(4096));
    group.bench_function("read_4kb_buffer", |b| {
        let pty = pty.clone();
        b.to_async(&runtime).iter(move || {
            let pty = pty.clone();
            async move {
                let mut buf = vec![0u8; 4096];
                let n = pty.lock().await.read(&mut buf).await.unwrap_or(0);
                black_box(n)
            }
        })
    });

    group.finish();
}

/// Benchmark write throughput
fn bench_write_throughput(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("unix_pty_write");
    group.measurement_time(Duration::from_secs(10));

    let config = PtyConfig {
        rows: 24,
        cols: 80,
        shell: "/bin/sh".to_string(),
        working_dir: PathBuf::from("/tmp"),
        environment: HashMap::new(),
    };

    let mut pty = runtime.block_on(async { UnixPtyBackend::create(config).await.unwrap() });
    runtime.block_on(disable_echo(&mut pty));
    let pty = Arc::new(Mutex::new(pty));

    // Test different write sizes
    for size in [64, 256, 1024, 4096].iter() {
        let data = vec![b'A'; *size];

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            let pty = pty.clone();
            b.to_async(&runtime).iter(move || {
                let pty = pty.clone();
                let data = data.clone();
                async move {
                    pty.lock().await.write(black_box(&data)).await.unwrap();
                }
            })
        });
    }

    group.finish();
}

/// Benchmark resize latency
/// Target: <10ms (SRS §6.1)
fn bench_resize_latency(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("unix_pty_resize");
    group.measurement_time(Duration::from_secs(5));

    let config = PtyConfig {
        rows: 24,
        cols: 80,
        shell: "/bin/sh".to_string(),
        working_dir: PathBuf::from("/tmp"),
        environment: HashMap::new(),
    };

    let mut pty = runtime.block_on(async { UnixPtyBackend::create(config).await.unwrap() });

    group.bench_function("resize", |b| {
        let mut rows = 24u16;
        let mut cols = 80u16;

        b.iter(|| {
            // Alternate between different sizes
            rows = if rows == 24 { 40 } else { 24 };
            cols = if cols == 80 { 120 } else { 80 };

            pty.resize(black_box(rows), black_box(cols)).unwrap();
        })
    });

    group.finish();
}

/// Benchmark concurrent operations (write + resize)
fn bench_concurrent_operations(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("unix_pty_concurrent");
    group.measurement_time(Duration::from_secs(10));

    let config = PtyConfig {
        rows: 24,
        cols: 80,
        shell: "/bin/sh".to_string(),
        working_dir: PathBuf::from("/tmp"),
        environment: HashMap::new(),
    };

    let mut pty = runtime.block_on(async { UnixPtyBackend::create(config).await.unwrap() });
    runtime.block_on(disable_echo(&mut pty));
    let pty = Arc::new(Mutex::new(pty));

    group.bench_function("write_and_resize", |b| {
        let pty = pty.clone();
        let rows = Arc::new(Mutex::new(24u16));

        b.to_async(&runtime).iter(move || {
            let pty = pty.clone();
            let rows = rows.clone();
            async move {
                // Write data (no trailing newline: the shell is in raw mode
                // from disable_echo(), so this never triggers command
                // execution/output that would go unread and fill the buffer)
                pty.lock().await.write(b"echo test").await.unwrap();

                // Resize immediately after
                let mut rows = rows.lock().await;
                *rows = if *rows == 24 { 40 } else { 24 };
                pty.lock().await.resize(black_box(*rows), 80).unwrap();
            }
        })
    });

    group.finish();
}

/// Benchmark shell process spawning overhead
fn bench_shell_spawn_overhead(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("unix_pty_shell_spawn");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    // Test different shells (if available)
    for shell in ["/bin/sh", "/bin/bash"].iter() {
        // Check if shell exists
        if !std::path::Path::new(shell).exists() {
            continue;
        }

        group.bench_with_input(BenchmarkId::from_parameter(shell), shell, |b, shell| {
            b.to_async(&runtime).iter(|| async {
                let config = PtyConfig {
                    rows: 24,
                    cols: 80,
                    shell: shell.to_string(),
                    working_dir: PathBuf::from("/tmp"),
                    environment: HashMap::new(),
                };

                let pty = UnixPtyBackend::create(config).await.unwrap();

                // Immediately terminate to measure spawn overhead only
                Box::new(pty).terminate().await.unwrap();
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_pty_creation,
    bench_read_throughput,
    bench_write_throughput,
    bench_resize_latency,
    bench_concurrent_operations,
    bench_shell_spawn_overhead,
);

criterion_main!(benches);
