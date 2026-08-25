//! Plugin manager (registration, lifecycle, hook routing)
//!
//! Phase 4 Week 3-4: Plugin lifecycle management

use super::{HookType, PluginAction, PluginConfig, PluginError, PluginRuntime, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Plugin ID type alias
pub type PluginId = String;

/// Plugin manager
///
/// # Responsibilities
/// - Register plugins from WASM bytes
/// - Manage hook subscriptions
/// - Dispatch hooks to subscribed plugins
/// - Unregister plugins
pub struct PluginManager {
    /// Registered plugins (plugin_id → runtime)
    plugins: Arc<RwLock<HashMap<PluginId, PluginRuntime>>>,

    /// Hook subscriptions (hook_type → [plugin_ids])
    hook_subscriptions: Arc<RwLock<HashMap<HookType, Vec<PluginId>>>>,

    /// Default plugin configuration
    default_config: PluginConfig,
}

impl PluginManager {
    /// Create new plugin manager
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            hook_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            default_config: PluginConfig::default(),
        }
    }

    /// Register plugin from WASM bytes
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin identifier
    /// * `wasm_bytes` - WASM module bytes
    /// * `hooks` - Requested hook subscriptions
    ///
    /// # Returns
    /// Ok(()) if registration successful
    ///
    /// # TODO: Week 3 Day 4-5
    /// - Validate plugin_id (alphanumeric + dash/underscore, max 64 chars)
    /// - Check if plugin already registered
    /// - Load WASM module
    /// - Register hook subscriptions
    /// - Store runtime
    pub async fn register_plugin(
        &self,
        plugin_id: String,
        wasm_bytes: Vec<u8>,
        hooks: Vec<HookType>,
    ) -> Result<()> {
        // TODO: Implement registration (Week 3 Day 4-5)

        // Validate plugin_id
        if !is_valid_plugin_id(&plugin_id) {
            return Err(PluginError::InvalidPluginId(plugin_id));
        }

        // Check if already registered
        {
            let plugins = self.plugins.read().await;
            if plugins.contains_key(&plugin_id) {
                return Err(PluginError::AlreadyRegistered(plugin_id));
            }
        }

        // Load WASM module (stub)
        let runtime = PluginRuntime::load_wasm(
            plugin_id.clone(),
            &wasm_bytes,
            self.default_config.clone(),
        )?;

        // Register hook subscriptions
        {
            let mut hook_subs = self.hook_subscriptions.write().await;
            for hook in &hooks {
                hook_subs
                    .entry(*hook)
                    .or_insert_with(Vec::new)
                    .push(plugin_id.clone());
            }
        }

        // Store runtime
        {
            let mut plugins = self.plugins.write().await;
            plugins.insert(plugin_id.clone(), runtime);
        }

        tracing::info!("Plugin '{}' registered (stub)", plugin_id);

        Ok(())
    }

    /// Unregister plugin
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin to unregister
    ///
    /// # TODO: Week 3 Day 4-5
    /// - Remove from plugins map
    /// - Remove from hook subscriptions
    pub async fn unregister_plugin(&self, plugin_id: &str) -> Result<()> {
        // TODO: Implement unregistration (Week 3 Day 4-5)

        // Remove from plugins
        {
            let mut plugins = self.plugins.write().await;
            plugins.remove(plugin_id);
        }

        // Remove from hook subscriptions
        {
            let mut hook_subs = self.hook_subscriptions.write().await;
            for subs in hook_subs.values_mut() {
                subs.retain(|id| id != plugin_id);
            }
        }

        tracing::info!("Plugin '{}' unregistered", plugin_id);

        Ok(())
    }

    /// Dispatch hook to all subscribed plugins
    ///
    /// # Arguments
    /// * `hook_type` - Hook type to dispatch
    /// * `data` - Hook data
    ///
    /// # Returns
    /// Vec of plugin actions (one per subscribed plugin)
    ///
    /// # TODO: Week 3 Day 4-5
    /// - Get subscribed plugins for hook
    /// - Call hook on each plugin (sequential for now)
    /// - Collect actions
    /// - Handle errors gracefully (log and continue)
    pub async fn dispatch_hook(
        &self,
        hook_type: HookType,
        data: Vec<u8>,
    ) -> Result<Vec<PluginAction>> {
        // TODO: Implement hook dispatch (Week 3 Day 4-5)

        // Get subscribed plugins
        let plugin_ids = {
            let hook_subs = self.hook_subscriptions.read().await;
            hook_subs.get(&hook_type).cloned().unwrap_or_default()
        };

        if plugin_ids.is_empty() {
            return Ok(vec![]);
        }

        // Call hook on each plugin (stub)
        let mut actions = Vec::new();

        for plugin_id in &plugin_ids {
            let plugins = self.plugins.read().await;
            if let Some(runtime) = plugins.get(plugin_id) {
                match hook_type {
                    HookType::OnOutput => {
                        match runtime.call_on_output(&data).await {
                            Ok(action) => actions.push(action),
                            Err(e) => {
                                tracing::warn!("Plugin '{}' on_output failed: {}", plugin_id, e);
                            }
                        }
                    }
                    HookType::OnStateChange => {
                        match runtime.call_on_state_change(&data).await {
                            Ok(action) => actions.push(action),
                            Err(e) => {
                                tracing::warn!("Plugin '{}' on_state_change failed: {}", plugin_id, e);
                            }
                        }
                    }
                }
            }
        }

        tracing::debug!("Dispatched {:?} to {} plugins", hook_type, plugin_ids.len());

        Ok(actions)
    }

    /// List registered plugins
    pub async fn list_plugins(&self) -> Vec<PluginId> {
        let plugins = self.plugins.read().await;
        plugins.keys().cloned().collect()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate plugin ID
///
/// Rules:
/// - Alphanumeric + dash/underscore only
/// - Max 64 characters
/// - Must start with alphanumeric
fn is_valid_plugin_id(plugin_id: &str) -> bool {
    if plugin_id.is_empty() || plugin_id.len() > 64 {
        return false;
    }

    let chars: Vec<char> = plugin_id.chars().collect();

    // Must start with alphanumeric
    if !chars[0].is_alphanumeric() {
        return false;
    }

    // All chars must be alphanumeric or dash/underscore
    chars
        .iter()
        .all(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_plugin_id() {
        assert!(is_valid_plugin_id("test-plugin"));
        assert!(is_valid_plugin_id("test_plugin_123"));
        assert!(!is_valid_plugin_id("-test-plugin"));  // Starts with dash
        assert!(!is_valid_plugin_id("test@plugin"));   // Invalid character
        assert!(!is_valid_plugin_id(""));               // Empty
        assert!(!is_valid_plugin_id(&"a".repeat(65)));  // Too long
    }

    #[tokio::test]
    async fn test_register_plugin() {
        let manager = PluginManager::new();

        let result = manager
            .register_plugin(
                "test-plugin".to_string(),
                vec![],  // Empty WASM bytes (stub)
                vec![HookType::OnOutput],
            )
            .await;

        assert!(result.is_ok());

        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0], "test-plugin");
    }

    #[tokio::test]
    async fn test_register_duplicate_plugin() {
        let manager = PluginManager::new();

        manager
            .register_plugin(
                "test-plugin".to_string(),
                vec![],
                vec![HookType::OnOutput],
            )
            .await
            .unwrap();

        let result = manager
            .register_plugin(
                "test-plugin".to_string(),
                vec![],
                vec![HookType::OnOutput],
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PluginError::AlreadyRegistered(_)));
    }

    #[tokio::test]
    async fn test_unregister_plugin() {
        let manager = PluginManager::new();

        manager
            .register_plugin(
                "test-plugin".to_string(),
                vec![],
                vec![HookType::OnOutput],
            )
            .await
            .unwrap();

        manager.unregister_plugin("test-plugin").await.unwrap();

        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 0);
    }

    #[tokio::test]
    async fn test_dispatch_hook() {
        let manager = PluginManager::new();

        manager
            .register_plugin(
                "test-plugin".to_string(),
                vec![],
                vec![HookType::OnOutput],
            )
            .await
            .unwrap();

        let actions = manager
            .dispatch_hook(HookType::OnOutput, vec![1, 2, 3])
            .await
            .unwrap();

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], PluginAction::Pass);
    }
}
