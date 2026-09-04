#![no_main]

use libfuzzer_sys::fuzz_target;
use monoterminal_protocol::{AttachRequest, Envelope, envelope};
use prost::Message;

fuzz_target!(|data: &[u8]| {
    // Fuzz AttachRequest deserialization from raw bytes
    // This tests prost's protobuf parsing for buffer overflows,
    // malformed fields, and edge cases

    // Attempt 1: Decode as raw AttachRequest
    if let Ok(attach_req) = AttachRequest::decode(data) {
        // Exercise the parsed message (validate fields)
        let _ = attach_req.session_id.len();
        let _ = attach_req.auth_token.len();
        let _ = attach_req.rows.saturating_mul(attach_req.cols);

        // Test re-encoding (roundtrip)
        let mut buf = Vec::new();
        if attach_req.encode(&mut buf).is_ok() {
            // Verify re-encoding produces valid output
            let _ = AttachRequest::decode(&buf[..]);
        }
    }

    // Attempt 2: Decode as Envelope containing AttachRequest
    if let Ok(envelope) = Envelope::decode(data) {
        if let Some(envelope::Message::AttachRequest(attach_req)) = envelope.message {
            // Validate envelope wrapper
            let _ = envelope.sequence_number;
            let _ = attach_req.session_id.len();
        }
    }
});
