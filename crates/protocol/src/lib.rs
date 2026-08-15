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
            auth_token: Some("test-jwt-token".to_string()),
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
}
