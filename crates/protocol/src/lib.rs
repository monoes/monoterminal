// Generated Protocol Buffer types for MONOTERMINAL wire protocol
// See: docs/monoterminal-srs.md §3.1.1

#![allow(clippy::all)]

// Re-export generated types for clean API
pub mod generated {
    include!("generated/monoterminal.v1.rs");
}

pub use generated::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resize_request() {
        let resize_req = ResizeRequest {
            rows: 40,
            cols: 120,
            auth_token: "test-jwt-token".to_string(),
        };

        let envelope = Envelope {
            sequence_number: 42,
            message: Some(envelope::Message::ResizeRequest(resize_req)),
        };

        assert_eq!(envelope.sequence_number, 42);
        match envelope.message {
            Some(envelope::Message::ResizeRequest(r)) => {
                assert_eq!(r.rows, 40);
                assert_eq!(r.cols, 120);
            }
            _ => panic!("Expected ResizeRequest"),
        }
    }

    #[test]
    fn test_webrtc_offer_roundtrip() {
        let offer = WebRtcOffer {
            session_id: "test-session-123".to_string(),
            client_id: "client-uuid-456".to_string(),
            sdp: "v=0\r\no=- 123456 2 IN IP4 127.0.0.1\r\n".to_string(),
            peer_id: "deadbeef1234567890abcdef".to_string(),
            nonce: 12345678,
        };

        let envelope = Envelope {
            sequence_number: 100,
            message: Some(envelope::Message::WebrtcOffer(offer.clone())),
        };

        match envelope.message {
            Some(envelope::Message::WebrtcOffer(o)) => {
                assert_eq!(o.session_id, offer.session_id);
                assert_eq!(o.client_id, offer.client_id);
                assert_eq!(o.sdp, offer.sdp);
                assert_eq!(o.peer_id, offer.peer_id);
                assert_eq!(o.nonce, offer.nonce);
            }
            _ => panic!("Expected WebRTCOffer"),
        }
    }

    #[test]
    fn test_webrtc_answer_with_turn_roundtrip() {
        let turn = TurnCredentials {
            urls: vec![
                "turn:coturn.example.com:3478".to_string(),
                "turns:coturn.example.com:5349".to_string(),
            ],
            username: "1692374400:testuser".to_string(),
            credential: "hmac_sha256_credential".to_string(),
            expires_at_ms: 1692375300000, // 15 minutes later
        };

        let answer = WebRtcAnswer {
            sdp: "v=0\r\no=- 654321 2 IN IP4 192.168.1.1\r\n".to_string(),
            turn: Some(turn.clone()),
            offer_timestamp_ms: 1692374400500,
        };

        let envelope = Envelope {
            sequence_number: 101,
            message: Some(envelope::Message::WebrtcAnswer(answer.clone())),
        };

        match envelope.message {
            Some(envelope::Message::WebrtcAnswer(a)) => {
                assert_eq!(a.sdp, answer.sdp);
                assert_eq!(a.offer_timestamp_ms, answer.offer_timestamp_ms);

                let turn_creds = a.turn.expect("TURN credentials should be present");
                assert_eq!(turn_creds.urls, turn.urls);
                assert_eq!(turn_creds.username, turn.username);
                assert_eq!(turn_creds.credential, turn.credential);
                assert_eq!(turn_creds.expires_at_ms, turn.expires_at_ms);
            }
            _ => panic!("Expected WebRTCAnswer"),
        }
    }

    #[test]
    fn test_ice_candidate_roundtrip() {
        let candidate = IceCandidate {
            session_id: "test-session-123".to_string(),
            client_id: "client-uuid-456".to_string(),
            candidate: "candidate:1 1 UDP 2130706431 192.168.1.100 54321 typ host".to_string(),
            sdp_mid: Some("0".to_string()),
            sdp_mline_index: Some(0),
        };

        let envelope = Envelope {
            sequence_number: 102,
            message: Some(envelope::Message::IceCandidate(candidate.clone())),
        };

        match envelope.message {
            Some(envelope::Message::IceCandidate(c)) => {
                assert_eq!(c.session_id, candidate.session_id);
                assert_eq!(c.client_id, candidate.client_id);
                assert_eq!(c.candidate, candidate.candidate);
                assert_eq!(c.sdp_mid, candidate.sdp_mid);
                assert_eq!(c.sdp_mline_index, candidate.sdp_mline_index);
            }
            _ => panic!("Expected ICECandidate"),
        }
    }

    #[test]
    fn test_ice_candidate_minimal_fields() {
        // Test with only required fields (session_id, client_id, candidate)
        let candidate = IceCandidate {
            session_id: "test-session-123".to_string(),
            client_id: "client-uuid-456".to_string(),
            candidate: "candidate:1 1 UDP 2130706431 192.168.1.100 54321 typ host".to_string(),
            sdp_mid: None,
            sdp_mline_index: None,
        };

        let envelope = Envelope {
            sequence_number: 103,
            message: Some(envelope::Message::IceCandidate(candidate.clone())),
        };

        match envelope.message {
            Some(envelope::Message::IceCandidate(c)) => {
                assert_eq!(c.session_id, candidate.session_id);
                assert_eq!(c.client_id, candidate.client_id);
                assert_eq!(c.candidate, candidate.candidate);
                assert!(c.sdp_mid.is_none());
                assert!(c.sdp_mline_index.is_none());
            }
            _ => panic!("Expected ICECandidate"),
        }
    }
}
