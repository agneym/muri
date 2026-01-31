use super::{Plugin, PluginError};
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

/// Plugin to discover PostCSS config files as entry points.
///
/// PostCSS config files often require/import other local files like
/// tailwind.config.js, custom plugins, etc. By adding the config file
/// as an entry point, normal import tracing will discover these dependencies.
pub struct PostcssPlugin;

impl PostcssPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find PostCSS config files in the project root.
    /// Follows the same lookup order as postcss-load-config:
    /// https://github.com/postcss/postcss-load-config
    fn find_config_files(&self, cwd: &Path) -> Vec<PathBuf> {
        let config_names = [
            // postcss.config.* variants
            "postcss.config.js",
            "postcss.config.cjs",
            "postcss.config.mjs",
            "postcss.config.ts",
            "postcss.config.cts",
            "postcss.config.mts",
            // .postcssrc.* variants
            ".postcssrc",
            ".postcssrc.js",
            ".postcssrc.cjs",
            ".postcssrc.mjs",
            ".postcssrc.ts",
            ".postcssrc.cts",
            ".postcssrc.mts",
            ".postcssrc.json",
            ".postcssrc.yaml",
            ".postcssrc.yml",
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

impl Default for PostcssPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for PostcssPlugin {
    fn name(&self) -> &str {
        "postcss"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("postcss") || dependencies.contains("postcss-cli")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        // Simply return config files as entry points.
        // The normal import/require tracing will discover any local dependencies
        // (like tailwind.config.js, custom plugins, etc.)
        Ok(self.find_config_files(cwd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_with_postcss() {
        let plugin = PostcssPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("postcss".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_enable_with_postcss_cli() {
        let plugin = PostcssPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("postcss-cli".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_postcss() {
        let plugin = PostcssPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("tailwindcss".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_find_postcss_config_js() {
        let plugin = PostcssPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
module.exports = {
  plugins: {
    autoprefixer: {},
  },
};
"#;
        fs::write(temp.path().join("postcss.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("postcss.config.js"));
    }

    #[test]
    fn test_find_postcss_config_mjs() {
        let plugin = PostcssPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
export default {
  plugins: {
    autoprefixer: {},
  },
};
"#;
        fs::write(temp.path().join("postcss.config.mjs"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("postcss.config.mjs"));
    }

    #[test]
    fn test_find_postcssrc() {
        let plugin = PostcssPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
module.exports = {
  plugins: [],
};
"#;
        fs::write(temp.path().join(".postcssrc.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with(".postcssrc.js"));
    }

    #[test]
    fn test_no_config_returns_empty() {
        let plugin = PostcssPlugin::new();
        let temp = tempdir().unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_find_multiple_configs() {
        let plugin = PostcssPlugin::new();
        let temp = tempdir().unwrap();

        // Create multiple config files (unusual but possible)
        fs::write(temp.path().join("postcss.config.js"), "module.exports = {}").unwrap();
        fs::write(temp.path().join(".postcssrc.json"), "{}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);
    }
}
