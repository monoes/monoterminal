fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Single source of truth: proto/monoterminal/v1/messages.proto
    // (Superseded envelope.proto removed 2026-08-15, see ADR-004)
    prost_build::Config::new()
        .out_dir("src/generated")
        .compile_protos(
            &["../../proto/monoterminal/v1/messages.proto"],
            &["../../proto"],
        )?;
    Ok(())
}
