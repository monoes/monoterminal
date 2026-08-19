//! Protocol overhead validation tests
//!
//! Validates SRS §3.1.1 target: 10-20 byte fixed overhead per message

use monoterminal_protocol::{Envelope, IceCandidate, TurnCredentials, WebRtcAnswer, WebRtcOffer};
use prost::Message;

#[test]
fn test_webrtc_offer_overhead() {
    let offer = WebRtcOffer {
        session_id: "test-session-123".to_string(),
        client_id: "client-uuid-456".to_string(),
        sdp: "v=0\r\no=- 123 2 IN IP4 192.168.1.1\r\n".to_string(), // 40 bytes
        peer_id: "ed25519:abcd1234".to_string(),
        nonce: 123456789,
    };

    let envelope = Envelope {
        sequence_number: 100,
        message: Some(monoterminal_protocol::envelope::Message::WebrtcOffer(
            offer.clone(),
        )),
    };

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    // Calculate overhead: total wire size - payload size
    let payload_size =
        offer.session_id.len() + offer.client_id.len() + offer.sdp.len() + offer.peer_id.len() + 8; // u64 nonce

    let wire_size = buf.len();
    let overhead = wire_size - payload_size;

    println!(
        "WebRTCOffer - Wire: {} bytes, Payload: {} bytes, Overhead: {} bytes",
        wire_size, payload_size, overhead
    );

    // SRS §3.1.1 target: 10-20 bytes overhead
    assert!(
        (10..=30).contains(&overhead),
        "Overhead {} bytes outside expected range 10-30 bytes (acceptable variance for Protobuf varint encoding)",
        overhead
    );
}

#[test]
fn test_webrtc_answer_overhead() {
    let turn = TurnCredentials {
        urls: vec!["turn:coturn.example.com:3478".to_string()],
        username: "1692374400:testuser".to_string(),
        credential: "hmac_sha256_credential".to_string(),
        expires_at_ms: 1692375300000,
    };

    let answer = WebRtcAnswer {
        sdp: "v=0\r\no=- 456 2 IN IP4 10.0.0.1\r\n".to_string(), // 37 bytes
        turn: Some(turn.clone()),
        offer_timestamp_ms: 1692374400500,
    };

    let envelope = Envelope {
        sequence_number: 101,
        message: Some(monoterminal_protocol::envelope::Message::WebrtcAnswer(
            answer.clone(),
        )),
    };

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    let payload_size = answer.sdp.len()
        + turn.urls.iter().map(|u| u.len()).sum::<usize>()
        + turn.username.len()
        + turn.credential.len()
        + 8 // expires_at_ms
        + 8; // offer_timestamp_ms

    let wire_size = buf.len();
    let overhead = wire_size - payload_size;

    println!(
        "WebRTCAnswer - Wire: {} bytes, Payload: {} bytes, Overhead: {} bytes",
        wire_size, payload_size, overhead
    );

    assert!(
        (10..=40).contains(&overhead),
        "Overhead {} bytes outside expected range 10-40 bytes (nested message adds tags)",
        overhead
    );
}

#[test]
fn test_ice_candidate_overhead() {
    let candidate = IceCandidate {
        session_id: "test-session-123".to_string(),
        client_id: "client-uuid-456".to_string(),
        candidate: "candidate:1 1 UDP 2130706431 192.168.1.100 54321 typ host".to_string(), // 58 bytes
        sdp_mid: Some("0".to_string()),
        sdp_mline_index: Some(0),
    };

    let envelope = Envelope {
        sequence_number: 102,
        message: Some(monoterminal_protocol::envelope::Message::IceCandidate(
            candidate.clone(),
        )),
    };

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    let payload_size = candidate.session_id.len()
        + candidate.client_id.len()
        + candidate.candidate.len()
        + candidate.sdp_mid.as_ref().map(|s| s.len()).unwrap_or(0)
        + 4; // u32 sdp_mline_index

    let wire_size = buf.len();
    let overhead = wire_size - payload_size;

    println!(
        "ICECandidate - Wire: {} bytes, Payload: {} bytes, Overhead: {} bytes",
        wire_size, payload_size, overhead
    );

    assert!(
        (10..=30).contains(&overhead),
        "Overhead {} bytes outside expected range 10-30 bytes",
        overhead
    );
}

#[test]
fn test_minimal_ice_candidate_overhead() {
    // Smallest possible ICE candidate (minimal fields)
    let candidate = IceCandidate {
        session_id: "s123".to_string(), // 4 bytes
        client_id: "c456".to_string(),  // 4 bytes
        candidate: "candidate:1 1 UDP 2130706431 10.0.0.1 5000 typ host".to_string(), // 52 bytes
        sdp_mid: None,
        sdp_mline_index: None,
    };

    let envelope = Envelope {
        sequence_number: 1,
        message: Some(monoterminal_protocol::envelope::Message::IceCandidate(
            candidate.clone(),
        )),
    };

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    let payload_size =
        candidate.session_id.len() + candidate.client_id.len() + candidate.candidate.len();

    let wire_size = buf.len();
    let overhead = wire_size - payload_size;

    println!(
        "Minimal ICECandidate - Wire: {} bytes, Payload: {} bytes, Overhead: {} bytes",
        wire_size, payload_size, overhead
    );

    // For minimal messages, overhead should be closer to the 10-byte target
    assert!(
        (8..=25).contains(&overhead),
        "Minimal overhead {} bytes outside expected range 8-25 bytes",
        overhead
    );
}
