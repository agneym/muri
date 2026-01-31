use super::{EntryPattern, Plugin, PluginEntries, PluginError};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, Expression, ModuleDeclaration, ObjectPropertyKind, PropertyKey, Statement,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use rustc_hash::{FxHashMap, FxHashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Default test file patterns used by Vitest when no include patterns are specified
/// Expanded from: **/*.{test,spec}.{js,mjs,cjs,ts,mts,cts,jsx,tsx}
const DEFAULT_TEST_PATTERNS: &[&str] = &[
    "**/*.test.js",
    "**/*.test.mjs",
    "**/*.test.cjs",
    "**/*.test.ts",
    "**/*.test.mts",
    "**/*.test.cts",
    "**/*.test.jsx",
    "**/*.test.tsx",
    "**/*.spec.js",
    "**/*.spec.mjs",
    "**/*.spec.cjs",
    "**/*.spec.ts",
    "**/*.spec.mts",
    "**/*.spec.cts",
    "**/*.spec.jsx",
    "**/*.spec.tsx",
    "**/__tests__/**/*.js",
    "**/__tests__/**/*.mjs",
    "**/__tests__/**/*.cjs",
    "**/__tests__/**/*.ts",
    "**/__tests__/**/*.mts",
    "**/__tests__/**/*.cts",
    "**/__tests__/**/*.jsx",
    "**/__tests__/**/*.tsx",
];

/// Plugin to discover Vitest test files and setup files as entry points
pub struct VitestPlugin;

impl VitestPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find vitest config file in the project
    fn find_vitest_config(&self, cwd: &Path) -> Option<PathBuf> {
        let extensions = ["js", "ts", "mjs", "cjs"];
        for ext in extensions {
            let path = cwd.join(format!("vitest.config.{}", ext));
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    /// Find vite config file (vitest can be configured in vite.config.*)
    fn find_vite_config(&self, cwd: &Path) -> Option<PathBuf> {
        let extensions = ["js", "ts", "mjs", "cjs"];
        for ext in extensions {
            let path = cwd.join(format!("vite.config.{}", ext));
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    /// Parse vitest/vite config and extract test configuration
    fn parse_config(&self, config_path: &Path) -> Result<VitestConfig, PluginError> {
        let content = fs::read_to_string(config_path)?;
        let allocator = Allocator::default();
        let source_type = SourceType::from_path(config_path).unwrap_or_default();
        let parsed = Parser::new(&allocator, &content, source_type).parse();

        if parsed.panicked {
            return Err(PluginError::ConfigParse(format!(
                "Failed to parse {}",
                config_path.display()
            )));
        }

        // First pass: collect variable declarations
        let mut var_configs: FxHashMap<String, VitestConfig> = FxHashMap::default();

        for stmt in &parsed.program.body {
            if let Some((name, config)) = self.extract_config_from_var_decl(stmt) {
                var_configs.insert(name, config);
            }
        }

        // Second pass: look for exports
        for stmt in &parsed.program.body {
            if let Some(config) = self.extract_config_from_statement(stmt, &var_configs) {
                return Ok(config);
            }
        }

        Ok(VitestConfig::default())
    }

    /// Extract config from a variable declaration
    fn extract_config_from_var_decl(&self, stmt: &Statement) -> Option<(String, VitestConfig)> {
        if let Statement::VariableDeclaration(var_decl) = stmt {
            for decl in &var_decl.declarations {
                let name = decl.id.get_identifier_name()?;

                if let Some(Expression::ObjectExpression(obj)) = &decl.init {
                    if let Some(config) = self.extract_config_from_object(obj) {
                        return Some((name.to_string(), config));
                    }
                }
            }
        }
        None
    }

    /// Extract vitest config from a statement
    fn extract_config_from_statement(
        &self,
        stmt: &Statement,
        var_configs: &FxHashMap<String, VitestConfig>,
    ) -> Option<VitestConfig> {
        match stmt {
            // Handle: module.exports = { ... }
            Statement::ExpressionStatement(expr_stmt) => {
                if let Expression::AssignmentExpression(assign) = &expr_stmt.expression {
                    if let Expression::ObjectExpression(obj) = &assign.right {
                        return self.extract_config_from_object(obj);
                    }
                }
                None
            }
            _ => {
                // Handle: export default ...
                if let Some(ModuleDeclaration::ExportDefaultDeclaration(export)) =
                    stmt.as_module_declaration()
                {
                    match &export.declaration {
                        oxc_ast::ast::ExportDefaultDeclarationKind::ObjectExpression(obj) => {
                            return self.extract_config_from_object(obj);
                        }
                        oxc_ast::ast::ExportDefaultDeclarationKind::CallExpression(call) => {
                            // Handle: export default defineConfig({ ... })
                            if let Some(Argument::ObjectExpression(obj)) = call.arguments.first() {
                                return self.extract_config_from_object(obj);
                            }
                        }
                        oxc_ast::ast::ExportDefaultDeclarationKind::Identifier(ident) => {
                            if let Some(config) = var_configs.get(ident.name.as_str()) {
                                return Some(config.clone());
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
        }
    }

    /// Extract vitest config from an object expression (looks for 'test' key or test-related keys)
    fn extract_config_from_object(
        &self,
        obj: &oxc_ast::ast::ObjectExpression,
    ) -> Option<VitestConfig> {
        let mut config = VitestConfig::default();
        let mut found_test_key = false;

        for prop in &obj.properties {
            if let ObjectPropertyKind::ObjectProperty(property) = prop {
                let key_name = match &property.key {
                    PropertyKey::StaticIdentifier(ident) => Some(ident.name.as_str()),
                    PropertyKey::StringLiteral(lit) => Some(lit.value.as_str()),
                    _ => None,
                };

                match key_name {
                    // Vitest config can be nested under 'test' key (in vite.config.*)
                    Some("test") => {
                        if let Expression::ObjectExpression(test_obj) = &property.value {
                            if let Some(test_config) =
                                self.extract_test_config_from_object(test_obj)
                            {
                                return Some(test_config);
                            }
                        }
                        found_test_key = true;
                    }
                    // Direct vitest.config.* keys
                    Some("include") => {
                        if let Some(patterns) = self.extract_string_array(&property.value) {
                            config.include = patterns;
                        }
                    }
                    Some("exclude") => {
                        if let Some(patterns) = self.extract_string_array(&property.value) {
                            config.exclude = patterns;
                        }
                    }
                    Some("setupFiles") => {
                        if let Some(files) = self.extract_string_or_array(&property.value) {
                            config.setup_files = files;
                        }
                    }
                    Some("globalSetup") => {
                        if let Some(files) = self.extract_string_or_array(&property.value) {
                            config.global_setup = files;
                        }
                    }
                    _ => {}
                }
            }
        }

        // If we found a test key but no test config extracted, or found any test-related keys
        if found_test_key
            || !config.include.is_empty()
            || !config.setup_files.is_empty()
            || !config.global_setup.is_empty()
        {
            Some(config)
        } else {
            // Check if this looks like a vitest config at all
            // by checking for other common vitest keys
            for prop in &obj.properties {
                if let ObjectPropertyKind::ObjectProperty(property) = prop {
                    let key_name = match &property.key {
                        PropertyKey::StaticIdentifier(ident) => Some(ident.name.as_str()),
                        PropertyKey::StringLiteral(lit) => Some(lit.value.as_str()),
                        _ => None,
                    };

                    // Common vitest config keys that indicate this is a vitest config
                    if matches!(
                        key_name,
                        Some("environment")
                            | Some("globals")
                            | Some("testTimeout")
                            | Some("coverage")
                            | Some("reporters")
                    ) {
                        return Some(config);
                    }
                }
            }
            None
        }
    }

    /// Extract test config from a nested 'test' object
    fn extract_test_config_from_object(
        &self,
        obj: &oxc_ast::ast::ObjectExpression,
    ) -> Option<VitestConfig> {
        let mut config = VitestConfig::default();

        for prop in &obj.properties {
            if let ObjectPropertyKind::ObjectProperty(property) = prop {
                let key_name = match &property.key {
                    PropertyKey::StaticIdentifier(ident) => Some(ident.name.as_str()),
                    PropertyKey::StringLiteral(lit) => Some(lit.value.as_str()),
                    _ => None,
                };

                match key_name {
                    Some("include") => {
                        if let Some(patterns) = self.extract_string_array(&property.value) {
                            config.include = patterns;
                        }
                    }
                    Some("exclude") => {
                        if let Some(patterns) = self.extract_string_array(&property.value) {
                            config.exclude = patterns;
                        }
                    }
                    Some("setupFiles") => {
                        if let Some(files) = self.extract_string_or_array(&property.value) {
                            config.setup_files = files;
                        }
                    }
                    Some("globalSetup") => {
                        if let Some(files) = self.extract_string_or_array(&property.value) {
                            config.global_setup = files;
                        }
                    }
                    _ => {}
                }
            }
        }

        Some(config)
    }

    /// Extract an array of strings from an expression
    fn extract_string_array(&self, expr: &Expression) -> Option<Vec<String>> {
        if let Expression::ArrayExpression(arr) = expr {
            let mut patterns = Vec::new();
            for elem in &arr.elements {
                if let Some(elem_expr) = elem.as_expression() {
                    if let Some(s) = self.extract_string(elem_expr) {
                        patterns.push(s);
                    }
                }
            }
            if !patterns.is_empty() {
                return Some(patterns);
            }
        }
        None
    }

    /// Extract a string or array of strings from an expression
    fn extract_string_or_array(&self, expr: &Expression) -> Option<Vec<String>> {
        // Try as array first
        if let Some(arr) = self.extract_string_array(expr) {
            return Some(arr);
        }
        // Try as single string
        if let Some(s) = self.extract_string(expr) {
            return Some(vec![s]);
        }
        None
    }

    /// Extract a string value from an expression
    fn extract_string(&self, expr: &Expression) -> Option<String> {
        match expr {
            Expression::StringLiteral(lit) => Some(lit.value.to_string()),
            Expression::TemplateLiteral(tpl) if tpl.expressions.is_empty() => {
                tpl.quasis.first().map(|q| q.value.raw.to_string())
            }
            _ => None,
        }
    }

    /// Convert include patterns to EntryPatterns
    /// Note: Vitest exclude patterns are not currently handled as the collector
    /// already excludes node_modules and common build directories
    fn patterns_to_entry_patterns(patterns: &[String]) -> Vec<EntryPattern> {
        patterns.iter().map(|p| EntryPattern::new(p.clone())).collect()
    }

    /// Resolve setup files to absolute paths
    fn resolve_setup_files(
        &self,
        files: &[String],
        cwd: &Path,
    ) -> Result<Vec<PathBuf>, PluginError> {
        let mut resolved = Vec::new();
        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());

        for file in files {
            let path = cwd.join(file);

            // Try exact path first
            if path.exists() {
                if let Ok(canonical) = path.canonicalize() {
                    if canonical.starts_with(&cwd_canonical) {
                        resolved.push(canonical);
                        continue;
                    }
                }
            }

            // Try with common extensions
            for ext in &["", ".ts", ".js", ".mts", ".mjs", ".cts", ".cjs"] {
                let with_ext = if ext.is_empty() {
                    path.clone()
                } else {
                    PathBuf::from(format!("{}{}", path.display(), ext))
                };

                if with_ext.exists() {
                    if let Ok(canonical) = with_ext.canonicalize() {
                        if canonical.starts_with(&cwd_canonical) {
                            resolved.push(canonical);
                            break;
                        }
                    }
                }
            }
        }

        Ok(resolved)
    }
}

impl Default for VitestPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed vitest configuration
#[derive(Debug, Clone, Default)]
struct VitestConfig {
    include: Vec<String>,
    exclude: Vec<String>,
    setup_files: Vec<String>,
    global_setup: Vec<String>,
}

impl Plugin for VitestPlugin {
    fn name(&self) -> &str {
        "vitest"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("vitest")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<PluginEntries, PluginError> {
        let mut paths = Vec::new();

        // Try to find and parse vitest config first
        let config_and_path = if let Some(vitest_config_path) = self.find_vitest_config(cwd) {
            let config = self.parse_config(&vitest_config_path).unwrap_or_default();
            Some((config, vitest_config_path))
        } else if let Some(vite_config_path) = self.find_vite_config(cwd) {
            // Check if vite.config.* has a 'test' key
            let config = self.parse_config(&vite_config_path).unwrap_or_default();
            Some((config, vite_config_path))
        } else {
            None
        };

        let config = if let Some((config, config_path)) = config_and_path {
            // Add config file as entry point (path, not pattern)
            if let Ok(canonical) = config_path.canonicalize() {
                paths.push(canonical);
            }
            config
        } else {
            VitestConfig::default()
        };

        // Determine include patterns
        let include_patterns: Vec<String> = if config.include.is_empty() {
            DEFAULT_TEST_PATTERNS.iter().map(|s| s.to_string()).collect()
        } else {
            config.include.clone()
        };

        // Convert to EntryPatterns
        let entry_patterns = Self::patterns_to_entry_patterns(&include_patterns);

        // Resolve setup files (paths, not patterns)
        let setup_files = self.resolve_setup_files(&config.setup_files, cwd)?;
        paths.extend(setup_files);

        // Resolve global setup files
        let global_setup_files = self.resolve_setup_files(&config.global_setup, cwd)?;
        paths.extend(global_setup_files);

        Ok(PluginEntries::mixed(entry_patterns, paths))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_with_vitest() {
        let plugin = VitestPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("vitest".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_vitest() {
        let plugin = VitestPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("jest".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_name() {
        let plugin = VitestPlugin::new();
        assert_eq!(plugin.name(), "vitest");
    }

    #[test]
    fn test_default_impl() {
        let _: VitestPlugin = Default::default();
    }

    #[test]
    fn test_find_vitest_config_js() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        fs::write(temp.path().join("vitest.config.js"), "export default {}").unwrap();

        let config = plugin.find_vitest_config(temp.path());
        assert!(config.is_some());
        assert!(config.unwrap().ends_with("vitest.config.js"));
    }

    #[test]
    fn test_find_vitest_config_ts() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        fs::write(temp.path().join("vitest.config.ts"), "export default {}").unwrap();

        let config = plugin.find_vitest_config(temp.path());
        assert!(config.is_some());
        assert!(config.unwrap().ends_with("vitest.config.ts"));
    }

    #[test]
    fn test_find_vite_config_fallback() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        fs::write(temp.path().join("vite.config.ts"), "export default {}").unwrap();

        let vitest_config = plugin.find_vitest_config(temp.path());
        assert!(vitest_config.is_none());

        let vite_config = plugin.find_vite_config(temp.path());
        assert!(vite_config.is_some());
        assert!(vite_config.unwrap().ends_with("vite.config.ts"));
    }

    #[test]
    fn test_parse_basic_vitest_config() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
export default {
  include: ['src/**/*.test.ts'],
  exclude: ['**/node_modules/**'],
  setupFiles: ['./setup.ts'],
};
"#;
        let config_path = temp.path().join("vitest.config.ts");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_config(&config_path).unwrap();
        assert_eq!(config.include, vec!["src/**/*.test.ts"]);
        assert_eq!(config.exclude, vec!["**/node_modules/**"]);
        assert_eq!(config.setup_files, vec!["./setup.ts"]);
    }

    #[test]
    fn test_parse_define_config_style() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
import { defineConfig } from 'vitest/config';

export default defineConfig({
  include: ['tests/**/*.spec.ts'],
  globalSetup: './global-setup.ts',
});
"#;
        let config_path = temp.path().join("vitest.config.ts");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_config(&config_path).unwrap();
        assert_eq!(config.include, vec!["tests/**/*.spec.ts"]);
        assert_eq!(config.global_setup, vec!["./global-setup.ts"]);
    }

    #[test]
    fn test_parse_vite_config_with_test_key() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [],
  test: {
    include: ['src/**/*.test.tsx'],
    setupFiles: ['./test/setup.ts'],
  },
});
"#;
        let config_path = temp.path().join("vite.config.ts");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_config(&config_path).unwrap();
        assert_eq!(config.include, vec!["src/**/*.test.tsx"]);
        assert_eq!(config.setup_files, vec!["./test/setup.ts"]);
    }

    #[test]
    fn test_parse_variable_export_config() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
const config = {
  include: ['lib/**/*.test.js'],
  environment: 'node',
};

export default config;
"#;
        let config_path = temp.path().join("vitest.config.js");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_config(&config_path).unwrap();
        assert_eq!(config.include, vec!["lib/**/*.test.js"]);
    }

    #[test]
    fn test_parse_setup_files_as_array() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
export default {
  setupFiles: ['./setup-dom.ts', './setup-mocks.ts'],
};
"#;
        let config_path = temp.path().join("vitest.config.ts");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_config(&config_path).unwrap();
        assert_eq!(config.setup_files, vec!["./setup-dom.ts", "./setup-mocks.ts"]);
    }

    #[test]
    fn test_detect_entries_with_default_patterns() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let patterns = entries.get_patterns();

        // Should have default test patterns (no config file, so uses defaults)
        assert!(!patterns.is_empty());
        // Verify some expected default patterns are present
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"**/*.test.ts"));
        assert!(pattern_strs.contains(&"**/*.spec.js"));
        assert!(pattern_strs.contains(&"**/__tests__/**/*.ts"));
    }

    #[test]
    fn test_detect_entries_with_config() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        // Create vitest config
        let config_content = r#"
export default {
  include: ['tests/**/*.test.ts'],
};
"#;
        fs::write(temp.path().join("vitest.config.ts"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // Should include config file in paths
        assert!(paths.iter().any(|p| p.ends_with("vitest.config.ts")));

        // Should have custom test patterns (not defaults)
        assert!(!patterns.is_empty());
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"tests/**/*.test.ts"));
        // Should NOT include default patterns when custom include is specified
        assert_eq!(patterns.len(), 1);
    }

    #[test]
    fn test_detect_entries_with_setup_files() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        // Create vitest config with setup files
        let config_content = r#"
export default {
  setupFiles: ['./test/setup.ts'],
  globalSetup: './test/global-setup.ts',
};
"#;
        fs::write(temp.path().join("vitest.config.ts"), config_content).unwrap();

        // Create setup files
        let test_dir = temp.path().join("test");
        fs::create_dir(&test_dir).unwrap();
        fs::write(test_dir.join("setup.ts"), "// setup").unwrap();
        fs::write(test_dir.join("global-setup.ts"), "// global setup").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();

        // Config file and setup files should be in paths
        let filenames: Vec<_> =
            paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();

        assert!(filenames.contains(&"vitest.config.ts".to_string()));
        assert!(filenames.contains(&"setup.ts".to_string()));
        assert!(filenames.contains(&"global-setup.ts".to_string()));
    }

    #[test]
    fn test_detect_entries_excludes_node_modules() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        // Note: The plugin returns patterns, not resolved paths for test files.
        // Exclusion of node_modules happens at the collector level when matching patterns.
        // This test verifies the plugin returns the expected patterns.
        let entries = plugin.detect_entries(temp.path()).unwrap();
        let patterns = entries.get_patterns();

        // Should have default test patterns
        assert!(!patterns.is_empty());
        // Patterns themselves don't include node_modules - that's handled by the collector
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.iter().any(|p| p.contains("**/*.test.ts")));
    }

    #[test]
    fn test_detect_entries_with_vite_config() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        // Create vite config with test key (no vitest.config.*)
        let config_content = r#"
import { defineConfig } from 'vite';

export default defineConfig({
  test: {
    include: ['src/**/*.spec.ts'],
  },
});
"#;
        fs::write(temp.path().join("vite.config.ts"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // Config file should be in paths
        let path_filenames: Vec<_> =
            paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(path_filenames.contains(&"vite.config.ts".to_string()));

        // Test patterns should be in patterns
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"src/**/*.spec.ts"));
    }

    #[test]
    fn test_detect_entries_no_config() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        // No config file, should use default patterns
        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        let patterns = entries.get_patterns();

        // Should NOT include any config file (none exists)
        assert!(paths.is_empty());

        // Should have default test patterns
        assert!(!patterns.is_empty());
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.contains(&"**/*.test.ts"));
    }

    #[test]
    fn test_detect_entries_tests_directory() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        // __tests__ directory pattern is in the default patterns
        let entries = plugin.detect_entries(temp.path()).unwrap();
        let patterns = entries.get_patterns();

        // Should have pattern for __tests__ directory
        let pattern_strs: Vec<_> = patterns.iter().map(|p| p.pattern.as_str()).collect();
        assert!(pattern_strs.iter().any(|p| p.contains("__tests__")));
    }

    #[test]
    fn test_parse_empty_config() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = "export default {};";
        let config_path = temp.path().join("vitest.config.ts");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_config(&config_path).unwrap();
        assert!(config.include.is_empty());
        assert!(config.exclude.is_empty());
        assert!(config.setup_files.is_empty());
    }

    #[test]
    fn test_resolve_setup_files_with_extension() {
        let plugin = VitestPlugin::new();
        let temp = tempdir().unwrap();

        // Create setup file without extension in path
        fs::write(temp.path().join("setup.ts"), "// setup").unwrap();

        let resolved = plugin.resolve_setup_files(&["./setup".to_string()], temp.path()).unwrap();

        assert_eq!(resolved.len(), 1);
        assert!(resolved[0].ends_with("setup.ts"));
    }
}
