use super::{Plugin, PluginError};
use glob::glob;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, Expression, ModuleDeclaration, ObjectPropertyKind, PropertyKey, Statement,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use rustc_hash::{FxHashMap, FxHashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Default test patterns used by Jest when no testMatch or testRegex is specified
/// These are expanded versions of Jest's default patterns that work with the glob crate
const DEFAULT_TEST_PATTERNS: &[&str] = &[
    // **/__tests__/**/*.[jt]s?(x) expanded
    "**/__tests__/**/*.js",
    "**/__tests__/**/*.jsx",
    "**/__tests__/**/*.ts",
    "**/__tests__/**/*.tsx",
    // **/?(*.)+(spec|test).[jt]s?(x) expanded - files ending in .spec or .test
    "**/*.spec.js",
    "**/*.spec.jsx",
    "**/*.spec.ts",
    "**/*.spec.tsx",
    "**/*.test.js",
    "**/*.test.jsx",
    "**/*.test.ts",
    "**/*.test.tsx",
];

/// Extensions to try when resolving module paths
const RESOLVE_EXTENSIONS: &[&str] = &[".js", ".ts", ".mjs", ".cjs", ".jsx", ".tsx"];

/// Index file names to try for directory imports
const INDEX_FILES: &[&str] =
    &["index.js", "index.ts", "index.jsx", "index.tsx", "index.mjs", "index.cjs"];

/// Plugin to discover Jest test files and configuration as entry points
pub struct JestPlugin;

impl JestPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find Jest config files in the project
    fn find_config_files(&self, cwd: &Path) -> Vec<PathBuf> {
        let extensions = ["js", "ts", "mjs", "cjs"];
        let mut found = Vec::new();

        // Check for jest.config.{js,ts,mjs,cjs}
        for ext in &extensions {
            let path = cwd.join(format!("jest.config.{}", ext));
            if path.exists() {
                found.push(path);
            }
        }

        // Check for jest.config.json
        let json_config = cwd.join("jest.config.json");
        if json_config.exists() {
            found.push(json_config);
        }

        found
    }

    /// Parse Jest config from package.json
    fn parse_package_json_config(&self, cwd: &Path) -> Option<JestConfig> {
        let package_json_path = cwd.join("package.json");
        if !package_json_path.exists() {
            return None;
        }

        let content = fs::read_to_string(&package_json_path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;

        let jest_config = json.get("jest")?;
        self.extract_config_from_json(jest_config)
    }

    /// Parse Jest config from a JSON file
    fn parse_json_config(&self, config_path: &Path) -> Result<JestConfig, PluginError> {
        let content = fs::read_to_string(config_path)?;
        let json: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| PluginError::ConfigParse(format!("Failed to parse JSON: {}", e)))?;

        self.extract_config_from_json(&json)
            .ok_or_else(|| PluginError::ConfigParse("Invalid Jest config".to_string()))
    }

    /// Extract Jest config from a JSON value
    fn extract_config_from_json(&self, json: &serde_json::Value) -> Option<JestConfig> {
        let obj = json.as_object()?;

        let test_match = obj.get("testMatch").and_then(|v| {
            v.as_array()
                .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        });

        let test_regex = obj.get("testRegex").and_then(|v| match v {
            serde_json::Value::String(s) => Some(vec![s.clone()]),
            serde_json::Value::Array(arr) => {
                Some(arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
            }
            _ => None,
        });

        let setup_files = obj.get("setupFiles").and_then(|v| {
            v.as_array()
                .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        });

        let setup_files_after_env = obj.get("setupFilesAfterEnv").and_then(|v| {
            v.as_array()
                .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        });

        let transform = obj.get("transform").and_then(|v| {
            v.as_object().map(|obj| {
                obj.values()
                    .filter_map(|v| match v {
                        serde_json::Value::String(s) => Some(s.clone()),
                        serde_json::Value::Array(arr) => {
                            arr.first().and_then(|s| s.as_str().map(String::from))
                        }
                        _ => None,
                    })
                    .collect()
            })
        });

        Some(JestConfig { test_match, test_regex, setup_files, setup_files_after_env, transform })
    }

    /// Parse Jest config from a JS/TS file
    fn parse_js_config(&self, config_path: &Path) -> Result<JestConfig, PluginError> {
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
        let mut var_configs: FxHashMap<String, JestConfig> = FxHashMap::default();

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

        Ok(JestConfig::default())
    }

    /// Extract Jest config from a variable declaration
    fn extract_config_from_var_decl(&self, stmt: &Statement) -> Option<(String, JestConfig)> {
        if let Statement::VariableDeclaration(var_decl) = stmt {
            for decl in &var_decl.declarations {
                let name = decl.id.get_identifier_name()?;

                if let Some(Expression::ObjectExpression(obj)) = &decl.init {
                    let config = self.extract_config_from_object(obj);
                    return Some((name.to_string(), config));
                }
            }
        }
        None
    }

    /// Extract Jest config from a statement
    fn extract_config_from_statement(
        &self,
        stmt: &Statement,
        var_configs: &FxHashMap<String, JestConfig>,
    ) -> Option<JestConfig> {
        match stmt {
            // Handle: module.exports = { ... }
            Statement::ExpressionStatement(expr_stmt) => {
                if let Expression::AssignmentExpression(assign) = &expr_stmt.expression {
                    if let Expression::ObjectExpression(obj) = &assign.right {
                        return Some(self.extract_config_from_object(obj));
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
                            return Some(self.extract_config_from_object(obj));
                        }
                        oxc_ast::ast::ExportDefaultDeclarationKind::CallExpression(call) => {
                            // Handle: export default defineConfig({ ... })
                            if let Some(Argument::ObjectExpression(obj)) = call.arguments.first() {
                                return Some(self.extract_config_from_object(obj));
                            }
                        }
                        // Handle: export default config (variable reference)
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

    /// Extract Jest config from an object expression
    fn extract_config_from_object(&self, obj: &oxc_ast::ast::ObjectExpression) -> JestConfig {
        let mut config = JestConfig::default();

        for prop in &obj.properties {
            if let ObjectPropertyKind::ObjectProperty(property) = prop {
                let key_name = match &property.key {
                    PropertyKey::StaticIdentifier(ident) => Some(ident.name.as_str()),
                    PropertyKey::StringLiteral(lit) => Some(lit.value.as_str()),
                    _ => None,
                };

                if let Some(key) = key_name {
                    match key {
                        "testMatch" => {
                            config.test_match = self.extract_string_array(&property.value);
                        }
                        "testRegex" => {
                            config.test_regex = self.extract_string_or_array(&property.value);
                        }
                        "setupFiles" => {
                            config.setup_files = self.extract_string_array(&property.value);
                        }
                        "setupFilesAfterEnv" => {
                            config.setup_files_after_env =
                                self.extract_string_array(&property.value);
                        }
                        "transform" => {
                            config.transform = self.extract_transform_paths(&property.value);
                        }
                        _ => {}
                    }
                }
            }
        }

        config
    }

    /// Extract an array of strings from an expression
    fn extract_string_array(&self, expr: &Expression) -> Option<Vec<String>> {
        if let Expression::ArrayExpression(arr) = expr {
            let strings: Vec<String> = arr
                .elements
                .iter()
                .filter_map(|elem| {
                    elem.as_expression().and_then(|e| self.extract_string_from_expression(e))
                })
                .collect();

            if strings.is_empty() { None } else { Some(strings) }
        } else {
            None
        }
    }

    /// Extract a string or array of strings from an expression
    fn extract_string_or_array(&self, expr: &Expression) -> Option<Vec<String>> {
        if let Some(s) = self.extract_string_from_expression(expr) {
            Some(vec![s])
        } else {
            self.extract_string_array(expr)
        }
    }

    /// Extract transform paths (values from transform object)
    fn extract_transform_paths(&self, expr: &Expression) -> Option<Vec<String>> {
        if let Expression::ObjectExpression(obj) = expr {
            let paths: Vec<String> = obj
                .properties
                .iter()
                .filter_map(|prop| {
                    if let ObjectPropertyKind::ObjectProperty(property) = prop {
                        match &property.value {
                            // String value: "babel-jest"
                            Expression::StringLiteral(lit) => Some(lit.value.to_string()),
                            // Array value: ["babel-jest", { ... }]
                            Expression::ArrayExpression(arr) => arr
                                .elements
                                .first()
                                .and_then(|e| e.as_expression())
                                .and_then(|e| self.extract_string_from_expression(e)),
                            _ => None,
                        }
                    } else {
                        None
                    }
                })
                .collect();

            if paths.is_empty() { None } else { Some(paths) }
        } else {
            None
        }
    }

    /// Extract a string value from an expression
    fn extract_string_from_expression(&self, expr: &Expression) -> Option<String> {
        match expr {
            Expression::StringLiteral(lit) => Some(lit.value.to_string()),
            Expression::TemplateLiteral(tpl) if tpl.expressions.is_empty() => {
                tpl.quasis.first().map(|q| q.value.raw.to_string())
            }
            _ => None,
        }
    }

    /// Convert Jest glob pattern to standard glob pattern
    /// Jest uses micromatch which supports ?(x) for optional, handles this and similar patterns
    fn convert_jest_glob(&self, pattern: &str) -> String {
        // Convert ?(x) optional pattern to {,x} for glob crate
        // e.g., *.[jt]s?(x) -> *.[jt]s{,x}

        // Simple conversion for common Jest patterns
        // ?(x) means "zero or one of x"
        let mut i = 0;
        let chars: Vec<char> = pattern.chars().collect();
        let mut new_result = String::new();

        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '?' && chars[i + 1] == '(' {
                // Find matching closing paren
                let mut depth = 1;
                let mut j = i + 2;
                while j < chars.len() && depth > 0 {
                    if chars[j] == '(' {
                        depth += 1;
                    } else if chars[j] == ')' {
                        depth -= 1;
                    }
                    j += 1;
                }
                // Extract content between parens
                let inner: String = chars[i + 2..j - 1].iter().collect();
                new_result.push_str(&format!("{{,{}}}", inner));
                i = j;
            } else {
                new_result.push(chars[i]);
                i += 1;
            }
        }

        new_result
    }

    /// Expand glob patterns to find matching files
    fn expand_patterns(
        &self,
        patterns: &[String],
        cwd: &Path,
    ) -> Result<Vec<PathBuf>, PluginError> {
        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let mut entries = FxHashSet::default();

        for pattern in patterns {
            let full_pattern = if let Some(stripped) = pattern.strip_prefix('/') {
                // Absolute pattern (relative to project root in Jest)
                cwd.join(stripped)
            } else {
                // Relative pattern - prepend cwd
                cwd.join(pattern)
            };

            let pattern_str = full_pattern.to_string_lossy();
            let glob_pattern = self.convert_jest_glob(&pattern_str);

            for entry in glob(&glob_pattern)? {
                let path = entry?;
                // Validate path is within project directory
                if let Ok(canonical) = path.canonicalize() {
                    if canonical.starts_with(&cwd_canonical) {
                        entries.insert(canonical);
                    }
                }
            }
        }

        Ok(entries.into_iter().collect())
    }

    /// Resolve setup files and transform paths to absolute paths
    fn resolve_paths(&self, paths: &[String], cwd: &Path) -> Vec<PathBuf> {
        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let mut resolved = Vec::new();

        for path in paths {
            // Skip node_modules packages
            if !path.starts_with('.') && !path.starts_with('/') && !path.starts_with('<') {
                continue;
            }

            // Handle <rootDir> prefix
            let normalized = if let Some(stripped) = path.strip_prefix("<rootDir>/") {
                cwd.join(stripped)
            } else if let Some(stripped) = path.strip_prefix("<rootDir>") {
                cwd.join(stripped)
            } else if let Some(stripped) = path.strip_prefix('/') {
                cwd.join(stripped)
            } else {
                cwd.join(path)
            };

            if let Some(resolved_path) = self.resolve_path(&normalized, cwd) {
                if resolved_path.starts_with(&cwd_canonical) {
                    resolved.push(resolved_path);
                }
            }
        }

        resolved
    }

    /// Resolve a path, trying extensions and index files
    fn resolve_path(&self, target: &Path, _cwd: &Path) -> Option<PathBuf> {
        // Try exact path first
        if target.exists() && target.is_file() {
            return target.canonicalize().ok();
        }

        // Try with extensions
        let target_str = target.to_string_lossy();
        for ext in RESOLVE_EXTENSIONS {
            let with_ext = PathBuf::from(format!("{}{}", target_str, ext));
            if with_ext.exists() && with_ext.is_file() {
                return with_ext.canonicalize().ok();
            }
        }

        // Try as directory with index file
        if target.exists() && target.is_dir() {
            for index_file in INDEX_FILES {
                let index_path = target.join(index_file);
                if index_path.exists() && index_path.is_file() {
                    return index_path.canonicalize().ok();
                }
            }
        }

        None
    }
}

impl Default for JestPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for JestPlugin {
    fn name(&self) -> &str {
        "jest"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("jest")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        let mut entries = FxHashSet::default();

        // Find and parse config files
        let config_files = self.find_config_files(cwd);
        let mut config = JestConfig::default();

        for config_path in &config_files {
            // Add config file itself as entry point
            if let Ok(canonical) = config_path.canonicalize() {
                entries.insert(canonical);
            }

            // Parse config
            let parsed_config = if config_path.extension().is_some_and(|ext| ext == "json") {
                self.parse_json_config(config_path).ok()
            } else {
                self.parse_js_config(config_path).ok()
            };

            if let Some(parsed) = parsed_config {
                config = config.merge(parsed);
            }
        }

        // Also check package.json for Jest config
        if let Some(pkg_config) = self.parse_package_json_config(cwd) {
            config = config.merge(pkg_config);
        }

        // Determine test patterns to use
        let test_patterns: Vec<String> = if config.test_match.is_some() {
            config.test_match.unwrap()
        } else if config.test_regex.is_some() {
            // testRegex patterns need different handling - they're regex, not globs
            // For now, skip regex patterns and fall back to defaults
            // A more complete implementation would convert regex to globs
            DEFAULT_TEST_PATTERNS.iter().map(|s| s.to_string()).collect()
        } else {
            DEFAULT_TEST_PATTERNS.iter().map(|s| s.to_string()).collect()
        };

        // Expand test patterns
        if let Ok(test_files) = self.expand_patterns(&test_patterns, cwd) {
            entries.extend(test_files);
        }

        // Resolve setup files
        if let Some(setup_files) = &config.setup_files {
            for path in self.resolve_paths(setup_files, cwd) {
                entries.insert(path);
            }
        }

        if let Some(setup_files_after_env) = &config.setup_files_after_env {
            for path in self.resolve_paths(setup_files_after_env, cwd) {
                entries.insert(path);
            }
        }

        // Resolve transform paths (only local ones)
        if let Some(transform) = &config.transform {
            for path in self.resolve_paths(transform, cwd) {
                entries.insert(path);
            }
        }

        Ok(entries.into_iter().collect())
    }
}

/// Parsed Jest configuration
#[derive(Debug, Clone, Default)]
struct JestConfig {
    test_match: Option<Vec<String>>,
    test_regex: Option<Vec<String>>,
    setup_files: Option<Vec<String>>,
    setup_files_after_env: Option<Vec<String>>,
    transform: Option<Vec<String>>,
}

impl JestConfig {
    /// Merge another config into this one (other takes precedence)
    fn merge(self, other: JestConfig) -> JestConfig {
        JestConfig {
            test_match: other.test_match.or(self.test_match),
            test_regex: other.test_regex.or(self.test_regex),
            setup_files: merge_option_vec(self.setup_files, other.setup_files),
            setup_files_after_env: merge_option_vec(
                self.setup_files_after_env,
                other.setup_files_after_env,
            ),
            transform: merge_option_vec(self.transform, other.transform),
        }
    }
}

/// Merge two optional vectors
fn merge_option_vec(a: Option<Vec<String>>, b: Option<Vec<String>>) -> Option<Vec<String>> {
    match (a, b) {
        (Some(mut a), Some(b)) => {
            a.extend(b);
            Some(a)
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_with_jest() {
        let plugin = JestPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("jest".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_jest() {
        let plugin = JestPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("mocha".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_find_jest_config_js() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
module.exports = {
  testMatch: ['**/__tests__/**/*.ts'],
};
"#;
        fs::write(temp.path().join("jest.config.js"), config_content).unwrap();

        let config_files = plugin.find_config_files(temp.path());
        assert_eq!(config_files.len(), 1);
        assert!(config_files[0].ends_with("jest.config.js"));
    }

    #[test]
    fn test_find_jest_config_json() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"{ "testMatch": ["**/__tests__/**/*.ts"] }"#;
        fs::write(temp.path().join("jest.config.json"), config_content).unwrap();

        let config_files = plugin.find_config_files(temp.path());
        assert_eq!(config_files.len(), 1);
        assert!(config_files[0].ends_with("jest.config.json"));
    }

    #[test]
    fn test_parse_json_config() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"{
  "testMatch": ["**/__tests__/**/*.ts", "**/*.spec.ts"],
  "setupFiles": ["./setup.js"],
  "setupFilesAfterEnv": ["./setupAfterEnv.js"],
  "transform": {
    "^.+\\.tsx?$": "ts-jest"
  }
}"#;
        let config_path = temp.path().join("jest.config.json");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_json_config(&config_path).unwrap();
        assert_eq!(
            config.test_match,
            Some(vec!["**/__tests__/**/*.ts".to_string(), "**/*.spec.ts".to_string()])
        );
        assert_eq!(config.setup_files, Some(vec!["./setup.js".to_string()]));
        assert_eq!(config.setup_files_after_env, Some(vec!["./setupAfterEnv.js".to_string()]));
        assert_eq!(config.transform, Some(vec!["ts-jest".to_string()]));
    }

    #[test]
    fn test_parse_js_config_module_exports() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
module.exports = {
  testMatch: ['**/__tests__/**/*.ts'],
  setupFiles: ['./jest.setup.js'],
};
"#;
        let config_path = temp.path().join("jest.config.js");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_js_config(&config_path).unwrap();
        assert_eq!(config.test_match, Some(vec!["**/__tests__/**/*.ts".to_string()]));
        assert_eq!(config.setup_files, Some(vec!["./jest.setup.js".to_string()]));
    }

    #[test]
    fn test_parse_js_config_export_default() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
export default {
  testMatch: ['**/tests/**/*.test.ts'],
  setupFilesAfterEnv: ['<rootDir>/setup.ts'],
};
"#;
        let config_path = temp.path().join("jest.config.ts");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_js_config(&config_path).unwrap();
        assert_eq!(config.test_match, Some(vec!["**/tests/**/*.test.ts".to_string()]));
        assert_eq!(config.setup_files_after_env, Some(vec!["<rootDir>/setup.ts".to_string()]));
    }

    #[test]
    fn test_parse_js_config_variable_reference() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
const config = {
  testMatch: ['**/*.spec.ts'],
  transform: {
    '^.+\\.tsx?$': './custom-transformer.js',
  },
};

export default config;
"#;
        let config_path = temp.path().join("jest.config.ts");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_js_config(&config_path).unwrap();
        assert_eq!(config.test_match, Some(vec!["**/*.spec.ts".to_string()]));
        assert_eq!(config.transform, Some(vec!["./custom-transformer.js".to_string()]));
    }

    #[test]
    fn test_parse_package_json_config() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        let package_json = r#"{
  "name": "test-project",
  "jest": {
    "testMatch": ["**/*.test.js"],
    "setupFiles": ["./polyfills.js"]
  }
}"#;
        fs::write(temp.path().join("package.json"), package_json).unwrap();

        let config = plugin.parse_package_json_config(temp.path()).unwrap();
        assert_eq!(config.test_match, Some(vec!["**/*.test.js".to_string()]));
        assert_eq!(config.setup_files, Some(vec!["./polyfills.js".to_string()]));
    }

    #[test]
    fn test_detect_entries_with_test_files() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        // Create jest.config.js
        let config_content = r#"
module.exports = {
  testMatch: ['**/__tests__/**/*.ts'],
};
"#;
        fs::write(temp.path().join("jest.config.js"), config_content).unwrap();

        // Create test files
        let tests_dir = temp.path().join("__tests__");
        fs::create_dir(&tests_dir).unwrap();
        fs::write(tests_dir.join("app.test.ts"), "test('works', () => {})").unwrap();
        fs::write(tests_dir.join("utils.test.ts"), "test('works', () => {})").unwrap();

        // Create non-test file
        let src_dir = temp.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("app.ts"), "export const foo = 1").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();

        // Should include config + test files
        assert!(entries.len() >= 3);
        let paths: Vec<_> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("jest.config.js")));
        assert!(paths.iter().any(|p| p.ends_with("app.test.ts")));
        assert!(paths.iter().any(|p| p.ends_with("utils.test.ts")));
        // Should not include non-test files
        assert!(!paths.iter().any(|p| p.ends_with("app.ts")));
    }

    #[test]
    fn test_detect_entries_with_setup_files() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        // Create setup file
        fs::write(temp.path().join("jest.setup.js"), "global.foo = 'bar'").unwrap();

        // Create jest.config.js
        let config_content = r#"
module.exports = {
  setupFilesAfterEnv: ['./jest.setup.js'],
};
"#;
        fs::write(temp.path().join("jest.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();

        let paths: Vec<_> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("jest.setup.js")));
    }

    #[test]
    fn test_detect_entries_with_root_dir_prefix() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        // Create setup file
        let setup_dir = temp.path().join("config");
        fs::create_dir(&setup_dir).unwrap();
        fs::write(setup_dir.join("setup.ts"), "// setup").unwrap();

        // Create jest.config.js with <rootDir>
        let config_content = r#"
module.exports = {
  setupFilesAfterEnv: ['<rootDir>/config/setup.ts'],
};
"#;
        fs::write(temp.path().join("jest.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();

        let paths: Vec<_> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("setup.ts")));
    }

    #[test]
    fn test_detect_entries_with_local_transform() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        // Create custom transformer
        fs::write(temp.path().join("custom-transformer.js"), "module.exports = {}").unwrap();

        // Create jest.config.js
        let config_content = r#"
module.exports = {
  transform: {
    '^.+\\.tsx?$': './custom-transformer.js',
    '^.+\\.jsx?$': 'babel-jest',
  },
};
"#;
        fs::write(temp.path().join("jest.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();

        let paths: Vec<_> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
        // Should include local transformer
        assert!(paths.iter().any(|p| p.ends_with("custom-transformer.js")));
        // Should not include npm package
        assert!(!paths.iter().any(|p| p.contains("babel-jest")));
    }

    #[test]
    fn test_detect_entries_default_patterns() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        // No jest.config.js, use default patterns

        // Create test files matching default patterns
        let tests_dir = temp.path().join("__tests__");
        fs::create_dir(&tests_dir).unwrap();
        fs::write(tests_dir.join("app.test.js"), "test('works', () => {})").unwrap();

        let src_dir = temp.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("utils.spec.ts"), "test('works', () => {})").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();

        let paths: Vec<_> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("app.test.js")));
        assert!(paths.iter().any(|p| p.ends_with("utils.spec.ts")));
    }

    #[test]
    fn test_convert_jest_glob() {
        let plugin = JestPlugin::new();

        // Test ?(x) conversion
        assert_eq!(plugin.convert_jest_glob("*.[jt]s?(x)"), "*.[jt]s{,x}");

        // Test multiple ?(x) patterns
        assert_eq!(
            plugin.convert_jest_glob("**/?(*.)+(spec|test).[jt]s?(x)"),
            "**/{,*.}+(spec|test).[jt]s{,x}"
        );

        // No change for patterns without ?(x)
        assert_eq!(plugin.convert_jest_glob("**/__tests__/**/*.ts"), "**/__tests__/**/*.ts");
    }

    #[test]
    fn test_transform_with_array_config() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        // Create custom transformer
        fs::write(temp.path().join("svg-transformer.js"), "module.exports = {}").unwrap();

        let config_content = r#"{
  "transform": {
    "^.+\\.svg$": ["./svg-transformer.js", { "exportType": "named" }]
  }
}"#;
        let config_path = temp.path().join("jest.config.json");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_json_config(&config_path).unwrap();
        assert_eq!(config.transform, Some(vec!["./svg-transformer.js".to_string()]));
    }

    #[test]
    fn test_test_regex_string() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"{
  "testRegex": "(/__tests__/.*|(\\.|/)(test|spec))\\.[jt]sx?$"
}"#;
        let config_path = temp.path().join("jest.config.json");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_json_config(&config_path).unwrap();
        assert!(config.test_regex.is_some());
        assert_eq!(config.test_regex.unwrap().len(), 1);
    }

    #[test]
    fn test_test_regex_array() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"{
  "testRegex": ["/__tests__/.*\\.ts$", "\\.spec\\.ts$"]
}"#;
        let config_path = temp.path().join("jest.config.json");
        fs::write(&config_path, config_content).unwrap();

        let config = plugin.parse_json_config(&config_path).unwrap();
        assert!(config.test_regex.is_some());
        assert_eq!(config.test_regex.unwrap().len(), 2);
    }

    #[test]
    fn test_config_file_included_as_entry() {
        let plugin = JestPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"module.exports = {};"#;
        fs::write(temp.path().join("jest.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();

        let paths: Vec<_> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("jest.config.js")));
    }
}
