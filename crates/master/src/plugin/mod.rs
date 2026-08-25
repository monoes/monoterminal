//! WASM Plugin System
//!
//! Phase 4 Week 3-4: Plugin backend implementation (ADR-019)
//!
//! # Architecture
//! - PluginRuntime: wasmer integration, WASM loading, hook invocation
//! - PluginManager: Registration, lifecycle, hook routing
//! - Sandbox: Security controls (WASI deny-all, resource limits)
//! - Error types: PluginError enum
//!
//! # Status: Week 3 Day 1 - Module Structure Created
//! TODO: wasmer dependencies blocked by virtual-net compilation (Day 1)
//! TODO: Resolve wasmer dependency issue (Day 2)
//! TODO: Implement PluginRuntime (Day 2-3)
//! TODO: Implement PluginManager (Day 4-5)
//! TODO: Integration tests (Week 4)

#![allow(dead_code)] // Week 3-4 implementation in progress

pub mod error;
pub mod manager;
pub mod runtime;
pub mod sandbox;

#[cfg(test)]
mod tests;

// Re-exports
pub use error::{PluginError, Result};
pub use manager::{PluginId, PluginManager};
pub use runtime::{HookType, PluginAction, PluginRuntime};
pub use sandbox::{PluginConfig, WasiCapabilities};
