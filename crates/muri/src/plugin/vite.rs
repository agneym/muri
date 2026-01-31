use super::{Plugin, PluginEntries, PluginError};
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

/// Plugin to discover Vite config files as entry points.
///
/// Vite config files often import other local files like custom plugins,
/// shared configurations, or utility modules. By adding the config file
/// as an entry point, normal import tracing will discover these dependencies.
pub struct VitePlugin;

impl VitePlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find Vite config files in the project root.
    /// Follows the same lookup order as Vite:
    /// https://vite.dev/config/#configuring-vite
    fn find_config_files(&self, cwd: &Path) -> Vec<PathBuf> {
        let config_names = [
            "vite.config.js",
            "vite.config.mjs",
            "vite.config.ts",
            "vite.config.cjs",
            "vite.config.mts",
            "vite.config.cts",
        ];

        let mut found = Vec::new();

        for name in &config_names {
            let path = cwd.join(name);
            if path.exists() && path.is_file() {
                if let Ok(canonical) = path.canonicalize() {
                    found.push(canonical);
                }
            }
        }

        found
    }
}

impl Default for VitePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for VitePlugin {
    fn name(&self) -> &str {
        "vite"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("vite")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<PluginEntries, PluginError> {
        // Simply return config files as entry points.
        // The normal import/require tracing will discover any local dependencies
        // (like custom plugins, shared configs, etc.)
        Ok(PluginEntries::paths(self.find_config_files(cwd)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_with_vite() {
        let plugin = VitePlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("vite".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_vite() {
        let plugin = VitePlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("webpack".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_with_empty_deps() {
        let plugin = VitePlugin::new();
        let deps = FxHashSet::default();

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_find_vite_config_js() {
        let plugin = VitePlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [],
});
"#;
        fs::write(temp.path().join("vite.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("vite.config.js"));
    }

    #[test]
    fn test_find_vite_config_ts() {
        let plugin = VitePlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [],
});
"#;
        fs::write(temp.path().join("vite.config.ts"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("vite.config.ts"));
    }

    #[test]
    fn test_find_vite_config_mjs() {
        let plugin = VitePlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [],
});
"#;
        fs::write(temp.path().join("vite.config.mjs"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("vite.config.mjs"));
    }

    #[test]
    fn test_find_vite_config_cjs() {
        let plugin = VitePlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
const { defineConfig } = require('vite');

module.exports = defineConfig({
  plugins: [],
});
"#;
        fs::write(temp.path().join("vite.config.cjs"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("vite.config.cjs"));
    }

    #[test]
    fn test_find_vite_config_mts() {
        let plugin = VitePlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [],
});
"#;
        fs::write(temp.path().join("vite.config.mts"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("vite.config.mts"));
    }

    #[test]
    fn test_find_vite_config_cts() {
        let plugin = VitePlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
const { defineConfig } = require('vite');

module.exports = defineConfig({
  plugins: [],
});
"#;
        fs::write(temp.path().join("vite.config.cts"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("vite.config.cts"));
    }

    #[test]
    fn test_no_config_returns_empty() {
        let plugin = VitePlugin::new();
        let temp = tempdir().unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_find_multiple_configs() {
        let plugin = VitePlugin::new();
        let temp = tempdir().unwrap();

        // Create multiple config files (unusual but possible)
        fs::write(temp.path().join("vite.config.js"), "export default {}").unwrap();
        fs::write(temp.path().join("vite.config.ts"), "export default {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_default_impl() {
        let _: VitePlugin = Default::default();
    }
}
