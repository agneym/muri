use super::{EntryPattern, Plugin, PluginEntries, PluginError};
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

/// Plugin to discover Cypress test files and config as entry points
pub struct CypressPlugin;

impl CypressPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find Cypress config file in the project
    fn find_config_file(&self, cwd: &Path) -> Option<PathBuf> {
        let extensions = ["js", "ts", "mjs", "cjs"];
        for ext in extensions {
            let path = cwd.join(format!("cypress.config.{}", ext));
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    /// Default test file patterns for Cypress
    fn default_patterns() -> &'static [&'static str] {
        &[
            // E2E tests
            "cypress/e2e/**/*.cy.js",
            "cypress/e2e/**/*.cy.jsx",
            "cypress/e2e/**/*.cy.ts",
            "cypress/e2e/**/*.cy.tsx",
            "cypress/e2e/**/*.spec.js",
            "cypress/e2e/**/*.spec.jsx",
            "cypress/e2e/**/*.spec.ts",
            "cypress/e2e/**/*.spec.tsx",
            // Support files
            "cypress/support/**/*.js",
            "cypress/support/**/*.ts",
            // Component tests
            "cypress/component/**/*.cy.js",
            "cypress/component/**/*.cy.jsx",
            "cypress/component/**/*.cy.ts",
            "cypress/component/**/*.cy.tsx",
            "cypress/component/**/*.spec.js",
            "cypress/component/**/*.spec.jsx",
            "cypress/component/**/*.spec.ts",
            "cypress/component/**/*.spec.tsx",
        ]
    }

    /// Convert default patterns to EntryPatterns
    fn patterns_to_entry_patterns() -> Vec<EntryPattern> {
        Self::default_patterns().iter().map(|p| EntryPattern::new(*p)).collect()
    }
}

impl Default for CypressPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CypressPlugin {
    fn name(&self) -> &str {
        "cypress"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("cypress")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<PluginEntries, PluginError> {
        let mut paths = Vec::new();

        // Add config file as entry point (path, not pattern)
        if let Some(config_path) = self.find_config_file(cwd) {
            if let Ok(canonical) = config_path.canonicalize() {
                paths.push(canonical);
            }
        }

        // Return test patterns + config path
        let entry_patterns = Self::patterns_to_entry_patterns();
        Ok(PluginEntries::mixed(entry_patterns, paths))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_with_cypress() {
        let plugin = CypressPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("cypress".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_cypress() {
        let plugin = CypressPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("jest".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_plugin_name() {
        let plugin = CypressPlugin::new();
        assert_eq!(plugin.name(), "cypress");
    }

    #[test]
    fn test_find_config_file_js() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        fs::write(temp.path().join("cypress.config.js"), "module.exports = {}").unwrap();

        let config = plugin.find_config_file(temp.path());
        assert!(config.is_some());
        assert!(config.unwrap().ends_with("cypress.config.js"));
    }

    #[test]
    fn test_find_config_file_ts() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        fs::write(temp.path().join("cypress.config.ts"), "export default defineConfig({})")
            .unwrap();

        let config = plugin.find_config_file(temp.path());
        assert!(config.is_some());
        assert!(config.unwrap().ends_with("cypress.config.ts"));
    }

    #[test]
    fn test_find_config_file_mjs() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        fs::write(temp.path().join("cypress.config.mjs"), "export default {}").unwrap();

        let config = plugin.find_config_file(temp.path());
        assert!(config.is_some());
        assert!(config.unwrap().ends_with("cypress.config.mjs"));
    }

    #[test]
    fn test_find_config_file_cjs() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        fs::write(temp.path().join("cypress.config.cjs"), "module.exports = {}").unwrap();

        let config = plugin.find_config_file(temp.path());
        assert!(config.is_some());
        assert!(config.unwrap().ends_with("cypress.config.cjs"));
    }

    #[test]
    fn test_find_config_file_not_found() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        let config = plugin.find_config_file(temp.path());
        assert!(config.is_none());
    }

    #[test]
    fn test_detect_entries_config_only() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        fs::write(temp.path().join("cypress.config.ts"), "export default {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // Should have config path
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("cypress.config.ts"));

        // Should have test patterns (the default patterns)
        assert!(!patterns.is_empty());
    }

    #[test]
    fn test_detect_entries_e2e_tests() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Create cypress config
        fs::write(temp.path().join("cypress.config.ts"), "export default {}").unwrap();

        // Create e2e directory structure (not needed for pattern-based matching, but kept for context)
        let e2e_dir = temp.path().join("cypress").join("e2e");
        fs::create_dir_all(&e2e_dir).unwrap();
        fs::write(e2e_dir.join("login.cy.ts"), "describe('login', () => {})").unwrap();
        fs::write(e2e_dir.join("signup.spec.ts"), "describe('signup', () => {})").unwrap();
        // Non-test file should not match patterns
        fs::write(e2e_dir.join("utils.ts"), "export const foo = 1").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // Should have config path
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("cypress.config.ts"));

        // Should have e2e test patterns
        let pattern_strs: Vec<&str> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"cypress/e2e/**/*.cy.ts"));
        assert!(pattern_strs.contains(&"cypress/e2e/**/*.spec.ts"));
    }

    #[test]
    fn test_detect_entries_support_files() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Create cypress config
        fs::write(temp.path().join("cypress.config.js"), "module.exports = {}").unwrap();

        // Create support directory structure (not needed for pattern-based matching, but kept for context)
        let support_dir = temp.path().join("cypress").join("support");
        fs::create_dir_all(&support_dir).unwrap();
        fs::write(support_dir.join("commands.ts"), "Cypress.Commands.add()").unwrap();
        fs::write(support_dir.join("e2e.ts"), "import './commands'").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // Should have config path
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("cypress.config.js"));

        // Should have support file patterns
        let pattern_strs: Vec<&str> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"cypress/support/**/*.ts"));
        assert!(pattern_strs.contains(&"cypress/support/**/*.js"));
    }

    #[test]
    fn test_detect_entries_component_tests() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Create cypress config
        fs::write(temp.path().join("cypress.config.ts"), "export default {}").unwrap();

        // Create component directory structure (not needed for pattern-based matching, but kept for context)
        let component_dir = temp.path().join("cypress").join("component");
        fs::create_dir_all(&component_dir).unwrap();
        fs::write(component_dir.join("Button.cy.tsx"), "describe('Button', () => {})").unwrap();
        fs::write(component_dir.join("Card.spec.jsx"), "describe('Card', () => {})").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // Should have config path
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("cypress.config.ts"));

        // Should have component test patterns
        let pattern_strs: Vec<&str> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"cypress/component/**/*.cy.tsx"));
        assert!(pattern_strs.contains(&"cypress/component/**/*.spec.jsx"));
    }

    #[test]
    fn test_detect_entries_nested_directories() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Create cypress config
        fs::write(temp.path().join("cypress.config.ts"), "export default {}").unwrap();

        // Create nested e2e directory structure (not needed for pattern-based matching, but kept for context)
        let nested_dir = temp.path().join("cypress").join("e2e").join("auth").join("flows");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(nested_dir.join("oauth.cy.ts"), "describe('oauth', () => {})").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // Should have config path
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("cypress.config.ts"));

        // Should have patterns that match nested directories via **
        let pattern_strs: Vec<&str> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"cypress/e2e/**/*.cy.ts"));
    }

    #[test]
    fn test_detect_entries_all_extensions() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Create cypress config
        fs::write(temp.path().join("cypress.config.js"), "module.exports = {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // Should have config path
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("cypress.config.js"));

        // Should have patterns for all supported extensions
        let pattern_strs: Vec<&str> = patterns.iter().map(|p| p.pattern.as_str()).collect();

        // Check e2e patterns for all extensions
        assert!(pattern_strs.contains(&"cypress/e2e/**/*.cy.js"));
        assert!(pattern_strs.contains(&"cypress/e2e/**/*.cy.jsx"));
        assert!(pattern_strs.contains(&"cypress/e2e/**/*.cy.ts"));
        assert!(pattern_strs.contains(&"cypress/e2e/**/*.cy.tsx"));
        assert!(pattern_strs.contains(&"cypress/e2e/**/*.spec.js"));
        assert!(pattern_strs.contains(&"cypress/e2e/**/*.spec.jsx"));
        assert!(pattern_strs.contains(&"cypress/e2e/**/*.spec.ts"));
        assert!(pattern_strs.contains(&"cypress/e2e/**/*.spec.tsx"));
    }

    #[test]
    fn test_detect_entries_no_cypress_directory() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Only config, no cypress directory
        fs::write(temp.path().join("cypress.config.ts"), "export default {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // Should have config path
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("cypress.config.ts"));

        // Should still have test patterns (they just won't match anything)
        assert!(!patterns.is_empty());
    }

    #[test]
    fn test_detect_entries_no_config_no_tests() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // No config found, so no paths
        assert!(paths.is_empty());

        // But patterns are always returned
        assert!(!patterns.is_empty());
    }

    #[test]
    fn test_default_impl() {
        let _: CypressPlugin = Default::default();
    }

    #[test]
    fn test_detect_entries_mixed_test_types() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Create cypress config
        fs::write(temp.path().join("cypress.config.ts"), "export default {}").unwrap();

        // Create e2e, support, and component directories (not needed for pattern-based matching, but kept for context)
        let e2e_dir = temp.path().join("cypress").join("e2e");
        let support_dir = temp.path().join("cypress").join("support");
        let component_dir = temp.path().join("cypress").join("component");

        fs::create_dir_all(&e2e_dir).unwrap();
        fs::create_dir_all(&support_dir).unwrap();
        fs::create_dir_all(&component_dir).unwrap();

        fs::write(e2e_dir.join("app.cy.ts"), "").unwrap();
        fs::write(support_dir.join("commands.ts"), "").unwrap();
        fs::write(component_dir.join("Widget.cy.tsx"), "").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // Should have config path
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("cypress.config.ts"));

        // Should have patterns for all test types (e2e, support, component)
        let pattern_strs: Vec<&str> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.iter().any(|p| p.contains("cypress/e2e/")));
        assert!(pattern_strs.iter().any(|p| p.contains("cypress/support/")));
        assert!(pattern_strs.iter().any(|p| p.contains("cypress/component/")));
    }
}
