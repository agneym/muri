use super::{Plugin, PluginEntries, PluginError};
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

/// Plugin to discover ESLint config files as entry points.
///
/// ESLint config files often require/import other local files like
/// custom rules, plugins, shared configs, etc. By adding the config file
/// as an entry point, normal import tracing will discover these dependencies.
pub struct EslintPlugin;

impl EslintPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find ESLint config files in the project root.
    /// Supports both flat config (eslint.config.*) and legacy config (.eslintrc*).
    fn find_config_files(&self, cwd: &Path) -> Vec<PathBuf> {
        let config_names = [
            // Flat config (ESLint 8.21.0+, default in 9.0.0+)
            "eslint.config.js",
            "eslint.config.mjs",
            "eslint.config.cjs",
            "eslint.config.ts",
            // Legacy config (.eslintrc*)
            ".eslintrc",
            ".eslintrc.js",
            ".eslintrc.cjs",
            ".eslintrc.mjs",
            ".eslintrc.json",
            ".eslintrc.yaml",
            ".eslintrc.yml",
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

impl Default for EslintPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EslintPlugin {
    fn name(&self) -> &str {
        "eslint"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("eslint")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<PluginEntries, PluginError> {
        // Simply return config files as entry points.
        // The normal import/require tracing will discover any local dependencies
        // (like custom rules, plugins, shared configs, etc.)
        Ok(PluginEntries::paths(self.find_config_files(cwd)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_with_eslint() {
        let plugin = EslintPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("eslint".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_eslint() {
        let plugin = EslintPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("prettier".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_with_empty_deps() {
        let plugin = EslintPlugin::new();
        let deps = FxHashSet::default();

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_find_flat_config_js() {
        let plugin = EslintPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
export default [
  {
    rules: {
      "no-unused-vars": "error",
    },
  },
];
"#;
        fs::write(temp.path().join("eslint.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("eslint.config.js"));
    }

    #[test]
    fn test_find_flat_config_mjs() {
        let plugin = EslintPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
export default [
  {
    rules: {
      "no-unused-vars": "error",
    },
  },
];
"#;
        fs::write(temp.path().join("eslint.config.mjs"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("eslint.config.mjs"));
    }

    #[test]
    fn test_find_flat_config_ts() {
        let plugin = EslintPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
import type { Linter } from "eslint";

const config: Linter.Config[] = [
  {
    rules: {
      "no-unused-vars": "error",
    },
  },
];

export default config;
"#;
        fs::write(temp.path().join("eslint.config.ts"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("eslint.config.ts"));
    }

    #[test]
    fn test_find_legacy_eslintrc_js() {
        let plugin = EslintPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
module.exports = {
  rules: {
    "no-unused-vars": "error",
  },
};
"#;
        fs::write(temp.path().join(".eslintrc.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with(".eslintrc.js"));
    }

    #[test]
    fn test_find_legacy_eslintrc_json() {
        let plugin = EslintPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
{
  "rules": {
    "no-unused-vars": "error"
  }
}
"#;
        fs::write(temp.path().join(".eslintrc.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with(".eslintrc.json"));
    }

    #[test]
    fn test_find_legacy_eslintrc_yaml() {
        let plugin = EslintPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
rules:
  no-unused-vars: error
"#;
        fs::write(temp.path().join(".eslintrc.yaml"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with(".eslintrc.yaml"));
    }

    #[test]
    fn test_find_legacy_eslintrc_yml() {
        let plugin = EslintPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
rules:
  no-unused-vars: error
"#;
        fs::write(temp.path().join(".eslintrc.yml"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with(".eslintrc.yml"));
    }

    #[test]
    fn test_find_legacy_eslintrc_no_extension() {
        let plugin = EslintPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
{
  "rules": {
    "no-unused-vars": "error"
  }
}
"#;
        fs::write(temp.path().join(".eslintrc"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with(".eslintrc"));
    }

    #[test]
    fn test_no_config_returns_empty() {
        let plugin = EslintPlugin::new();
        let temp = tempdir().unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_find_multiple_configs() {
        let plugin = EslintPlugin::new();
        let temp = tempdir().unwrap();

        // Create multiple config files (unusual but possible during migration)
        fs::write(temp.path().join("eslint.config.js"), "export default []").unwrap();
        fs::write(temp.path().join(".eslintrc.json"), "{}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_plugin_name() {
        let plugin = EslintPlugin::new();
        assert_eq!(plugin.name(), "eslint");
    }

    #[test]
    fn test_default_impl() {
        let _: EslintPlugin = Default::default();
    }
}
