use super::Plugin;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Registry of plugins for discovering entry points
pub struct PluginRegistry {
    plugins: Vec<Arc<dyn Plugin>>,
}

impl PluginRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    /// Register a plugin
    pub fn register(&mut self, plugin: Arc<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// Discover all entry points from registered plugins
    /// Errors from individual plugins are logged but don't fail the overall detection
    pub fn detect_all_entries(&self, cwd: &Path) -> Vec<PathBuf> {
        let mut all_entries = Vec::new();

        for plugin in &self.plugins {
            match plugin.detect_entries(cwd) {
                Ok(entries) => {
                    all_entries.extend(entries);
                }
                Err(e) => {
                    // Log error but continue - plugins are best-effort
                    eprintln!("Warning: Plugin '{}' failed: {}", plugin.name(), e);
                }
            }
        }

        all_entries
    }

    /// Get names of all registered plugins
    pub fn plugin_names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
