use super::{Plugin, PluginEntries, PluginError};
use regex::Regex;
use rustc_hash::FxHashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Plugin to discover entry points from Husky git hooks.
///
/// Husky hooks are shell scripts that often invoke JS/TS files directly
/// (e.g., `node scripts/lint.js`, `npx ts-node scripts/check.ts`).
/// This plugin parses those shell scripts to find referenced JS/TS files.
pub struct HuskyPlugin;

impl HuskyPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Get the path to the .husky directory
    fn husky_dir(&self, cwd: &Path) -> PathBuf {
        cwd.join(".husky")
    }

    /// Check if a file is a shell script (husky hook files typically have no extension)
    fn is_hook_file(&self, path: &Path) -> bool {
        // Skip directories
        if !path.is_file() {
            return false;
        }

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => return false,
        };

        // Skip hidden files (like .gitignore) and special husky files
        if file_name.starts_with('.') || file_name == "_" {
            return false;
        }

        // Common husky hook names (files without extensions in .husky/)
        let hook_names = [
            "applypatch-msg",
            "commit-msg",
            "fsmonitor-watchman",
            "post-applypatch",
            "post-checkout",
            "post-commit",
            "post-merge",
            "post-receive",
            "post-rewrite",
            "post-update",
            "pre-applypatch",
            "pre-auto-gc",
            "pre-commit",
            "pre-merge-commit",
            "pre-push",
            "pre-rebase",
            "pre-receive",
            "prepare-commit-msg",
            "push-to-checkout",
            "sendemail-validate",
            "update",
        ];

        // Accept known hook names or any file without an extension
        hook_names.contains(&file_name) || !file_name.contains('.')
    }

    /// Find all hook files in the .husky directory
    fn find_hook_files(&self, cwd: &Path) -> Vec<PathBuf> {
        let husky_dir = self.husky_dir(cwd);

        if !husky_dir.exists() || !husky_dir.is_dir() {
            return Vec::new();
        }

        let mut hook_files = Vec::new();

        if let Ok(entries) = fs::read_dir(&husky_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if self.is_hook_file(&path) {
                    hook_files.push(path);
                }
            }
        }

        hook_files
    }

    /// Parse a shell script and extract referenced JS/TS file paths
    fn extract_js_files_from_script(&self, script_path: &Path, cwd: &Path) -> Vec<PathBuf> {
        let content = match fs::read_to_string(script_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let mut found_files = FxHashSet::default();

        // Patterns to match JS/TS file references in shell scripts
        static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
        let patterns = PATTERNS.get_or_init(|| {
            vec![
                // node script.js, node ./script.js, node ../scripts/lint.js
                Regex::new(r#"(?:^|\s)node\s+(?:--[^\s]+\s+)*["']?([^\s"']+\.(?:js|mjs|cjs))["']?"#)
                    .unwrap(),
                // npx ts-node script.ts, npx ts-node ./scripts/check.ts
                Regex::new(
                    r#"(?:^|\s)npx\s+ts-node\s+(?:--[^\s]+\s+)*["']?([^\s"']+\.(?:ts|mts|cts))["']?"#,
                )
                .unwrap(),
                // npx tsx script.ts
                Regex::new(
                    r#"(?:^|\s)npx\s+tsx\s+(?:--[^\s]+\s+)*["']?([^\s"']+\.(?:ts|tsx|mts|cts|js|jsx|mjs|cjs))["']?"#,
                )
                .unwrap(),
                // tsx script.ts (when tsx is installed globally or via npx)
                Regex::new(
                    r#"(?:^|\s)tsx\s+(?:--[^\s]+\s+)*["']?([^\s"']+\.(?:ts|tsx|mts|cts|js|jsx|mjs|cjs))["']?"#,
                )
                .unwrap(),
                // ts-node script.ts (when ts-node is in PATH)
                Regex::new(
                    r#"(?:^|\s)ts-node\s+(?:--[^\s]+\s+)*["']?([^\s"']+\.(?:ts|mts|cts))["']?"#,
                )
                .unwrap(),
                // bun run script.ts, bun script.ts
                Regex::new(
                    r#"(?:^|\s)bun\s+(?:run\s+)?(?:--[^\s]+\s+)*["']?([^\s"']+\.(?:ts|tsx|js|jsx|mts|cts|mjs|cjs))["']?"#,
                )
                .unwrap(),
                // deno run script.ts
                Regex::new(
                    r#"(?:^|\s)deno\s+run\s+(?:--[^\s]+\s+)*["']?([^\s"']+\.(?:ts|tsx|js|jsx|mts|cts|mjs|cjs))["']?"#,
                )
                .unwrap(),
                // ./node_modules/.bin/ts-node script.ts
                Regex::new(
                    r#"(?:^|\s)\./node_modules/\.bin/ts-node\s+(?:--[^\s]+\s+)*["']?([^\s"']+\.(?:ts|mts|cts))["']?"#,
                )
                .unwrap(),
                // Generic: require('./script.js') or import('./script.ts') in shell heredocs
                Regex::new(r#"require\s*\(\s*["']([^"']+\.(?:js|mjs|cjs|ts|mts|cts))["']\s*\)"#)
                    .unwrap(),
            ]
        });

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            for pattern in patterns {
                for cap in pattern.captures_iter(line) {
                    if let Some(file_match) = cap.get(1) {
                        let file_path = file_match.as_str();

                        // Resolve the path relative to cwd
                        let resolved = if file_path.starts_with('/') {
                            PathBuf::from(file_path)
                        } else {
                            cwd.join(file_path)
                        };

                        // Validate and canonicalize
                        if resolved.exists() {
                            if let Ok(canonical) = resolved.canonicalize() {
                                // Security check: ensure path is within project directory
                                if canonical.starts_with(&cwd_canonical) {
                                    found_files.insert(canonical);
                                }
                            }
                        }
                    }
                }
            }
        }

        found_files.into_iter().collect()
    }
}

impl Default for HuskyPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for HuskyPlugin {
    fn name(&self) -> &str {
        "husky"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("husky")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<PluginEntries, PluginError> {
        let hook_files = self.find_hook_files(cwd);
        let mut entries = FxHashSet::default();

        for hook_file in hook_files {
            let js_files = self.extract_js_files_from_script(&hook_file, cwd);
            entries.extend(js_files);
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
    fn test_plugin_name() {
        let plugin = HuskyPlugin::new();
        assert_eq!(plugin.name(), "husky");
    }

    #[test]
    fn test_default_impl() {
        let _: HuskyPlugin = Default::default();
    }

    #[test]
    fn test_should_enable_with_husky() {
        let plugin = HuskyPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("husky".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_husky() {
        let plugin = HuskyPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("lint-staged".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_with_empty_deps() {
        let plugin = HuskyPlugin::new();
        let deps = FxHashSet::default();

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_no_husky_dir_returns_empty() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_empty_husky_dir_returns_empty() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create empty .husky directory
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_find_hook_files() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create .husky directory with hooks
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        fs::write(husky_dir.join("pre-commit"), "#!/bin/sh\necho test").unwrap();
        fs::write(husky_dir.join("pre-push"), "#!/bin/sh\necho test").unwrap();
        fs::write(husky_dir.join("commit-msg"), "#!/bin/sh\necho test").unwrap();
        // Create a file that should be skipped
        fs::write(husky_dir.join(".gitignore"), "*").unwrap();
        fs::write(husky_dir.join("_"), "#!/bin/sh").unwrap();

        let hook_files = plugin.find_hook_files(temp.path());
        assert_eq!(hook_files.len(), 3);

        let names: Vec<_> =
            hook_files.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
        assert!(names.contains(&"pre-commit"));
        assert!(names.contains(&"pre-push"));
        assert!(names.contains(&"commit-msg"));
    }

    #[test]
    fn test_extract_node_script() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create scripts directory with a JS file
        let scripts_dir = temp.path().join("scripts");
        fs::create_dir(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("lint.js"), "console.log('lint')").unwrap();

        // Create .husky directory with pre-commit hook
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let hook_content = r#"#!/bin/sh
. "$(dirname "$0")/_/husky.sh"

node scripts/lint.js
"#;
        let hook_path = husky_dir.join("pre-commit");
        fs::write(&hook_path, hook_content).unwrap();

        let entries = plugin.extract_js_files_from_script(&hook_path, temp.path());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("lint.js"));
    }

    #[test]
    fn test_extract_node_script_with_relative_path() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create scripts directory with a JS file
        let scripts_dir = temp.path().join("scripts");
        fs::create_dir(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("check.js"), "console.log('check')").unwrap();

        // Create .husky directory with pre-commit hook
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let hook_content = r#"#!/bin/sh
node ./scripts/check.js
"#;
        let hook_path = husky_dir.join("pre-commit");
        fs::write(&hook_path, hook_content).unwrap();

        let entries = plugin.extract_js_files_from_script(&hook_path, temp.path());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("check.js"));
    }

    #[test]
    fn test_extract_npx_ts_node_script() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create scripts directory with a TS file
        let scripts_dir = temp.path().join("scripts");
        fs::create_dir(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("validate.ts"), "console.log('validate')").unwrap();

        // Create .husky directory with pre-commit hook
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let hook_content = r#"#!/bin/sh
npx ts-node scripts/validate.ts
"#;
        let hook_path = husky_dir.join("pre-commit");
        fs::write(&hook_path, hook_content).unwrap();

        let entries = plugin.extract_js_files_from_script(&hook_path, temp.path());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("validate.ts"));
    }

    #[test]
    fn test_extract_tsx_script() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create scripts directory with a TS file
        let scripts_dir = temp.path().join("scripts");
        fs::create_dir(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("build.ts"), "console.log('build')").unwrap();

        // Create .husky directory with pre-push hook
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let hook_content = r#"#!/bin/sh
npx tsx scripts/build.ts
"#;
        let hook_path = husky_dir.join("pre-push");
        fs::write(&hook_path, hook_content).unwrap();

        let entries = plugin.extract_js_files_from_script(&hook_path, temp.path());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("build.ts"));
    }

    #[test]
    fn test_extract_bun_script() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create scripts directory with a TS file
        let scripts_dir = temp.path().join("scripts");
        fs::create_dir(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("test.ts"), "console.log('test')").unwrap();

        // Create .husky directory with pre-commit hook
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let hook_content = r#"#!/bin/sh
bun run scripts/test.ts
"#;
        let hook_path = husky_dir.join("pre-commit");
        fs::write(&hook_path, hook_content).unwrap();

        let entries = plugin.extract_js_files_from_script(&hook_path, temp.path());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("test.ts"));
    }

    #[test]
    fn test_extract_multiple_scripts() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create scripts directory with multiple files
        let scripts_dir = temp.path().join("scripts");
        fs::create_dir(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("lint.js"), "console.log('lint')").unwrap();
        fs::write(scripts_dir.join("test.ts"), "console.log('test')").unwrap();
        fs::write(scripts_dir.join("format.mjs"), "console.log('format')").unwrap();

        // Create .husky directory with pre-commit hook
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let hook_content = r#"#!/bin/sh
node scripts/lint.js
npx ts-node scripts/test.ts
node scripts/format.mjs
"#;
        let hook_path = husky_dir.join("pre-commit");
        fs::write(&hook_path, hook_content).unwrap();

        let entries = plugin.extract_js_files_from_script(&hook_path, temp.path());
        assert_eq!(entries.len(), 3);

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
        assert!(filenames.contains(&"lint.js"));
        assert!(filenames.contains(&"test.ts"));
        assert!(filenames.contains(&"format.mjs"));
    }

    #[test]
    fn test_detect_entries_from_multiple_hooks() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create scripts directory with files
        let scripts_dir = temp.path().join("scripts");
        fs::create_dir(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("pre-commit-check.js"), "// check").unwrap();
        fs::write(scripts_dir.join("pre-push-check.ts"), "// check").unwrap();

        // Create .husky directory with multiple hooks
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        fs::write(husky_dir.join("pre-commit"), "#!/bin/sh\nnode scripts/pre-commit-check.js")
            .unwrap();
        fs::write(husky_dir.join("pre-push"), "#!/bin/sh\nnpx ts-node scripts/pre-push-check.ts")
            .unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        let paths = entries.get_paths();
        assert_eq!(paths.len(), 2);

        let filenames: Vec<_> =
            paths.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
        assert!(filenames.contains(&"pre-commit-check.js"));
        assert!(filenames.contains(&"pre-push-check.ts"));
    }

    #[test]
    fn test_nonexistent_script_ignored() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create .husky directory with pre-commit hook referencing non-existent file
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let hook_content = r#"#!/bin/sh
node scripts/nonexistent.js
"#;
        let hook_path = husky_dir.join("pre-commit");
        fs::write(&hook_path, hook_content).unwrap();

        let entries = plugin.extract_js_files_from_script(&hook_path, temp.path());
        assert!(entries.is_empty());
    }

    #[test]
    fn test_comments_ignored() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create scripts directory with a JS file
        let scripts_dir = temp.path().join("scripts");
        fs::create_dir(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("lint.js"), "console.log('lint')").unwrap();

        // Create .husky directory with pre-commit hook
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let hook_content = r#"#!/bin/sh
# This is a comment: node scripts/should-be-ignored.js
# node scripts/also-ignored.js
node scripts/lint.js
"#;
        let hook_path = husky_dir.join("pre-commit");
        fs::write(&hook_path, hook_content).unwrap();

        let entries = plugin.extract_js_files_from_script(&hook_path, temp.path());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("lint.js"));
    }

    #[test]
    fn test_node_with_flags() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create scripts directory with a JS file
        let scripts_dir = temp.path().join("scripts");
        fs::create_dir(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("lint.js"), "console.log('lint')").unwrap();

        // Create .husky directory with pre-commit hook
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let hook_content = r#"#!/bin/sh
node --experimental-modules --no-warnings scripts/lint.js
"#;
        let hook_path = husky_dir.join("pre-commit");
        fs::write(&hook_path, hook_content).unwrap();

        let entries = plugin.extract_js_files_from_script(&hook_path, temp.path());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("lint.js"));
    }

    #[test]
    fn test_quoted_paths() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create scripts directory with a JS file
        let scripts_dir = temp.path().join("scripts");
        fs::create_dir(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("lint.js"), "console.log('lint')").unwrap();

        // Create .husky directory with pre-commit hook
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let hook_content = r#"#!/bin/sh
node "scripts/lint.js"
"#;
        let hook_path = husky_dir.join("pre-commit");
        fs::write(&hook_path, hook_content).unwrap();

        let entries = plugin.extract_js_files_from_script(&hook_path, temp.path());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("lint.js"));
    }

    #[test]
    fn test_is_hook_file() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create test files
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        // These should be recognized as hooks
        fs::write(husky_dir.join("pre-commit"), "#!/bin/sh").unwrap();
        fs::write(husky_dir.join("pre-push"), "#!/bin/sh").unwrap();
        fs::write(husky_dir.join("commit-msg"), "#!/bin/sh").unwrap();
        fs::write(husky_dir.join("custom-hook"), "#!/bin/sh").unwrap();

        // These should NOT be recognized as hooks
        fs::write(husky_dir.join(".gitignore"), "*").unwrap();
        fs::write(husky_dir.join("_"), "#!/bin/sh").unwrap();
        fs::write(husky_dir.join("README.md"), "# Husky").unwrap();

        assert!(plugin.is_hook_file(&husky_dir.join("pre-commit")));
        assert!(plugin.is_hook_file(&husky_dir.join("pre-push")));
        assert!(plugin.is_hook_file(&husky_dir.join("commit-msg")));
        assert!(plugin.is_hook_file(&husky_dir.join("custom-hook")));

        assert!(!plugin.is_hook_file(&husky_dir.join(".gitignore")));
        assert!(!plugin.is_hook_file(&husky_dir.join("_")));
        assert!(!plugin.is_hook_file(&husky_dir.join("README.md")));
    }

    #[test]
    fn test_deno_run_script() {
        let plugin = HuskyPlugin::new();
        let temp = tempdir().unwrap();

        // Create scripts directory with a TS file
        let scripts_dir = temp.path().join("scripts");
        fs::create_dir(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("check.ts"), "console.log('check')").unwrap();

        // Create .husky directory with pre-commit hook
        let husky_dir = temp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();

        let hook_content = r#"#!/bin/sh
deno run --allow-read scripts/check.ts
"#;
        let hook_path = husky_dir.join("pre-commit");
        fs::write(&hook_path, hook_content).unwrap();

        let entries = plugin.extract_js_files_from_script(&hook_path, temp.path());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("check.ts"));
    }
}
