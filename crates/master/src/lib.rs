//! MONOTERMINAL Master Daemon Library
//!
//! This library exposes the core components of the MONOTERMINAL master daemon
//! for use in integration tests and benchmarks.
//!
//! The actual daemon binary is in src/main.rs.

pub mod auth;
pub mod discovery; // Phase 2: Discovery services (mDNS + directory)
pub mod persistence;
pub mod platform;
pub mod pty;
pub mod server;
pub mod session;
pub mod ui;
pub mod webrtc; // Phase 2: WebRTC P2P networking // Phase 3 Week 3: Cross-platform file paths
