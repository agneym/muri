use super::{Plugin, PluginError};
use glob::glob;
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

/// Plugin to discover Playwright test files as entry points
pub struct PlaywrightPlugin;

impl PlaywrightPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find the Playwright config file in the project root
    fn find_config_file(&self, cwd: &Path) -> Option<PathBuf> {
        let extensions = ["js", "ts", "mjs", "cjs"];
        for ext in extensions {
            let path = cwd.join(format!("playwright.config.{}", ext));
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    /// Default test patterns for Playwright projects
    fn default_patterns() -> &'static [&'static str] {
        &[
            "tests/**/*.spec.js",
            "tests/**/*.spec.ts",
            "tests/**/*.test.js",
            "tests/**/*.test.ts",
            "e2e/**/*.spec.js",
            "e2e/**/*.spec.ts",
            "e2e/**/*.test.js",
            "e2e/**/*.test.ts",
        ]
    }

    /// Expand glob patterns relative to the project directory
    fn expand_patterns(&self, patterns: &[&str], cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let mut entries = Vec::new();

        for pattern in patterns {
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

impl Default for PlaywrightPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for PlaywrightPlugin {
    fn name(&self) -> &str {
        "playwright"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("@playwright/test")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        let mut entries = Vec::new();

        // Add config file as entry point if it exists
        if let Some(config_path) = self.find_config_file(cwd) {
            if let Ok(canonical) = config_path.canonicalize() {
                entries.push(canonical);
            }
        }

        // Find test files using default patterns
        let test_files = self.expand_patterns(Self::default_patterns(), cwd)?;
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
    fn test_should_enable_with_playwright_test() {
        let plugin = PlaywrightPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("@playwright/test".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_playwright() {
        let plugin = PlaywrightPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("react".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_with_playwright_core_only() {
        let plugin = PlaywrightPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("playwright-core".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_config_file_as_entry_point() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create playwright.config.ts
        let config_content = r#"
import { defineConfig } from '@playwright/test';
export default defineConfig({});
"#;
        fs::write(temp.path().join("playwright.config.ts"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("playwright.config.ts"));
    }

    #[test]
    fn test_config_file_js_extension() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create playwright.config.js
        let config_content = r#"
module.exports = {};
"#;
        fs::write(temp.path().join("playwright.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("playwright.config.js"));
    }

    #[test]
    fn test_config_file_mjs_extension() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create playwright.config.mjs
        let config_content = r#"
export default {};
"#;
        fs::write(temp.path().join("playwright.config.mjs"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("playwright.config.mjs"));
    }

    #[test]
    fn test_detect_tests_directory_spec_files() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create tests directory with spec files
        let tests_dir = temp.path().join("tests");
        fs::create_dir(&tests_dir).unwrap();
        fs::write(tests_dir.join("login.spec.ts"), "test('login', async () => {});").unwrap();
        fs::write(tests_dir.join("signup.spec.ts"), "test('signup', async () => {});").unwrap();
        fs::write(tests_dir.join("utils.ts"), "export const helper = () => {};").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"login.spec.ts".to_string()));
        assert!(filenames.contains(&"signup.spec.ts".to_string()));
        assert!(!filenames.contains(&"utils.ts".to_string()));
    }

    #[test]
    fn test_detect_tests_directory_test_files() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create tests directory with test files (alternative naming)
        let tests_dir = temp.path().join("tests");
        fs::create_dir(&tests_dir).unwrap();
        fs::write(tests_dir.join("login.test.ts"), "test('login', async () => {});").unwrap();
        fs::write(tests_dir.join("signup.test.js"), "test('signup', async () => {});").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"login.test.ts".to_string()));
        assert!(filenames.contains(&"signup.test.js".to_string()));
    }

    #[test]
    fn test_detect_e2e_directory_spec_files() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create e2e directory with spec files
        let e2e_dir = temp.path().join("e2e");
        fs::create_dir(&e2e_dir).unwrap();
        fs::write(e2e_dir.join("checkout.spec.ts"), "test('checkout', async () => {});").unwrap();
        fs::write(e2e_dir.join("cart.spec.js"), "test('cart', async () => {});").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"checkout.spec.ts".to_string()));
        assert!(filenames.contains(&"cart.spec.js".to_string()));
    }

    #[test]
    fn test_detect_e2e_directory_test_files() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create e2e directory with test files
        let e2e_dir = temp.path().join("e2e");
        fs::create_dir(&e2e_dir).unwrap();
        fs::write(e2e_dir.join("checkout.test.ts"), "test('checkout', async () => {});").unwrap();
        fs::write(e2e_dir.join("cart.test.js"), "test('cart', async () => {});").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"checkout.test.ts".to_string()));
        assert!(filenames.contains(&"cart.test.js".to_string()));
    }

    #[test]
    fn test_detect_nested_test_files() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create nested directory structure
        let tests_dir = temp.path().join("tests");
        let auth_dir = tests_dir.join("auth");
        let checkout_dir = tests_dir.join("checkout");
        fs::create_dir_all(&auth_dir).unwrap();
        fs::create_dir_all(&checkout_dir).unwrap();

        fs::write(auth_dir.join("login.spec.ts"), "test('login', async () => {});").unwrap();
        fs::write(checkout_dir.join("payment.spec.ts"), "test('payment', async () => {});")
            .unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"login.spec.ts".to_string()));
        assert!(filenames.contains(&"payment.spec.ts".to_string()));
    }

    #[test]
    fn test_detect_config_and_tests() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create config file
        fs::write(temp.path().join("playwright.config.ts"), "export default {};").unwrap();

        // Create tests
        let tests_dir = temp.path().join("tests");
        fs::create_dir(&tests_dir).unwrap();
        fs::write(tests_dir.join("login.spec.ts"), "test('login', async () => {});").unwrap();

        let e2e_dir = temp.path().join("e2e");
        fs::create_dir(&e2e_dir).unwrap();
        fs::write(e2e_dir.join("checkout.spec.ts"), "test('checkout', async () => {});").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 3);

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"playwright.config.ts".to_string()));
        assert!(filenames.contains(&"login.spec.ts".to_string()));
        assert!(filenames.contains(&"checkout.spec.ts".to_string()));
    }

    #[test]
    fn test_no_entries_when_no_tests_or_config() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create src directory with regular files (not tests)
        let src_dir = temp.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("app.ts"), "export const app = {};").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_plugin_name() {
        let plugin = PlaywrightPlugin::new();
        assert_eq!(plugin.name(), "playwright");
    }

    #[test]
    fn test_default_impl() {
        let _: PlaywrightPlugin = Default::default();
    }
}
