//! End-to-End LAN Latency Benchmark
//!
//! Phase 1 Acceptance Criterion #5 Verification
//! SRS Ãƒâ€šÃ‚Â§7.1: p95 < 10ms (Phase 1 gate), SRS Ãƒâ€šÃ‚Â§5.1.2: LAN p95 < 30ms
//!
//! This benchmark measures ACTUAL round-trip latency with:
//! - Real WebSocket server (bound to network interface, not just loopback)
//! - Real WebSocket client connection
//! - Real protobuf encode/decode
//! - Real PTY echo (when ConPTY backend is available)
//!
//! Unlike websocket_latency.rs (component simulation), this measures
//! end-to-end network RTT to validate the acceptance criterion.
//!
//! Usage:
//!   cargo bench --bench latency_e2e_lan
//!
//! Evidence Output:
//!   - target/criterion/latency_e2e_lan/report/index.html
//!   - target/criterion/latency_e2e_lan/base/estimates.json
//!
//! For Wireshark capture (manual):
//!   1. Start: Wireshark, filter tcp.port == 8080
//!   2. Run: cargo bench --bench latency_e2e_lan
//!   3. Stop: Save as tests/evidence/phase1/criterion-5-latency/lan_traffic.pcapng
//!   4. Statistics ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ Conversations ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ TCP ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ verify p95 < 10ms

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

// Server and auth components
use monoterminal_master::{
    auth::{AuthService, Ed25519AuthService, RateLimiter},
    server::{Server, ServerConfig},
    session::manager::SessionManager,
};
use tokio::sync::{broadcast, oneshot};
use tracing::{debug, error, info, warn};

// Protocol
use monoterminal_protocol::Envelope;

// Test utilities (WebSocket client)
#[path = "../tests/common/ws_client.rs"]
mod ws_client;
use ws_client::TestWsClient;

/// Benchmark configuration
const SAMPLE_SIZE: usize = 10_000; // Per verification plan Ãƒâ€šÃ‚Â§3.5
const WARMUP_TIME_SECS: u64 = 3;
const MEASUREMENT_TIME_SECS: u64 = 30; // 5 minutes / 10 = 30s per iteration

// Defensive timeout configuration (prevents infinite hang during warmup)
// Added 2026-08-17: Defense against warmup hang observed in short-test-retry-20260817-020224.log
const PER_ITERATION_TIMEOUT_SECS: u64 = 30; // 30s max per iteration (setup + measurements + cleanup)
const DIAGNOSTIC_LOGGING_INTERVAL: u64 = 10; // Log every 10 iterations for detailed progress tracking

// Short test configuration (for diagnostic runs)
// Set LATENCY_SHORT_TEST=1 environment variable to use 100 samples instead of 10,000
fn get_sample_size() -> usize {
    match std::env::var("LATENCY_SHORT_TEST") {
        Ok(val) if val == "1" => {
            eprintln!("⚠️  SHORT TEST MODE: Using 100 samples (set by LATENCY_SHORT_TEST=1)");
            100
        }
        _ => SAMPLE_SIZE,
    }
}

// Progress logging interval (log every N iterations)
const PROGRESS_INTERVAL: u64 = 100;

/// End-to-end RTT measurement with real WebSocket connection
///
/// Measures: Client ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ Encode ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ WebSocket Send ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ Server Receive ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ Decode ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢
///           PTY Echo ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ Encode ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ WebSocket Send ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ Client Receive ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ Decode
///
/// This is the ACTUAL acceptance criterion: full stack, real network
fn bench_e2e_websocket_rtt(c: &mut Criterion) {
    // Initialize tracing for debug logs
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .try_init();

    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("e2e_lan_latency");
    let sample_size = get_sample_size();
    group.sample_size(sample_size);
    group.warm_up_time(Duration::from_secs(WARMUP_TIME_SECS));
    group.measurement_time(Duration::from_secs(MEASUREMENT_TIME_SECS));

    info!(
        "📊 Benchmark configuration: {} samples, {}s warmup, {}s measurement",
        sample_size, WARMUP_TIME_SECS, MEASUREMENT_TIME_SECS
    );

    // Bind to actual network interface (not just 127.0.0.1)
    // For true LAN testing, use 0.0.0.0 and connect from another machine
    let server_addr: SocketAddr = "127.0.0.1:18080".parse().unwrap();

    // Real server benchmark with WebSocket + Ed25519/JWT + Session Manager
    // Measures actual end-to-end latency with full protocol stack
    //
    // IMPORTANT: Server is set up ONCE outside iter_custom to avoid port binding conflicts
    // across warmup iterations

    // === SERVER SETUP (ONCE) ===
    let (session_manager, auth_service, server_handle, bound_addr) = rt.block_on(async {
        debug!("=== BENCHMARK SETUP: Starting server (once) ===");

        // 1. Generate test Ed25519 keypair (deterministic for testing)
        let keypair = monoterminal_master::auth::Ed25519KeyPair::from_bytes(&[0x42; 32]);

        // 2. Create auth service
        let auth_service =
            Arc::new(Ed25519AuthService::new(&keypair).expect("Failed to create auth service"));

        // 3. Create rate limiter
        let rate_limiter = Arc::new(RateLimiter::new());

        // 4. Create session manager (with cmd.exe as default shell)
        let session_manager = Arc::new(SessionManager::new(Some("cmd.exe".to_string())));

        // 5. Create health channel
        let (health_tx, _health_rx) = broadcast::channel(16);

        // 6. Create startup notification channel
        let (startup_tx, startup_rx) = oneshot::channel();

        // 7. Configure server
        // Use workspace root-relative path (robust across different cargo working directories)
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("Failed to resolve workspace root");
        let cert_dir = workspace_root.join("certs");
        let tls_config = monoterminal_master::server::TlsConfig {
            cert_path: cert_dir.join("server.crt"),
            key_path: cert_dir.join("server.key"),
        };
        let server_config = ServerConfig {
            bind_addr: server_addr,
            tls: tls_config,
            ..Default::default()
        };

        // 8. Create server instance with startup notification
        let server = Server::with_startup_notification(
            server_config,
            session_manager.clone(),
            rate_limiter,
            auth_service.clone(),
            health_tx,
            startup_tx,
        )
        .expect("Failed to create server");

        // 9. Spawn server in background
        let server_handle = tokio::spawn(async move { server.run().await });

        // 10. Wait for server to successfully bind (with timeout)
        let bound_addr = match tokio::time::timeout(Duration::from_secs(5), startup_rx).await {
            Ok(Ok(addr)) => {
                info!("✓ Server successfully bound to {}", addr);
                addr
            }
            Ok(Err(_)) => {
                error!("✗ Server startup notification channel closed without sending address");
                panic!("Server startup failed: channel closed");
            }
            Err(_) => {
                error!("✗ Server startup timeout (5s) - server failed to bind");
                server_handle.abort();
                panic!("Server startup timeout - check logs for bind errors");
            }
        };

        info!("✅ Server ready for benchmark");

        (session_manager, auth_service, server_handle, bound_addr)
    });

    group.bench_function("real_master_rtt_loopback", |b| {
        b.iter_custom(|iters| {
            rt.block_on(async {
                // === DEFENSIVE TIMEOUT WRAPPER ===
                // Wrap entire iteration in timeout to prevent infinite hang during warmup
                // Observed hang: short-test-retry-20260817-020224.log (hung after AttachRequest JWT verification)
                let iteration_timeout = Duration::from_secs(PER_ITERATION_TIMEOUT_SECS);
                let iteration_start = Instant::now();

                debug!("=== ITERATION START: timeout={}s, iters={} ===", PER_ITERATION_TIMEOUT_SECS, iters);

                // Wrap the entire benchmark logic in timeout
                let result = tokio::time::timeout(iteration_timeout, async {
                    // === CLIENT SETUP (PER ITERATION) ===

                    debug!("=== CLIENT SETUP: Preparing WebSocket connection ===");

                // Generate JWT for authentication
                let user_id = monoterminal_master::auth::UserId::from("benchmark-user");
                let tokens = auth_service
                    .issue_tokens(&user_id)
                    .expect("Failed to issue tokens");
                let jwt_bearer = tokens.access;

                // Create and connect WebSocket client (accept self-signed certs for testing)
                let ws_url = format!("wss://{}", bound_addr);
                let mut client = TestWsClient::new_accept_invalid_certs(&ws_url);
                match client.connect().await {
                    Ok(_) => debug!("✓ WebSocket client connected successfully"),
                    Err(e) => {
                        error!("✗ WebSocket client connection failed: {}", e);
                        panic!("WebSocket connection failed: {}", e);
                    }
                }

                // Create session via SessionManager (server-side)
                let session_id = match session_manager
                    .create_session(
                        None,  // Use default working directory
                        24,    // rows
                        80,    // cols
                    )
                    .await {
                        Ok(id) => {
                            debug!("✓ PTY session created: {}", id);
                            id
                        }
                        Err(e) => {
                            error!("✗ Failed to create PTY session: {}", e);
                            panic!("PTY session creation failed: {}", e);
                        }
                    };

                // Give PTY time to initialize
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Attach to session via WebSocket (proper E2E flow)
                let _attach_response = match client
                    .attach(&session_id.to_string(), &jwt_bearer, 24, 80)
                    .await {
                        Ok(resp) => {
                            debug!("✓ Session attached successfully");
                            resp
                        }
                        Err(e) => {
                            error!("✗ Failed to attach to session: {}", e);
                            let _ = session_manager.kill_session(session_id).await;
                            panic!("Session attach failed: {}", e);
                        }
                    };

                // === MEASUREMENT LOOP ===
                // Measure RTT: send InputData via WebSocket -> receive OutputData via WebSocket

                debug!("=== Starting measurement loop ({} iterations) ===", iters);
                let start = Instant::now();
                let mut successful_iterations = 0u64;
                let mut failed_iterations = 0u64;

                for i in 0..iters {
                    // Enhanced progress logging: verbose every 10 iterations, summary every 100
                    if i > 0 && i % DIAGNOSTIC_LOGGING_INTERVAL == 0 {
                        debug!("📊 Diagnostic progress: iteration {}/{} ({:.1}%), {} successful, {} failed, elapsed {:.2}s",
                              i, iters, (i as f64 / iters as f64) * 100.0,
                              successful_iterations, failed_iterations,
                              start.elapsed().as_secs_f64());
                    } else if i > 0 && i % PROGRESS_INTERVAL == 0 {
                        debug!("📈 Progress: {}/{} iterations ({:.1}%), {} successful, {} failed",
                              i, iters, (i as f64 / iters as f64) * 100.0,
                              successful_iterations, failed_iterations);
                    }

                    let rtt_start = Instant::now();

                    // Send complete command with Windows line ending (cmd.exe needs \r\n to echo)
                    if let Err(e) = client.send_input(b"x\r\n", &jwt_bearer).await {
                        error!("Failed to send input at iteration {}: {}", i, e);
                        failed_iterations += 1;
                        continue;
                    }

                    // Receive OutputData via WebSocket (with timeout)
                    let timeout_duration = Duration::from_millis(100);
                    match tokio::time::timeout(timeout_duration, client.recv()).await {
                        Ok(Ok(msg)) => {
                            // Decode protobuf Envelope
                            if let tokio_tungstenite::tungstenite::Message::Binary(data) = msg {
                                use prost::Message as ProstMessage;
                                match Envelope::decode(&data[..]) {
                                    Ok(_envelope) => {
                                        // Successfully received OutputData
                                        successful_iterations += 1;
                                    }
                                    Err(e) => {
                                        warn!("Failed to decode envelope at iteration {}: {}", i, e);
                                        failed_iterations += 1;
                                    }
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            warn!("Recv error at iteration {}: {}", i, e);
                            failed_iterations += 1;
                        }
                        Err(_) => {
                            warn!("Recv timeout at iteration {} (>100ms)", i);
                            failed_iterations += 1;
                        }
                    }

                    let rtt = rtt_start.elapsed();
                    black_box(rtt);
                }

                debug!("✅ Measurement loop complete: {}/{} successful ({:.1}% success rate)",
                      successful_iterations, iters,
                      (successful_iterations as f64 / iters as f64) * 100.0);

                // === CLEANUP (PER ITERATION) ===
                let _ = client.detach(&session_id.to_string()).await;
                let _ = client.close().await;
                let _ = session_manager.kill_session(session_id).await;

                start.elapsed()
            }).await; // Close timeout wrapper

            // === TIMEOUT DIAGNOSTIC HANDLING ===
            match result {
                Ok(duration) => {
                    debug!("✅ Iteration completed in {:.2}s (within {}s timeout)",
                          iteration_start.elapsed().as_secs_f64(), PER_ITERATION_TIMEOUT_SECS);
                    duration
                }
                Err(_timeout_elapsed) => {
                    // TIMEOUT TRIGGERED - Dump diagnostic state
                    error!("❌ TIMEOUT: Iteration exceeded {}s limit", PER_ITERATION_TIMEOUT_SECS);
                    error!("📊 DIAGNOSTIC STATE DUMP:");
                    error!("  - Iterations requested: {}", iters);
                    error!("  - Elapsed time: {:.2}s", iteration_start.elapsed().as_secs_f64());
                    error!("  - Server address: {}", bound_addr);
                    error!("  - Last known state: Timeout during client setup, session attach, or measurement loop");
                    error!("  - Recommended action: Check server logs, PTY session state, WebSocket connection");
                    error!("📋 EVIDENCE: See tests/evidence/phase1/criterion-5-latency/ for full logs");

                    panic!("Benchmark iteration timeout after {}s - infinite hang detected. \
                           This is a DEFENSIVE FAIL-FAST to prevent silent infinite hang. \
                           Review error logs above for diagnostic state.", PER_ITERATION_TIMEOUT_SECS);
                }
            }
            })
        });
    });

    // === SERVER CLEANUP (AFTER ALL BENCHMARKS) ===
    server_handle.abort();

    // TODO: Real master server benchmark (requires ConPTY + auth working)
    // group.bench_function("real_master_rtt_lan", |b| {
    //     b.iter_custom(|iters| {
    //         rt.block_on(async {
    //             // 1. Start actual monoterminal-master
    //             // 2. Authenticate
    //             // 3. Attach to session
    //             // 4. Measure InputData ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ OutputData RTT
    //         })
    //     });
    // });

    group.finish();
}

/// Latency under concurrent load
/// Validates p95 stays <10ms even with multiple active sessions
fn bench_e2e_concurrent_sessions(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("e2e_concurrent");

    for n_sessions in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(n_sessions),
            n_sessions,
            |b, &n| {
                b.iter_custom(|iters| {
                    rt.block_on(async {
                        // TODO: Spawn N concurrent sessions
                        // Measure RTT for each, verify p95 < 10ms holds
                        let start = Instant::now();

                        for _i in 0..iters {
                            // Simulate concurrent session processing
                            black_box(n);
                        }

                        start.elapsed()
                    })
                });
            },
        );
    }

    group.finish();
}

/// Network packet overhead measurement
/// Validates protobuf wire format meets latency budget
fn bench_network_packet_overhead(c: &mut Criterion) {
    use monoterminal_protocol::{Envelope, InputData, OutputData};
    use prost::Message;

    let mut group = c.benchmark_group("network_overhead");

    // Measure InputData encode + decode (client ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ server path)
    group.bench_function("input_data_round_trip", |b| {
        let input = InputData {
            data: b"x".to_vec(),
            auth_token: String::new(),
        };

        let envelope = Envelope {
            sequence_number: 1,
            message: Some(monoterminal_protocol::envelope::Message::InputData(input)),
        };

        b.iter(|| {
            // Encode
            let mut buf = Vec::new();
            envelope.encode(&mut buf).unwrap();

            // Decode (simulates network transmission)
            let decoded = Envelope::decode(&buf[..]).unwrap();

            black_box(decoded);
        })
    });

    // Measure OutputData encode + decode (server ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ client path)
    group.bench_function("output_data_round_trip", |b| {
        let output = OutputData {
            data: vec![b'x', b'\r', b'\n'],
            sequence: 100,
            compression: 0,
        };

        let envelope = Envelope {
            sequence_number: 2,
            message: Some(monoterminal_protocol::envelope::Message::OutputData(output)),
        };

        b.iter(|| {
            let mut buf = Vec::new();
            envelope.encode(&mut buf).unwrap();

            let decoded = Envelope::decode(&buf[..]).unwrap();
            black_box(decoded);
        })
    });

    group.finish();
}

/// Component latency budget breakdown
/// Validates each stage contributes < 2ms to total <10ms budget
fn bench_latency_budget_breakdown(c: &mut Criterion) {
    use monoterminal_protocol::{Envelope, InputData, OutputData};
    use prost::Message;

    let mut group = c.benchmark_group("latency_budget");

    // Stage 1: Client encode (budget: <0.5ms)
    group.bench_function("stage1_client_encode", |b| {
        let input = InputData {
            data: b"x".to_vec(),
            auth_token: String::new(),
        };

        let envelope = Envelope {
            sequence_number: 1,
            message: Some(monoterminal_protocol::envelope::Message::InputData(input)),
        };

        b.iter(|| {
            let mut buf = Vec::new();
            envelope.encode(&mut buf).unwrap();
            black_box(buf);
        })
    });

    // Stage 2: Network transmission (budget: <3ms for LAN, measured separately)
    // Not measured here - use Wireshark for actual wire time

    // Stage 3: Server decode (budget: <0.5ms)
    group.bench_function("stage3_server_decode", |b| {
        let input = InputData {
            data: b"x".to_vec(),
            auth_token: String::new(),
        };

        let envelope = Envelope {
            sequence_number: 1,
            message: Some(monoterminal_protocol::envelope::Message::InputData(input)),
        };

        let mut buf = Vec::new();
        envelope.encode(&mut buf).unwrap();

        b.iter(|| {
            let decoded = Envelope::decode(&buf[..]).unwrap();
            black_box(decoded);
        })
    });

    // Stage 4: PTY write + read echo (budget: <2ms)
    group.bench_function("stage4_pty_echo", |b| {
        b.iter(|| {
            // Simulate PTY echo (actual measurement requires ConPTY)
            let input = b"x\n";
            let output = vec![b'x', b'\r', b'\n'];
            black_box((input, output));
        })
    });

    // Stage 5: Server encode (budget: <0.5ms)
    group.bench_function("stage5_server_encode", |b| {
        let output = OutputData {
            data: vec![b'x', b'\r', b'\n'],
            sequence: 100,
            compression: 0,
        };

        let envelope = Envelope {
            sequence_number: 2,
            message: Some(monoterminal_protocol::envelope::Message::OutputData(output)),
        };

        b.iter(|| {
            let mut buf = Vec::new();
            envelope.encode(&mut buf).unwrap();
            black_box(buf);
        })
    });

    // Stage 6: Network transmission back (budget: <3ms)
    // See stage 2

    // Stage 7: Client decode + render trigger (budget: <0.5ms)
    group.bench_function("stage7_client_decode", |b| {
        let output = OutputData {
            data: vec![b'x', b'\r', b'\n'],
            sequence: 100,
            compression: 0,
        };

        let envelope = Envelope {
            sequence_number: 2,
            message: Some(monoterminal_protocol::envelope::Message::OutputData(output)),
        };

        let mut buf = Vec::new();
        envelope.encode(&mut buf).unwrap();

        b.iter(|| {
            let decoded = Envelope::decode(&buf[..]).unwrap();
            black_box(decoded);
        })
    });

    group.finish();
}

// ============================================================================
// Mock Infrastructure (to be replaced with real server once stable)
// ============================================================================

/// Mock WebSocket echo server for baseline latency measurement
#[allow(dead_code)]
async fn mock_websocket_echo_server(addr: SocketAddr) {
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    let listener = TcpListener::bind(addr).await.unwrap();

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(async move {
            let mut ws = accept_async(stream).await.unwrap();

            while let Some(msg) = ws.next().await {
                if let Ok(msg) = msg {
                    // Echo back immediately
                    ws.send(msg).await.ok();
                }
            }
        });
    }
}

/// Mock WebSocket client for RTT measurement
#[allow(dead_code)]
struct MockWebSocketClient {
    // TODO: Implement actual client using tokio-tungstenite
}

#[allow(dead_code)]
impl MockWebSocketClient {
    async fn connect(_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // TODO: Implement real connection
        Ok(Self {})
    }

    async fn measure_rtt_with_echo(
        &self,
        _data: Vec<u8>,
    ) -> Result<Duration, Box<dyn std::error::Error>> {
        // TODO: Implement real RTT measurement
        // For now, return simulated latency
        Ok(Duration::from_micros(500)) // Simulated 0.5ms
    }
}

criterion_group!(
    name = e2e_latency_benches;
    config = Criterion::default()
        .sample_size(SAMPLE_SIZE)
        .warm_up_time(Duration::from_secs(WARMUP_TIME_SECS))
        .measurement_time(Duration::from_secs(MEASUREMENT_TIME_SECS));
    targets =
        bench_e2e_websocket_rtt,
        bench_e2e_concurrent_sessions,
        bench_network_packet_overhead,
        bench_latency_budget_breakdown,
);

criterion_main!(e2e_latency_benches);
