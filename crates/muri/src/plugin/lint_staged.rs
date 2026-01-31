use super::{Plugin, PluginError};
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

/// Plugin to discover lint-staged config files as entry points.
///
/// lint-staged config files can import/require other local files like
/// custom scripts or shared configurations. By adding the config file
/// as an entry point, normal import tracing will discover these dependencies.
pub struct LintStagedPlugin;

impl LintStagedPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find lint-staged config files in the project root.
    /// Follows the same lookup order as lint-staged:
    /// https://github.com/lint-staged/lint-staged#configuration
    fn find_config_files(&self, cwd: &Path) -> Vec<PathBuf> {
        let config_names = [
            // lint-staged.config.* variants
            "lint-staged.config.js",
            "lint-staged.config.mjs",
            "lint-staged.config.cjs",
            // .lintstagedrc.* variants
            ".lintstagedrc",
            ".lintstagedrc.js",
            ".lintstagedrc.cjs",
            ".lintstagedrc.mjs",
            ".lintstagedrc.json",
            ".lintstagedrc.yaml",
            ".lintstagedrc.yml",
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

impl Default for LintStagedPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for LintStagedPlugin {
    fn name(&self) -> &str {
        "lint-staged"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("lint-staged")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        // Simply return config files as entry points.
        // The normal import/require tracing will discover any local dependencies
        // (like custom scripts, shared configs, etc.)
        Ok(self.find_config_files(cwd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_with_lint_staged() {
        let plugin = LintStagedPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("lint-staged".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_lint_staged() {
        let plugin = LintStagedPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("husky".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_with_empty_deps() {
        let plugin = LintStagedPlugin::new();
        let deps = FxHashSet::default();

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_find_lint_staged_config_js() {
        let plugin = LintStagedPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
module.exports = {
  '*.js': ['eslint --fix', 'prettier --write'],
};
"#;
        fs::write(temp.path().join("lint-staged.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("lint-staged.config.js"));
    }

    #[test]
    fn test_find_lint_staged_config_mjs() {
        let plugin = LintStagedPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
export default {
  '*.js': ['eslint --fix', 'prettier --write'],
};
"#;
        fs::write(temp.path().join("lint-staged.config.mjs"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("lint-staged.config.mjs"));
    }

    #[test]
    fn test_find_lint_staged_config_cjs() {
        let plugin = LintStagedPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
module.exports = {
  '*.js': 'eslint --fix',
};
"#;
        fs::write(temp.path().join("lint-staged.config.cjs"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("lint-staged.config.cjs"));
    }

    #[test]
    fn test_find_lintstagedrc() {
        let plugin = LintStagedPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"{"*.js": "eslint --fix"}"#;
        fs::write(temp.path().join(".lintstagedrc"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with(".lintstagedrc"));
    }

    #[test]
    fn test_find_lintstagedrc_js() {
        let plugin = LintStagedPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
module.exports = {
  '*.ts': ['eslint --fix'],
};
"#;
        fs::write(temp.path().join(".lintstagedrc.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with(".lintstagedrc.js"));
    }

    #[test]
    fn test_find_lintstagedrc_json() {
        let plugin = LintStagedPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"{"*.js": "eslint --fix"}"#;
        fs::write(temp.path().join(".lintstagedrc.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with(".lintstagedrc.json"));
    }

    #[test]
    fn test_find_lintstagedrc_yaml() {
        let plugin = LintStagedPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
'*.js':
  - eslint --fix
  - prettier --write
"#;
        fs::write(temp.path().join(".lintstagedrc.yaml"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with(".lintstagedrc.yaml"));
    }

    #[test]
    fn test_find_lintstagedrc_yml() {
        let plugin = LintStagedPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
'*.js':
  - eslint --fix
"#;
        fs::write(temp.path().join(".lintstagedrc.yml"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with(".lintstagedrc.yml"));
    }

    #[test]
    fn test_no_config_returns_empty() {
        let plugin = LintStagedPlugin::new();
        let temp = tempdir().unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_find_multiple_configs() {
        let plugin = LintStagedPlugin::new();
        let temp = tempdir().unwrap();

        // Create multiple config files (unusual but possible)
        fs::write(temp.path().join("lint-staged.config.js"), "module.exports = {}").unwrap();
        fs::write(temp.path().join(".lintstagedrc.json"), "{}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_plugin_name() {
        let plugin = LintStagedPlugin::new();
        assert_eq!(plugin.name(), "lint-staged");
    }

    #[test]
    fn test_default_impl() {
        let _: LintStagedPlugin = Default::default();
    }
}
