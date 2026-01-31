use super::{Plugin, PluginError};
use glob::glob;
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

    /// Expand glob patterns relative to the project directory
    fn expand_patterns(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let mut entries = Vec::new();

        for pattern in Self::default_patterns() {
            let full_pattern = cwd.join(pattern);
            let pattern_str = full_pattern.to_string_lossy();

            for entry in glob(&pattern_str)? {
                let path = entry?;
                // Validate path is within project directory to prevent path traversal
                if let Ok(canonical) = path.canonicalize() {
                    if canonical.starts_with(&cwd_canonical) {
                        entries.push(canonical);
                    }
                }
            }
        }

        Ok(entries)
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

    fn detect_entries(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        let mut entries = Vec::new();

        // Add config file as entry point if found
        if let Some(config_path) = self.find_config_file(cwd) {
            if let Ok(canonical) = config_path.canonicalize() {
                entries.push(canonical);
            }
        }

        // Find all test and support files using default patterns
        let test_files = self.expand_patterns(cwd)?;
        entries.extend(test_files);

        Ok(entries)
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
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("cypress.config.ts"));
    }

    #[test]
    fn test_detect_entries_e2e_tests() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Create cypress config
        fs::write(temp.path().join("cypress.config.ts"), "export default {}").unwrap();

        // Create e2e directory structure
        let e2e_dir = temp.path().join("cypress").join("e2e");
        fs::create_dir_all(&e2e_dir).unwrap();
        fs::write(e2e_dir.join("login.cy.ts"), "describe('login', () => {})").unwrap();
        fs::write(e2e_dir.join("signup.spec.ts"), "describe('signup', () => {})").unwrap();
        // Non-test file should not be included
        fs::write(e2e_dir.join("utils.ts"), "export const foo = 1").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 3); // config + 2 test files

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"cypress.config.ts".to_string()));
        assert!(filenames.contains(&"login.cy.ts".to_string()));
        assert!(filenames.contains(&"signup.spec.ts".to_string()));
        assert!(!filenames.contains(&"utils.ts".to_string()));
    }

    #[test]
    fn test_detect_entries_support_files() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Create cypress config
        fs::write(temp.path().join("cypress.config.js"), "module.exports = {}").unwrap();

        // Create support directory structure
        let support_dir = temp.path().join("cypress").join("support");
        fs::create_dir_all(&support_dir).unwrap();
        fs::write(support_dir.join("commands.ts"), "Cypress.Commands.add()").unwrap();
        fs::write(support_dir.join("e2e.ts"), "import './commands'").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 3); // config + 2 support files

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"commands.ts".to_string()));
        assert!(filenames.contains(&"e2e.ts".to_string()));
    }

    #[test]
    fn test_detect_entries_component_tests() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Create cypress config
        fs::write(temp.path().join("cypress.config.ts"), "export default {}").unwrap();

        // Create component directory structure
        let component_dir = temp.path().join("cypress").join("component");
        fs::create_dir_all(&component_dir).unwrap();
        fs::write(component_dir.join("Button.cy.tsx"), "describe('Button', () => {})").unwrap();
        fs::write(component_dir.join("Card.spec.jsx"), "describe('Card', () => {})").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 3); // config + 2 component tests

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"Button.cy.tsx".to_string()));
        assert!(filenames.contains(&"Card.spec.jsx".to_string()));
    }

    #[test]
    fn test_detect_entries_nested_directories() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Create cypress config
        fs::write(temp.path().join("cypress.config.ts"), "export default {}").unwrap();

        // Create nested e2e directory structure
        let nested_dir = temp.path().join("cypress").join("e2e").join("auth").join("flows");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(nested_dir.join("oauth.cy.ts"), "describe('oauth', () => {})").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2); // config + nested test

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"oauth.cy.ts".to_string()));
    }

    #[test]
    fn test_detect_entries_all_extensions() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Create cypress config
        fs::write(temp.path().join("cypress.config.js"), "module.exports = {}").unwrap();

        // Create e2e tests with different extensions
        let e2e_dir = temp.path().join("cypress").join("e2e");
        fs::create_dir_all(&e2e_dir).unwrap();
        fs::write(e2e_dir.join("test1.cy.js"), "").unwrap();
        fs::write(e2e_dir.join("test2.cy.jsx"), "").unwrap();
        fs::write(e2e_dir.join("test3.cy.ts"), "").unwrap();
        fs::write(e2e_dir.join("test4.cy.tsx"), "").unwrap();
        fs::write(e2e_dir.join("test5.spec.js"), "").unwrap();
        fs::write(e2e_dir.join("test6.spec.jsx"), "").unwrap();
        fs::write(e2e_dir.join("test7.spec.ts"), "").unwrap();
        fs::write(e2e_dir.join("test8.spec.tsx"), "").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 9); // config + 8 test files
    }

    #[test]
    fn test_detect_entries_no_cypress_directory() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        // Only config, no cypress directory
        fs::write(temp.path().join("cypress.config.ts"), "export default {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1); // Just the config file
    }

    #[test]
    fn test_detect_entries_no_config_no_tests() {
        let plugin = CypressPlugin::new();
        let temp = tempdir().unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert!(entries.is_empty());
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

        // Create e2e, support, and component directories
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
        assert_eq!(entries.len(), 4); // config + e2e + support + component

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"cypress.config.ts".to_string()));
        assert!(filenames.contains(&"app.cy.ts".to_string()));
        assert!(filenames.contains(&"commands.ts".to_string()));
        assert!(filenames.contains(&"Widget.cy.tsx".to_string()));
    }
}
