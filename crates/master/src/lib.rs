//! MONOTERMINAL Master Daemon Library
//!
//! This library exposes the core components of the MONOTERMINAL master daemon
//! for use in integration tests and benchmarks.
//!
//! The actual daemon binary is in src/main.rs.

pub mod auth;
pub mod clipboard; // Phase 4: Bidirectional clipboard backend (ADR-020, task-74)
pub mod discovery; // Phase 2: Discovery services (mDNS + directory)
pub mod layout; // Phase 4: Splits/Tabs layout manager (ADR-018, task-72)
pub mod persistence;
pub mod platform;
pub mod plugin; // Phase 4 Week 3-4: WASM plugin system (ADR-019, task-76)
pub mod pty;
pub mod server;
pub mod session;
pub mod ui;
pub mod webrtc; // Phase 2: WebRTC P2P networking // Phase 3 Week 3: Cross-platform file paths
