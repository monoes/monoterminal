//! UI Test Example
//!
//! Tests the wgpu + egui rendering system independently
//! Run with: cargo run --example ui_test

use anyhow::Result;

// We need to re-export from the library
// For now, we'll just test the modules compile

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    tracing::info!("MONOTERMINAL UI Test");
    tracing::info!("Testing wgpu + egui rendering (60 FPS target)");

    println!("UI Test - Press Escape to exit");

    // TODO: Initialize UI once library exports are set up
    // let terminal_ui = pollster::block_on(monoterminal_master::ui::TerminalUI::new())?;
    // terminal_ui.run()?;

    println!("UI module compiled successfully!");
    println!("Full UI test will be available after library exports are configured.");

    Ok(())
}
