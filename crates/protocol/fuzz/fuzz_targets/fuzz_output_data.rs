#![no_main]

use libfuzzer_sys::fuzz_target;
use monoterminal_protocol::{OutputData, Envelope, envelope, CompressionType};
use prost::Message;

fuzz_target!(|data: &[u8]| {
    // Fuzz OutputData deserialization
    // Critical path: PTY output → Client rendering
    // Tests: malformed output, large chunks, compression edge cases, sequence overflow

    // Attempt 1: Decode as raw OutputData
    if let Ok(output_data) = OutputData::decode(data) {
        // Exercise PTY output data
        let _ = output_data.data.len();
        let _ = output_data.sequence;

        // Validate compression type enum
        let compression = CompressionType::try_from(output_data.compression)
            .unwrap_or(CompressionType::None);

        match compression {
            CompressionType::None => {
                // Uncompressed: raw bytes
                let _ = output_data.data.len();
            }
            CompressionType::Zstd => {
                // Compressed: attempt decompression (if zstd available)
                // Note: zstd decompression is client's responsibility
                // Here we just validate the field exists
                let _ = output_data.data.len();
            }
        }

        // Test sequence number edge cases
        let _ = output_data.sequence.wrapping_add(1);

        // Test re-encoding roundtrip
        let mut buf = Vec::new();
        if output_data.encode(&mut buf).is_ok() {
            let _ = OutputData::decode(&buf[..]);
        }
    }

    // Attempt 2: Decode as Envelope containing OutputData
    if let Ok(envelope) = Envelope::decode(data) {
        if let Some(envelope::Message::OutputData(output_data)) = envelope.message {
            let _ = envelope.sequence_number;
            let _ = output_data.data.len();
            let _ = output_data.sequence;
        }
    }
});
