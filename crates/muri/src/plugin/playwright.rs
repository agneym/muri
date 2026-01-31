use super::{EntryPattern, Plugin, PluginEntries, PluginError};
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

    /// Convert default patterns to EntryPatterns
    fn patterns_to_entry_patterns() -> Vec<EntryPattern> {
        Self::default_patterns().iter().map(|p| EntryPattern::new(*p)).collect()
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
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("playwright.config.ts"));
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
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("playwright.config.js"));
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
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("playwright.config.mjs"));
    }

    #[test]
    fn test_detect_tests_directory_spec_files() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create tests directory with spec files (for verifying patterns would match them)
        let tests_dir = temp.path().join("tests");
        fs::create_dir(&tests_dir).unwrap();
        fs::write(tests_dir.join("login.spec.ts"), "test('login', async () => {});").unwrap();
        fs::write(tests_dir.join("signup.spec.ts"), "test('signup', async () => {});").unwrap();
        fs::write(tests_dir.join("utils.ts"), "export const helper = () => {};").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let patterns = entries.get_patterns();

        // Should have patterns for tests/**/*.spec.ts
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"tests/**/*.spec.ts"));
        assert!(pattern_strs.contains(&"tests/**/*.spec.js"));
        // utils.ts would not match because it doesn't have .spec. or .test. suffix
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
        let patterns = entries.get_patterns();

        // Should have patterns for tests/**/*.test.ts and tests/**/*.test.js
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"tests/**/*.test.ts"));
        assert!(pattern_strs.contains(&"tests/**/*.test.js"));
    }

    #[test]
    fn test_detect_e2e_directory_spec_files() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create e2e directory with spec files (for verifying patterns would match them)
        let e2e_dir = temp.path().join("e2e");
        fs::create_dir(&e2e_dir).unwrap();
        fs::write(e2e_dir.join("checkout.spec.ts"), "test('checkout', async () => {});").unwrap();
        fs::write(e2e_dir.join("cart.spec.js"), "test('cart', async () => {});").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let patterns = entries.get_patterns();

        // Should have patterns for e2e/**/*.spec.ts and e2e/**/*.spec.js
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"e2e/**/*.spec.ts"));
        assert!(pattern_strs.contains(&"e2e/**/*.spec.js"));
    }

    #[test]
    fn test_detect_e2e_directory_test_files() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create e2e directory with test files (for verifying patterns would match them)
        let e2e_dir = temp.path().join("e2e");
        fs::create_dir(&e2e_dir).unwrap();
        fs::write(e2e_dir.join("checkout.test.ts"), "test('checkout', async () => {});").unwrap();
        fs::write(e2e_dir.join("cart.test.js"), "test('cart', async () => {});").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let patterns = entries.get_patterns();

        // Should have patterns for e2e/**/*.test.ts and e2e/**/*.test.js
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"e2e/**/*.test.ts"));
        assert!(pattern_strs.contains(&"e2e/**/*.test.js"));
    }

    #[test]
    fn test_detect_nested_test_files() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create nested directory structure (to verify the ** glob would match nested files)
        let tests_dir = temp.path().join("tests");
        let auth_dir = tests_dir.join("auth");
        let checkout_dir = tests_dir.join("checkout");
        fs::create_dir_all(&auth_dir).unwrap();
        fs::create_dir_all(&checkout_dir).unwrap();

        fs::write(auth_dir.join("login.spec.ts"), "test('login', async () => {});").unwrap();
        fs::write(checkout_dir.join("payment.spec.ts"), "test('payment', async () => {});")
            .unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let patterns = entries.get_patterns();

        // The patterns use ** which matches nested directories
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"tests/**/*.spec.ts"));
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

        // Config file should be in paths
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("playwright.config.ts"));

        // Test patterns should be in patterns
        let patterns = entries.get_patterns();
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"tests/**/*.spec.ts"));
        assert!(pattern_strs.contains(&"e2e/**/*.spec.ts"));
    }

    #[test]
    fn test_no_config_file_still_returns_patterns() {
        let plugin = PlaywrightPlugin::new();
        let temp = tempdir().unwrap();

        // Create src directory with regular files (not tests)
        let src_dir = temp.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("app.ts"), "export const app = {};").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();

        // No config file means no paths
        let paths = entries.get_paths();
        assert!(paths.is_empty());

        // But patterns are still returned (they just won't match any files)
        let patterns = entries.get_patterns();
        assert!(!patterns.is_empty());
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
