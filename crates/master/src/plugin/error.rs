//! Plugin error types
//!
//! Phase 4 Week 3-4: Error handling for WASM plugin system

use thiserror::Error;

/// Plugin operation errors
#[derive(Debug, Error)]
pub enum PluginError {
    /// Plugin ID validation failed
    #[error("Plugin ID invalid: {0}")]
    InvalidPluginId(String),

    /// Plugin already registered
    #[error("Plugin already registered: {0}")]
    AlreadyRegistered(String),

    /// Plugin not found
    #[error("Plugin not found: {0}")]
    NotFound(String),

    /// WASM compilation failed
    #[error("WASM compilation failed: {0}")]
    CompilationFailed(String),

    /// WASI initialization failed
    #[error("WASI initialization failed: {0}")]
    WasiInitFailed(String),

    /// Plugin instantiation failed
    #[error("Plugin instantiation failed: {0}")]
    InstantiationFailed(String),

    /// Plugin not loaded
    #[error("Plugin not loaded")]
    NotLoaded,

    /// Function not found
    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    /// CPU timeout exceeded
    #[error("CPU timeout exceeded (>100ms)")]
    CpuTimeout,

    /// Memory limit exceeded
    #[error("Memory limit exceeded (>50MB)")]
    MemoryLimitExceeded,

    /// Hook invocation failed
    #[error("Hook invocation failed: {0}")]
    HookInvocationFailed(String),
}

/// Result type alias for plugin operations
pub type Result<T> = std::result::Result<T, PluginError>;

impl PluginError {
    /// Map to protocol error code (Plugin Protocol §5)
    pub fn to_error_code(&self) -> u32 {
        match self {
            PluginError::NotFound(_) => 20,                  // PLUGIN_NOT_FOUND
            PluginError::CompilationFailed(_) => 21,         // PLUGIN_LOAD_FAILED
            PluginError::InstantiationFailed(_) => 21,       // PLUGIN_LOAD_FAILED
            PluginError::CpuTimeout => 22,                   // PLUGIN_TIMEOUT
            PluginError::InvalidPluginId(_) => 23,           // PLUGIN_PERMISSION_DENIED
            PluginError::AlreadyRegistered(_) => 23,         // PLUGIN_PERMISSION_DENIED
            _ => 21,                                         // Default: PLUGIN_LOAD_FAILED
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_mapping() {
        assert_eq!(PluginError::NotFound("test".to_string()).to_error_code(), 20);
        assert_eq!(PluginError::CpuTimeout.to_error_code(), 22);
        assert_eq!(
            PluginError::InvalidPluginId("test".to_string()).to_error_code(),
            23
        );
    }
}
