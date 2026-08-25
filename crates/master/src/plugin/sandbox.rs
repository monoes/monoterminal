//! Security sandbox configuration
//!
//! Phase 4 Week 3-4: 5-layer defense (ADR-019)

use std::time::Duration;

/// Plugin sandbox configuration
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Memory limit (bytes)
    pub memory_limit: usize,

    /// CPU timeout per hook invocation
    pub cpu_timeout: Duration,

    /// Allowed host functions (whitelist)
    pub allowed_host_functions: Vec<String>,

    /// WASI capabilities (deny-all by default)
    pub wasi_capabilities: WasiCapabilities,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            memory_limit: 50_000_000,  // 50 MB (ADR-019)
            cpu_timeout: Duration::from_millis(100),  // 100ms (ADR-019)
            allowed_host_functions: vec![
                "log".to_string(),
                "get_config".to_string(),
                "storage_get".to_string(),
                "storage_set".to_string(),
            ],
            wasi_capabilities: WasiCapabilities::deny_all(),
        }
    }
}

/// WASI capabilities configuration
#[derive(Debug, Clone)]
pub struct WasiCapabilities {
    /// Filesystem access
    pub filesystem: bool,

    /// Network access
    pub network: bool,

    /// Environment variables
    pub env_vars: bool,

    /// Clock access
    pub clock: bool,
}

impl WasiCapabilities {
    /// Deny-all WASI capabilities (secure default)
    pub fn deny_all() -> Self {
        Self {
            filesystem: false,
            network: false,
            env_vars: false,
            clock: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PluginConfig::default();
        assert_eq!(config.memory_limit, 50_000_000);
        assert_eq!(config.cpu_timeout, Duration::from_millis(100));
        assert!(!config.wasi_capabilities.filesystem);
        assert!(!config.wasi_capabilities.network);
    }

    #[test]
    fn test_deny_all() {
        let caps = WasiCapabilities::deny_all();
        assert!(!caps.filesystem);
        assert!(!caps.network);
        assert!(!caps.env_vars);
        assert!(!caps.clock);
    }
}
