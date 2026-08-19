//! Protocol codec benchmarks
//!
//! Validates SRS §6.1 performance targets:
//! - Protocol encode/decode throughput (messages/sec)
//! - Compression overhead (zstd level 3)

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use monoterminal_protocol::{
    AttachRequest, AttachResponse, Envelope, IceCandidate, OutputData, SessionMetadata,
    TurnCredentials, WebRtcAnswer, WebRtcOffer,
};
use prost::Message;

fn bench_encode_attach_request(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");

    let request = AttachRequest {
        session_id: "test-session-12345".to_string(),
        auth_token: "BENCHMARK_FAKE_JWT_PAYLOAD".to_string(),
        rows: 24,
        cols: 80,
        last_seen_sequence: 0,
    };

    let envelope = Envelope {
        sequence_number: 1,
        message: Some(monoterminal_protocol::envelope::Message::AttachRequest(
            request,
        )),
    };

    group.bench_function("attach_request", |b| {
        b.iter(|| {
            let mut buf = Vec::new();
            envelope.encode(&mut buf).unwrap();
            black_box(buf);
        })
    });

    group.finish();
}

fn bench_decode_attach_request(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");

    let request = AttachRequest {
        session_id: "test-session-12345".to_string(),
        auth_token: "BENCHMARK_FAKE_JWT_PAYLOAD".to_string(),
        rows: 24,
        cols: 80,
        last_seen_sequence: 0,
    };

    let envelope = Envelope {
        sequence_number: 1,
        message: Some(monoterminal_protocol::envelope::Message::AttachRequest(
            request,
        )),
    };

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    group.bench_function("attach_request", |b| {
        b.iter(|| {
            let decoded = Envelope::decode(black_box(&buf[..])).unwrap();
            black_box(decoded);
        })
    });

    group.finish();
}

fn bench_encode_output_data(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_output");

    // Simulate typical terminal output sizes
    for size in [256, 1024, 4096, 16384].iter() {
        let output_data = OutputData {
            data: vec![b'A'; *size],
            sequence: 100,
            compression: 0, // NONE
        };

        let envelope = Envelope {
            sequence_number: 1,
            message: Some(monoterminal_protocol::envelope::Message::OutputData(
                output_data,
            )),
        };

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(format!("{}_bytes", size), size, |b, _| {
            b.iter(|| {
                let mut buf = Vec::new();
                envelope.encode(&mut buf).unwrap();
                black_box(buf);
            })
        });
    }

    group.finish();
}

fn bench_decode_output_data(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_output");

    // Simulate typical terminal output sizes
    for size in [256, 1024, 4096, 16384].iter() {
        let output_data = OutputData {
            data: vec![b'A'; *size],
            sequence: 100,
            compression: 0, // NONE
        };

        let envelope = Envelope {
            sequence_number: 1,
            message: Some(monoterminal_protocol::envelope::Message::OutputData(
                output_data,
            )),
        };

        let mut buf = Vec::new();
        envelope.encode(&mut buf).unwrap();

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(format!("{}_bytes", size), size, |b, _| {
            b.iter(|| {
                let decoded = Envelope::decode(black_box(&buf[..])).unwrap();
                black_box(decoded);
            })
        });
    }

    group.finish();
}

fn bench_compression_zstd(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    // Typical terminal output: repeated patterns compress well
    let test_data = "echo 'Hello World'\n".repeat(1000).into_bytes();

    group.throughput(Throughput::Bytes(test_data.len() as u64));
    group.bench_function("zstd_level3_encode", |b| {
        b.iter(|| {
            let compressed = zstd::bulk::compress(black_box(&test_data), 3).unwrap();
            black_box(compressed);
        })
    });

    let compressed = zstd::bulk::compress(&test_data, 3).unwrap();

    group.throughput(Throughput::Bytes(compressed.len() as u64));
    group.bench_function("zstd_level3_decode", |b| {
        b.iter(|| {
            let decompressed =
                zstd::bulk::decompress(black_box(&compressed), test_data.len()).unwrap();
            black_box(decompressed);
        })
    });

    group.finish();
}

fn bench_attach_response_with_scrollback(c: &mut Criterion) {
    let mut group = c.benchmark_group("attach_response");

    // Simulate 1000 lines of scrollback
    let scrollback: Vec<monoterminal_protocol::Line> = (0..1000)
        .map(|i| monoterminal_protocol::Line {
            data: format!("Line {} with some terminal output content here", i).into_bytes(),
            line_number: i as u64,
        })
        .collect();

    let response = AttachResponse {
        session_id: "test-session".to_string(),
        metadata: Some(SessionMetadata {
            shell_type: "cmd.exe".to_string(),
            working_dir: "C:\\Users\\user".to_string(),
            rows: 24,
            cols: 80,
            created_at: 1723641600,
            last_activity: 1723641600,
        }),
        scrollback,
    };

    let envelope = Envelope {
        sequence_number: 1,
        message: Some(monoterminal_protocol::envelope::Message::AttachResponse(
            response,
        )),
    };

    group.bench_function("1000_lines", |b| {
        b.iter(|| {
            let mut buf = Vec::new();
            envelope.encode(&mut buf).unwrap();
            black_box(buf);
        })
    });

    group.finish();
}

/// Benchmark WebSocket frame overhead (simulated)
fn bench_websocket_frame_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("websocket_frame");

    // Small message (typical interactive input)
    let small_payload = vec![b'a'; 64];

    // Large message (burst output)
    let large_payload = vec![b'x'; 4096];

    group.throughput(Throughput::Bytes(small_payload.len() as u64));
    group.bench_function("frame_small_64bytes", |b| {
        b.iter(|| {
            // Simulate WebSocket framing (header + payload)
            let mut frame = Vec::with_capacity(small_payload.len() + 14);
            frame.extend_from_slice(&[0x82]); // Binary frame, FIN bit set
            frame.push(small_payload.len() as u8);
            frame.extend_from_slice(black_box(&small_payload));
            black_box(frame);
        })
    });

    group.throughput(Throughput::Bytes(large_payload.len() as u64));
    group.bench_function("frame_large_4096bytes", |b| {
        b.iter(|| {
            let mut frame = Vec::with_capacity(large_payload.len() + 14);
            frame.extend_from_slice(&[0x82, 126]); // Extended payload length
            frame.extend_from_slice(&(large_payload.len() as u16).to_be_bytes());
            frame.extend_from_slice(black_box(&large_payload));
            black_box(frame);
        })
    });

    group.finish();
}

/// Benchmark client fan-out broadcast (1→N)
fn bench_client_fanout(c: &mut Criterion) {
    let mut group = c.benchmark_group("fanout_broadcast");

    let output_data = OutputData {
        data: vec![b'X'; 1024],
        sequence: 100,
        compression: 0,
    };

    let envelope = Envelope {
        sequence_number: 1,
        message: Some(monoterminal_protocol::envelope::Message::OutputData(
            output_data,
        )),
    };

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    // Simulate broadcasting to N clients
    for n_clients in [1, 2, 5, 10].iter() {
        group.throughput(Throughput::Elements(*n_clients as u64));
        group.bench_with_input(
            format!("broadcast_{}_clients", n_clients),
            n_clients,
            |b, &n| {
                b.iter(|| {
                    // Simulate cloning message for each client
                    for _ in 0..n {
                        let _client_copy = buf.clone();
                        black_box(_client_copy);
                    }
                })
            },
        );
    }

    group.finish();
}

/// Benchmark WebRTC signaling messages (Phase 2)
fn bench_webrtc_offer(c: &mut Criterion) {
    let mut group = c.benchmark_group("webrtc_signaling");

    let offer = WebRtcOffer {
        session_id: "test-session-12345".to_string(),
        client_id: "client-uuid-67890".to_string(),
        sdp: "v=0\r\no=- 123456789 2 IN IP4 192.168.1.100\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0\r\n"
            .to_string(),
        peer_id: "ed25519:deadbeef1234567890abcdef".to_string(),
        nonce: 9876543210,
    };

    let envelope = Envelope {
        sequence_number: 100,
        message: Some(monoterminal_protocol::envelope::Message::WebrtcOffer(offer)),
    };

    group.bench_function("encode_offer", |b| {
        b.iter(|| {
            let mut buf = Vec::new();
            envelope.encode(&mut buf).unwrap();
            black_box(buf);
        })
    });

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    group.bench_function("decode_offer", |b| {
        b.iter(|| {
            let decoded = Envelope::decode(black_box(&buf[..])).unwrap();
            black_box(decoded);
        })
    });

    group.finish();
}

fn bench_webrtc_answer_with_turn(c: &mut Criterion) {
    let mut group = c.benchmark_group("webrtc_answer");

    let turn = TurnCredentials {
        urls: vec![
            "turn:coturn.example.com:3478".to_string(),
            "turns:coturn.example.com:5349".to_string(),
        ],
        username: "1692374400:testuser".to_string(),
        credential: "hmac_sha256_credential_string_here".to_string(),
        expires_at_ms: 1692375300000,
    };

    let answer = WebRtcAnswer {
        sdp: "v=0\r\no=- 987654321 2 IN IP4 10.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0\r\n"
            .to_string(),
        turn: Some(turn),
        offer_timestamp_ms: 1692374400500,
    };

    let envelope = Envelope {
        sequence_number: 101,
        message: Some(monoterminal_protocol::envelope::Message::WebrtcAnswer(
            answer,
        )),
    };

    group.bench_function("encode_answer_with_turn", |b| {
        b.iter(|| {
            let mut buf = Vec::new();
            envelope.encode(&mut buf).unwrap();
            black_box(buf);
        })
    });

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    group.bench_function("decode_answer_with_turn", |b| {
        b.iter(|| {
            let decoded = Envelope::decode(black_box(&buf[..])).unwrap();
            black_box(decoded);
        })
    });

    group.finish();
}

fn bench_ice_candidate(c: &mut Criterion) {
    let mut group = c.benchmark_group("ice_candidate");

    let candidate = IceCandidate {
        session_id: "test-session-12345".to_string(),
        client_id: "client-uuid-67890".to_string(),
        candidate: "candidate:1 1 UDP 2130706431 192.168.1.100 54321 typ host".to_string(),
        sdp_mid: Some("0".to_string()),
        sdp_mline_index: Some(0),
    };

    let envelope = Envelope {
        sequence_number: 102,
        message: Some(monoterminal_protocol::envelope::Message::IceCandidate(
            candidate,
        )),
    };

    group.bench_function("encode", |b| {
        b.iter(|| {
            let mut buf = Vec::new();
            envelope.encode(&mut buf).unwrap();
            black_box(buf);
        })
    });

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    group.bench_function("decode", |b| {
        b.iter(|| {
            let decoded = Envelope::decode(black_box(&buf[..])).unwrap();
            black_box(decoded);
        })
    });

    group.finish();
}

/// Benchmark ICE candidate trickle (rapid succession)
fn bench_ice_trickle_burst(c: &mut Criterion) {
    let mut group = c.benchmark_group("ice_trickle");

    // Simulate 10 ICE candidates discovered in rapid succession
    let candidates: Vec<IceCandidate> = (0..10)
        .map(|i| IceCandidate {
            session_id: "test-session-12345".to_string(),
            client_id: "client-uuid-67890".to_string(),
            candidate: format!(
                "candidate:{} 1 UDP {} 192.168.1.{} {} typ host",
                i,
                2130706431 - i,
                100 + i,
                50000 + i
            ),
            sdp_mid: Some("0".to_string()),
            sdp_mline_index: Some(0),
        })
        .collect();

    group.throughput(Throughput::Elements(10));
    group.bench_function("encode_10_candidates", |b| {
        b.iter(|| {
            for (seq, candidate) in candidates.iter().enumerate() {
                let envelope = Envelope {
                    sequence_number: seq as u64,
                    message: Some(monoterminal_protocol::envelope::Message::IceCandidate(
                        candidate.clone(),
                    )),
                };
                let mut buf = Vec::new();
                envelope.encode(&mut buf).unwrap();
                black_box(buf);
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_encode_attach_request,
    bench_decode_attach_request,
    bench_encode_output_data,
    bench_decode_output_data,
    bench_compression_zstd,
    bench_attach_response_with_scrollback,
    bench_websocket_frame_overhead,
    bench_client_fanout,
    bench_webrtc_offer,
    bench_webrtc_answer_with_turn,
    bench_ice_candidate,
    bench_ice_trickle_burst,
);

criterion_main!(benches);
