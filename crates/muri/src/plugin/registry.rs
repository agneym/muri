use super::{EntryPattern, Plugin, PluginEntries};
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

    /// Collect all patterns and paths from registered plugins in a single pass.
    ///
    /// Returns (patterns, paths) where:
    /// - patterns: Glob patterns to match during the collector's filesystem walk
    /// - paths: Already-resolved absolute paths (config files, etc.)
    ///
    /// This is more efficient than calling collect_patterns() and collect_paths()
    /// separately, as it only calls detect_entries() once per plugin.
    pub fn collect_all(&self, cwd: &Path) -> (Vec<EntryPattern>, Vec<PathBuf>) {
        let mut all_patterns = Vec::new();
        let mut all_paths = Vec::new();

        for plugin in &self.plugins {
            match plugin.detect_entries(cwd) {
                Ok(entries) => match entries {
                    PluginEntries::Empty => {}
                    PluginEntries::Patterns(patterns) => {
                        all_patterns.extend(patterns);
                    }
                    PluginEntries::Paths(paths) => {
                        all_paths.extend(paths);
                    }
                    PluginEntries::Mixed { patterns, paths } => {
                        all_patterns.extend(patterns);
                        all_paths.extend(paths);
                    }
                },
                Err(e) => {
                    eprintln!("Warning: Plugin '{}' failed: {}", plugin.name(), e);
                }
            }
        }

        (all_patterns, all_paths)
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
