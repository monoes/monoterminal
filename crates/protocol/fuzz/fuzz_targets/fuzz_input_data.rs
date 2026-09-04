#![no_main]

use libfuzzer_sys::fuzz_target;
use monoterminal_protocol::{InputData, Envelope, envelope};
use prost::Message;

fuzz_target!(|data: &[u8]| {
    // Fuzz InputData deserialization
    // Critical path: User keyboard input → PTY
    // Tests: malformed UTF-8, large payloads, auth token edge cases

    // Attempt 1: Decode as raw InputData
    if let Ok(input_data) = InputData::decode(data) {
        // Exercise keyboard input data
        let _ = input_data.data.len();
        let _ = input_data.auth_token.len();

        // Test pane_id (optional field added in task-69)
        if let Some(pane_id) = &input_data.pane_id {
            let _ = pane_id.len();
        }

        // Validate UTF-8 parsing (keyboard input should be valid UTF-8)
        let _ = std::str::from_utf8(&input_data.data);

        // Test re-encoding roundtrip
        let mut buf = Vec::new();
        if input_data.encode(&mut buf).is_ok() {
            let _ = InputData::decode(&buf[..]);
        }
    }

    // Attempt 2: Decode as Envelope containing InputData
    if let Ok(envelope) = Envelope::decode(data) {
        if let Some(envelope::Message::InputData(input_data)) = envelope.message {
            let _ = envelope.sequence_number;
            let _ = input_data.data.len();
        }
    }
});
