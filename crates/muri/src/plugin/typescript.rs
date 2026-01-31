use super::{Plugin, PluginEntries, PluginError};
use fast_glob::glob_match;
use rustc_hash::FxHashSet;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Plugin to discover TypeScript configuration files and their dependencies as entry points.
///
/// This plugin detects:
/// - tsconfig.json and tsconfig.*.json files (e.g., tsconfig.build.json)
/// - Files explicitly listed in the `files` array
/// - Extended base config files from the `extends` field
pub struct TypescriptPlugin;

impl TypescriptPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find tsconfig files in the project root.
    /// Looks for tsconfig.json and tsconfig.*.json patterns.
    fn find_config_files(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        let mut found = FxHashSet::default();

        // Check for tsconfig.json
        let tsconfig_path = cwd.join("tsconfig.json");
        if tsconfig_path.exists() && tsconfig_path.is_file() {
            if let Ok(canonical) = tsconfig_path.canonicalize() {
                found.insert(canonical);
            }
        }

        // Find tsconfig.*.json files (e.g., tsconfig.build.json, tsconfig.test.json)
        // Use fast-glob pattern matching on directory entries
        let pattern = "tsconfig.*.json";
        if let Ok(read_dir) = std::fs::read_dir(cwd) {
            for entry in read_dir.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_name) = path.file_name() {
                        let name = file_name.to_string_lossy();
                        if glob_match(pattern, name.as_ref()) {
                            if let Ok(canonical) = path.canonicalize() {
                                found.insert(canonical);
                            }
                        }
                    }
                }
            }
        }

        Ok(found.into_iter().collect())
    }

    /// Parse a tsconfig file and extract entry points.
    /// Returns (files_from_files_array, extended_config_paths).
    fn parse_config(
        &self,
        config_path: &Path,
    ) -> Result<(Vec<PathBuf>, Vec<PathBuf>), PluginError> {
        let content = fs::read_to_string(config_path)?;

        // Strip comments from JSON (tsconfig allows comments)
        let clean_content = strip_json_comments(&content);

        let json: Value = serde_json::from_str(&clean_content).map_err(|e| {
            PluginError::ConfigParse(format!("Failed to parse {}: {}", config_path.display(), e))
        })?;

        let config_dir = config_path.parent().unwrap_or(Path::new("."));
        let mut files = Vec::new();
        let mut extends = Vec::new();

        // Extract files from "files" array
        if let Some(files_array) = json.get("files").and_then(|v| v.as_array()) {
            for file_value in files_array {
                if let Some(file_str) = file_value.as_str() {
                    let file_path = config_dir.join(file_str);
                    if file_path.exists() && file_path.is_file() {
                        if let Ok(canonical) = file_path.canonicalize() {
                            files.push(canonical);
                        }
                    }
                }
            }
        }

        // Extract extended config paths from "extends"
        if let Some(extends_value) = json.get("extends") {
            let extends_paths = match extends_value {
                Value::String(s) => vec![s.as_str()],
                Value::Array(arr) => arr.iter().filter_map(|v| v.as_str()).collect(),
                _ => vec![],
            };

            for extend_path in extends_paths {
                if let Some(resolved) = self.resolve_extends(config_dir, extend_path) {
                    extends.push(resolved);
                }
            }
        }

        Ok((files, extends))
    }

    /// Resolve an extends path to an absolute path.
    /// Handles relative paths like "./base.json" or "../shared/tsconfig.json".
    fn resolve_extends(&self, config_dir: &Path, extend_path: &str) -> Option<PathBuf> {
        // Only handle local relative paths, not npm packages
        if !extend_path.starts_with("./") && !extend_path.starts_with("../") {
            return None;
        }

        let target = config_dir.join(extend_path);

        // Try exact path first
        if target.exists() && target.is_file() {
            return target.canonicalize().ok();
        }

        // Try with .json extension if not present
        if !extend_path.ends_with(".json") {
            let with_ext = config_dir.join(format!("{}.json", extend_path));
            if with_ext.exists() && with_ext.is_file() {
                return with_ext.canonicalize().ok();
            }
        }

        None
    }
}

impl Default for TypescriptPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Strip single-line (//) and multi-line (/* */) comments from JSON content.
/// This is needed because tsconfig.json allows comments (JSONC format).
fn strip_json_comments(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if in_string {
            result.push(c);
            continue;
        }

        // Check for comments outside strings
        if c == '/' {
            if let Some(&next) = chars.peek() {
                if next == '/' {
                    // Single-line comment: skip until end of line
                    chars.next(); // consume the second '/'
                    while let Some(&nc) = chars.peek() {
                        if nc == '\n' {
                            break;
                        }
                        chars.next();
                    }
                    continue;
                } else if next == '*' {
                    // Multi-line comment: skip until */
                    chars.next(); // consume the '*'
                    while let Some(nc) = chars.next() {
                        if nc == '*' {
                            if let Some(&'/') = chars.peek() {
                                chars.next(); // consume the '/'
                                break;
                            }
                        }
                    }
                    continue;
                }
            }
        }

        result.push(c);
    }

    result
}

impl Plugin for TypescriptPlugin {
    fn name(&self) -> &str {
        "typescript"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("typescript")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<PluginEntries, PluginError> {
        let mut entries = FxHashSet::default();
        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());

        // Find all tsconfig files
        let config_files = self.find_config_files(cwd)?;

        for config_path in &config_files {
            // Add the tsconfig file itself as an entry point
            entries.insert(config_path.clone());

            // Parse the config to extract files and extends
            if let Ok((files, extends)) = self.parse_config(config_path) {
                // Add files from the "files" array
                for file in files {
                    // Validate path is within project directory
                    if file.starts_with(&cwd_canonical) {
                        entries.insert(file);
                    }
                }

                // Add extended config files
                for extend in extends {
                    // Validate path is within project directory
                    if extend.starts_with(&cwd_canonical) {
                        entries.insert(extend);
                    }
                }
            }
        }

        Ok(PluginEntries::paths(entries.into_iter().collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_with_typescript() {
        let plugin = TypescriptPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("typescript".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_typescript() {
        let plugin = TypescriptPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("react".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_find_tsconfig_json() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"{
  "compilerOptions": {
    "target": "ES2020"
  }
}"#;
        fs::write(temp.path().join("tsconfig.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("tsconfig.json"));
    }

    #[test]
    fn test_find_tsconfig_build_json() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"{
  "compilerOptions": {
    "target": "ES2020"
  }
}"#;
        fs::write(temp.path().join("tsconfig.build.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("tsconfig.build.json"));
    }

    #[test]
    fn test_find_multiple_tsconfig_files() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"{ "compilerOptions": {} }"#;
        fs::write(temp.path().join("tsconfig.json"), config_content).unwrap();
        fs::write(temp.path().join("tsconfig.build.json"), config_content).unwrap();
        fs::write(temp.path().join("tsconfig.test.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 3);

        let filenames: Vec<_> =
            paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"tsconfig.json".to_string()));
        assert!(filenames.contains(&"tsconfig.build.json".to_string()));
        assert!(filenames.contains(&"tsconfig.test.json".to_string()));
    }

    #[test]
    fn test_extract_files_array() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        // Create source files
        let src_dir = temp.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("index.ts"), "export const foo = 1;").unwrap();
        fs::write(src_dir.join("utils.ts"), "export const bar = 2;").unwrap();

        // Create tsconfig with files array
        let config_content = r#"{
  "compilerOptions": {
    "target": "ES2020"
  },
  "files": [
    "src/index.ts",
    "src/utils.ts"
  ]
}"#;
        fs::write(temp.path().join("tsconfig.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 3); // tsconfig.json + 2 files

        let filenames: Vec<_> =
            paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"tsconfig.json".to_string()));
        assert!(filenames.contains(&"index.ts".to_string()));
        assert!(filenames.contains(&"utils.ts".to_string()));
    }

    #[test]
    fn test_extract_extends_relative_path() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        // Create base config
        let base_content = r#"{
  "compilerOptions": {
    "strict": true
  }
}"#;
        fs::write(temp.path().join("tsconfig.base.json"), base_content).unwrap();

        // Create main config that extends base
        let config_content = r#"{
  "extends": "./tsconfig.base.json",
  "compilerOptions": {
    "target": "ES2020"
  }
}"#;
        fs::write(temp.path().join("tsconfig.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 2);

        let filenames: Vec<_> =
            paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"tsconfig.json".to_string()));
        assert!(filenames.contains(&"tsconfig.base.json".to_string()));
    }

    #[test]
    fn test_extract_extends_without_json_extension() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        // Create base config
        let base_content = r#"{ "compilerOptions": {} }"#;
        fs::write(temp.path().join("tsconfig.base.json"), base_content).unwrap();

        // Create main config that extends base without .json extension
        let config_content = r#"{
  "extends": "./tsconfig.base"
}"#;
        fs::write(temp.path().join("tsconfig.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 2);

        let filenames: Vec<_> =
            paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"tsconfig.base.json".to_string()));
    }

    #[test]
    fn test_extract_extends_array() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        // Create base configs
        fs::write(temp.path().join("tsconfig.base.json"), r#"{ "compilerOptions": {} }"#).unwrap();
        fs::write(temp.path().join("tsconfig.strict.json"), r#"{ "compilerOptions": {} }"#)
            .unwrap();

        // Create main config that extends multiple bases (TypeScript 5.0+ feature)
        let config_content = r#"{
  "extends": ["./tsconfig.base.json", "./tsconfig.strict.json"]
}"#;
        fs::write(temp.path().join("tsconfig.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 3);

        let filenames: Vec<_> =
            paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"tsconfig.json".to_string()));
        assert!(filenames.contains(&"tsconfig.base.json".to_string()));
        assert!(filenames.contains(&"tsconfig.strict.json".to_string()));
    }

    #[test]
    fn test_extends_in_subdirectory() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        // Create config in subdirectory
        let config_dir = temp.path().join("config");
        fs::create_dir(&config_dir).unwrap();
        fs::write(config_dir.join("base.json"), r#"{ "compilerOptions": {} }"#).unwrap();

        // Create main config
        let config_content = r#"{
  "extends": "./config/base.json"
}"#;
        fs::write(temp.path().join("tsconfig.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let entry_paths = entries.get_paths();
        assert_eq!(entry_paths.len(), 2);

        let paths: Vec<_> = entry_paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("tsconfig.json")));
        assert!(paths.iter().any(|p| p.ends_with("base.json")));
    }

    #[test]
    fn test_ignore_npm_package_extends() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        // Create main config that extends an npm package
        let config_content = r#"{
  "extends": "@tsconfig/node18/tsconfig.json"
}"#;
        fs::write(temp.path().join("tsconfig.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        // Should only include tsconfig.json, not the npm package
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("tsconfig.json"));
    }

    #[test]
    fn test_parse_config_with_comments() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        // Create source file
        fs::write(temp.path().join("index.ts"), "export const x = 1;").unwrap();

        // Create tsconfig with comments (JSONC format)
        let config_content = r#"{
  // This is a single-line comment
  "compilerOptions": {
    "target": "ES2020" /* inline comment */
  },
  /* Multi-line
     comment */
  "files": ["index.ts"]
}"#;
        fs::write(temp.path().join("tsconfig.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 2);

        let filenames: Vec<_> =
            paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"tsconfig.json".to_string()));
        assert!(filenames.contains(&"index.ts".to_string()));
    }

    #[test]
    fn test_no_config_returns_empty() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_files_with_nonexistent_paths_ignored() {
        let plugin = TypescriptPlugin::new();
        let temp = tempdir().unwrap();

        // Create tsconfig referencing non-existent files
        let config_content = r#"{
  "files": [
    "nonexistent.ts",
    "also-missing.ts"
  ]
}"#;
        fs::write(temp.path().join("tsconfig.json"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        // Should only include tsconfig.json, not the missing files
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("tsconfig.json"));
    }

    #[test]
    fn test_strip_json_comments() {
        // Single-line comments
        assert_eq!(strip_json_comments(r#"{ "a": 1 } // comment"#), r#"{ "a": 1 } "#);

        // Multi-line comments
        assert_eq!(strip_json_comments(r#"{ /* comment */ "a": 1 }"#), r#"{  "a": 1 }"#);

        // Preserve strings that look like comments
        assert_eq!(
            strip_json_comments(r#"{ "url": "http://example.com" }"#),
            r#"{ "url": "http://example.com" }"#
        );

        // Handle escape sequences in strings
        assert_eq!(strip_json_comments(r#"{ "path": "C:\\dir" }"#), r#"{ "path": "C:\\dir" }"#);
    }

    #[test]
    fn test_default_impl() {
        let _: TypescriptPlugin = Default::default();
    }
}
