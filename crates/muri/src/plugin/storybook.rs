use super::{Plugin, PluginError};
use globset::GlobBuilder;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, Expression, ModuleDeclaration, ObjectPropertyKind, PropertyKey, Statement,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Convert Storybook's @() glob syntax to standard {} glob syntax
/// Example: "**/*.stories.@(js|jsx|ts|tsx)" -> "**/*.stories.{js,jsx,ts,tsx}"
///
/// Handles nested patterns by processing inside-out:
/// "**/*.@(mdx|stories.@(tsx|ts))" -> "**/*.{mdx,stories.{tsx,ts}}"
fn convert_storybook_glob(pattern: &str) -> String {
    static STORYBOOK_GLOB_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = STORYBOOK_GLOB_REGEX.get_or_init(|| {
        // Match @(...) patterns where content is pipe-separated extensions
        // [^)]+ ensures we match innermost patterns first (no nested parens)
        Regex::new(r"@\(([^)]+)\)").unwrap()
    });

    let mut result = pattern.to_string();
    // Loop to handle nested @() patterns from inside out
    loop {
        let new_result = regex
            .replace_all(&result, |caps: &regex::Captures| {
                // Convert pipe-separated to comma-separated inside braces
                let inner = &caps[1];
                format!("{{{}}}", inner.replace('|', ","))
            })
            .to_string();

        if new_result == result {
            break;
        }
        result = new_result;
    }
    result
}

/// Plugin to discover Storybook story files as entry points
pub struct StorybookPlugin;

impl StorybookPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find the Storybook main config file
    fn find_config_file(&self, cwd: &Path) -> Option<PathBuf> {
        let extensions = ["js", "ts", "mjs", "cjs", "mts", "cts"];
        for ext in extensions {
            let path = cwd.join(".storybook").join(format!("main.{}", ext));
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    /// Parse Storybook config and extract story patterns
    fn parse_config(&self, config_path: &Path) -> Result<Vec<String>, PluginError> {
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

        // First pass: collect variable declarations with stories
        // Maps variable name -> stories patterns
        let mut var_stories: FxHashMap<String, Vec<String>> = FxHashMap::default();

        for stmt in &parsed.program.body {
            if let Some((name, patterns)) = self.extract_stories_from_var_decl(stmt) {
                var_stories.insert(name, patterns);
            }
        }

        // Second pass: look for exports
        for stmt in &parsed.program.body {
            if let Some(patterns) = self.extract_stories_from_statement(stmt, &var_stories) {
                if !patterns.is_empty() {
                    return Ok(patterns);
                }
            }
        }

        Ok(Vec::new())
    }

    /// Extract stories from a variable declaration
    /// Returns (variable_name, stories_patterns) if found
    fn extract_stories_from_var_decl(&self, stmt: &Statement) -> Option<(String, Vec<String>)> {
        if let Statement::VariableDeclaration(var_decl) = stmt {
            for decl in &var_decl.declarations {
                // Get the variable name
                let name = decl.id.get_identifier_name()?;

                // Check if it has an object initializer with stories
                if let Some(Expression::ObjectExpression(obj)) = &decl.init {
                    if let Some(patterns) = self.extract_stories_from_object(obj) {
                        return Some((name.to_string(), patterns));
                    }
                }
            }
        }
        None
    }

    /// Extract stories patterns from a statement
    fn extract_stories_from_statement(
        &self,
        stmt: &Statement,
        var_stories: &FxHashMap<String, Vec<String>>,
    ) -> Option<Vec<String>> {
        match stmt {
            // Handle: module.exports = { stories: [...] }
            Statement::ExpressionStatement(expr_stmt) => {
                if let Expression::AssignmentExpression(assign) = &expr_stmt.expression {
                    if let Expression::ObjectExpression(obj) = &assign.right {
                        return self.extract_stories_from_object(obj);
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
                            return self.extract_stories_from_object(obj);
                        }
                        oxc_ast::ast::ExportDefaultDeclarationKind::CallExpression(call) => {
                            // Handle: export default defineConfig({ stories: [...] })
                            if let Some(Argument::ObjectExpression(obj)) = call.arguments.first() {
                                return self.extract_stories_from_object(obj);
                            }
                        }
                        // Handle: export default config (variable reference)
                        oxc_ast::ast::ExportDefaultDeclarationKind::Identifier(ident) => {
                            if let Some(patterns) = var_stories.get(ident.name.as_str()) {
                                return Some(patterns.clone());
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
        }
    }

    /// Extract stories array from an object expression
    fn extract_stories_from_object(
        &self,
        obj: &oxc_ast::ast::ObjectExpression,
    ) -> Option<Vec<String>> {
        for prop in &obj.properties {
            if let ObjectPropertyKind::ObjectProperty(property) = prop {
                // Check if the key is "stories"
                let is_stories_key = match &property.key {
                    PropertyKey::StaticIdentifier(ident) => ident.name == "stories",
                    PropertyKey::StringLiteral(lit) => lit.value == "stories",
                    _ => false,
                };

                if is_stories_key {
                    return self.extract_patterns_from_expression(&property.value);
                }
            }
        }
        None
    }

    /// Extract string patterns from an expression (array or single string)
    fn extract_patterns_from_expression(&self, expr: &Expression) -> Option<Vec<String>> {
        match expr {
            Expression::ArrayExpression(arr) => {
                let mut patterns = Vec::new();
                for elem in &arr.elements {
                    if let Some(expr) = elem.as_expression() {
                        if let Some(pattern) = self.extract_string_from_expression(expr) {
                            patterns.push(pattern);
                        }
                    }
                }
                if patterns.is_empty() { None } else { Some(patterns) }
            }
            _ => self.extract_string_from_expression(expr).map(|s| vec![s]),
        }
    }

    /// Extract a string value from an expression
    fn extract_string_from_expression(&self, expr: &Expression) -> Option<String> {
        match expr {
            Expression::StringLiteral(lit) => Some(lit.value.to_string()),
            Expression::TemplateLiteral(tpl) if tpl.expressions.is_empty() => {
                // Simple template literal with no expressions
                tpl.quasis.first().map(|q| q.value.raw.to_string())
            }
            Expression::ObjectExpression(obj) => {
                // Handle: { directory: '../src', files: '**/*.stories.tsx' }
                // Look for 'directory' or 'files' property
                let mut directory = None;
                let mut files = None;

                for prop in &obj.properties {
                    if let ObjectPropertyKind::ObjectProperty(property) = prop {
                        let key_name = match &property.key {
                            PropertyKey::StaticIdentifier(ident) => Some(ident.name.as_str()),
                            PropertyKey::StringLiteral(lit) => Some(lit.value.as_str()),
                            _ => None,
                        };

                        if let Some(key) = key_name {
                            if key == "directory" {
                                if let Expression::StringLiteral(lit) = &property.value {
                                    directory = Some(lit.value.to_string());
                                }
                            } else if key == "files" {
                                if let Expression::StringLiteral(lit) = &property.value {
                                    files = Some(lit.value.to_string());
                                }
                            }
                        }
                    }
                }

                // Combine directory and files if both present
                match (directory, files) {
                    (Some(dir), Some(f)) => Some(format!("{}/{}", dir, f)),
                    (Some(dir), None) => {
                        // Default files pattern if only directory is specified
                        Some(format!("{}/**/*.stories.@(js|jsx|ts|tsx)", dir))
                    }
                    (None, Some(f)) => Some(f), // Use files pattern as-is
                    (None, None) => None,
                }
            }
            _ => None,
        }
    }

    /// Expand glob patterns relative to the .storybook directory
    fn expand_patterns(
        &self,
        patterns: &[String],
        cwd: &Path,
    ) -> Result<Vec<PathBuf>, PluginError> {
        let storybook_dir = cwd.join(".storybook");
        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let mut entries = Vec::new();

        for pattern in patterns {
            // Resolve base directory relative to .storybook directory (where main.js is)
            // We need to separate the directory path from the glob pattern because:
            // 1. Directory paths may contain ".." which needs canonicalization
            // 2. Glob patterns contain wildcards that can't be canonicalized
            let full_pattern = if pattern.starts_with("../") || pattern.starts_with("./") {
                storybook_dir.join(pattern)
            } else {
                cwd.join(pattern)
            };

            let pattern_str = full_pattern.to_string_lossy();

            // Convert Storybook's @() syntax to glob {} syntax
            let glob_pattern = convert_storybook_glob(&pattern_str);

            // Find where glob special characters start to split base dir from pattern
            let glob_chars = ['*', '{', '[', '?'];
            let glob_start = glob_pattern.find(|c| glob_chars.contains(&c));

            let (base_dir, glob_suffix) = if let Some(idx) = glob_start {
                // Find the last path separator before the glob starts
                let before_glob = &glob_pattern[..idx];
                let last_sep = before_glob.rfind('/').unwrap_or(0);
                let base = PathBuf::from(&glob_pattern[..last_sep]);
                let suffix = &glob_pattern[last_sep..];
                // Remove leading slash from suffix if present
                let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
                (base, suffix.to_string())
            } else {
                // No glob chars, treat entire path as base
                (PathBuf::from(&*glob_pattern), String::new())
            };

            // Canonicalize the base directory to resolve ".." components
            let canonical_base = match base_dir.canonicalize() {
                Ok(p) => p,
                Err(_) => continue, // Skip if directory doesn't exist
            };

            // Ensure base is within project directory
            if !canonical_base.starts_with(&cwd_canonical) {
                continue;
            }

            if glob_suffix.is_empty() {
                // No glob pattern, just add the directory/file if it exists
                if canonical_base.exists() {
                    entries.push(canonical_base);
                }
                continue;
            }

            // Build glob matcher using globset (which supports brace expansion)
            let glob = match GlobBuilder::new(&glob_suffix).literal_separator(true).build() {
                Ok(g) => g,
                Err(_) => continue,
            };
            let matcher = glob.compile_matcher();

            // Walk directory and match files
            Self::walk_and_match(&canonical_base, &canonical_base, &matcher, &mut entries);
        }

        Ok(entries)
    }

    /// Recursively walk directory and collect files matching the glob pattern
    fn walk_and_match(
        dir: &Path,
        base: &Path,
        matcher: &globset::GlobMatcher,
        entries: &mut Vec<PathBuf>,
    ) {
        let read_dir = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return,
        };

        for entry in read_dir.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                Self::walk_and_match(&path, base, matcher, entries);
            } else if path.is_file() {
                // Match against relative path from base
                if let Ok(relative) = path.strip_prefix(base) {
                    if matcher.is_match(relative) {
                        entries.push(path);
                    }
                }
            }
        }
    }

    /// Default story patterns when config parsing fails
    fn default_patterns() -> &'static [&'static str] {
        &[
            "**/*.stories.ts",
            "**/*.stories.tsx",
            "**/*.stories.js",
            "**/*.stories.jsx",
            "**/*.story.ts",
            "**/*.story.tsx",
            "**/*.story.js",
            "**/*.story.jsx",
        ]
    }
}

impl Default for StorybookPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for StorybookPlugin {
    fn name(&self) -> &str {
        "storybook"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        // Check for any @storybook/* package or the storybook package
        dependencies.iter().any(|d| d.starts_with("@storybook/") || d == "storybook")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        // Try to find and parse config file
        if let Some(config_path) = self.find_config_file(cwd) {
            if let Ok(patterns) = self.parse_config(&config_path) {
                if !patterns.is_empty() {
                    return self.expand_patterns(&patterns, cwd);
                }
            }
        }

        // Fall back to default patterns if config not found or parsing failed
        let default_patterns: Vec<String> =
            Self::default_patterns().iter().map(|s| s.to_string()).collect();
        self.expand_patterns(&default_patterns, cwd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_with_storybook_react() {
        let plugin = StorybookPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("@storybook/react".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_enable_with_storybook_package() {
        let plugin = StorybookPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("storybook".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_storybook() {
        let plugin = StorybookPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("react".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_parse_module_exports_config() {
        let plugin = StorybookPlugin::new();
        let temp = tempdir().unwrap();

        // Create .storybook directory and config
        let storybook_dir = temp.path().join(".storybook");
        fs::create_dir(&storybook_dir).unwrap();

        let config_content = r#"
module.exports = {
  stories: ['../src/**/*.stories.tsx', '../components/**/*.stories.tsx'],
  addons: ['@storybook/addon-essentials'],
};
"#;
        fs::write(storybook_dir.join("main.js"), config_content).unwrap();

        let patterns = plugin.parse_config(&storybook_dir.join("main.js")).unwrap();
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0], "../src/**/*.stories.tsx");
        assert_eq!(patterns[1], "../components/**/*.stories.tsx");
    }

    #[test]
    fn test_parse_export_default_config() {
        let plugin = StorybookPlugin::new();
        let temp = tempdir().unwrap();

        let storybook_dir = temp.path().join(".storybook");
        fs::create_dir(&storybook_dir).unwrap();

        let config_content = r#"
export default {
  stories: ['../src/**/*.stories.tsx'],
};
"#;
        fs::write(storybook_dir.join("main.ts"), config_content).unwrap();

        let patterns = plugin.parse_config(&storybook_dir.join("main.ts")).unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0], "../src/**/*.stories.tsx");
    }

    #[test]
    fn test_parse_object_pattern() {
        let plugin = StorybookPlugin::new();
        let temp = tempdir().unwrap();

        let storybook_dir = temp.path().join(".storybook");
        fs::create_dir(&storybook_dir).unwrap();

        let config_content = r#"
module.exports = {
  stories: [
    { directory: '../src', files: '**/*.stories.tsx' }
  ],
};
"#;
        fs::write(storybook_dir.join("main.js"), config_content).unwrap();

        let patterns = plugin.parse_config(&storybook_dir.join("main.js")).unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0], "../src/**/*.stories.tsx");
    }

    #[test]
    fn test_parse_variable_reference_config() {
        let plugin = StorybookPlugin::new();
        let temp = tempdir().unwrap();

        let storybook_dir = temp.path().join(".storybook");
        fs::create_dir(&storybook_dir).unwrap();

        // This pattern is used by many TypeScript configs:
        // const config: StorybookConfig = { ... }; export default config;
        let config_content = r#"
const config = {
  stories: [
    { directory: '../app/javascript/react', files: '**/*.stories.tsx' }
  ],
};
export default config;
"#;
        fs::write(storybook_dir.join("main.ts"), config_content).unwrap();

        let patterns = plugin.parse_config(&storybook_dir.join("main.ts")).unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0], "../app/javascript/react/**/*.stories.tsx");
    }

    #[test]
    fn test_parse_typed_variable_reference_config() {
        let plugin = StorybookPlugin::new();
        let temp = tempdir().unwrap();

        let storybook_dir = temp.path().join(".storybook");
        fs::create_dir(&storybook_dir).unwrap();

        // TypeScript config with type annotation (like circle's config)
        let config_content = r#"
import type { StorybookConfig } from "@storybook/react-webpack5";

const config: StorybookConfig = {
  stories: [
    {
      directory: "../app/javascript/react",
      files: "**/*.@(mdx|stories.@(tsx|ts|jsx|js))",
    },
  ],
};

export default config;
"#;
        fs::write(storybook_dir.join("main.ts"), config_content).unwrap();

        let patterns = plugin.parse_config(&storybook_dir.join("main.ts")).unwrap();
        assert_eq!(patterns.len(), 1);
        // parse_config returns raw patterns; conversion happens in expand_patterns
        assert_eq!(patterns[0], "../app/javascript/react/**/*.@(mdx|stories.@(tsx|ts|jsx|js))");
    }

    #[test]
    fn test_detect_entries_with_real_files() {
        let plugin = StorybookPlugin::new();
        let temp = tempdir().unwrap();

        // Create .storybook directory and config
        let storybook_dir = temp.path().join(".storybook");
        fs::create_dir(&storybook_dir).unwrap();

        let config_content = r#"
module.exports = {
  stories: ['../src/**/*.stories.tsx'],
};
"#;
        fs::write(storybook_dir.join("main.js"), config_content).unwrap();

        // Create src directory with story files
        let src_dir = temp.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("Button.stories.tsx"), "export default {}").unwrap();
        fs::write(src_dir.join("Card.stories.tsx"), "export default {}").unwrap();
        fs::write(src_dir.join("utils.ts"), "export const foo = 1").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"Button.stories.tsx".to_string()));
        assert!(filenames.contains(&"Card.stories.tsx".to_string()));
    }

    #[test]
    fn test_fallback_to_default_patterns() {
        let plugin = StorybookPlugin::new();
        let temp = tempdir().unwrap();

        // No .storybook directory, should use default patterns
        let src_dir = temp.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("Button.stories.tsx"), "export default {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_convert_storybook_glob() {
        // Basic conversion
        assert_eq!(
            convert_storybook_glob("**/*.stories.@(js|jsx|ts|tsx)"),
            "**/*.stories.{js,jsx,ts,tsx}"
        );

        // Multiple @() patterns
        assert_eq!(
            convert_storybook_glob("**/@(stories|__stories__)/*.@(ts|tsx)"),
            "**/{stories,__stories__}/*.{ts,tsx}"
        );

        // Nested @() patterns (common in Storybook configs)
        assert_eq!(
            convert_storybook_glob("**/*.@(mdx|stories.@(tsx|ts|jsx|js))"),
            "**/*.{mdx,stories.{tsx,ts,jsx,js}}"
        );

        // Deeply nested @() patterns
        assert_eq!(convert_storybook_glob("@(a|@(b|@(c|d)))"), "{a,{b,{c,d}}}");

        // No @() pattern - should remain unchanged
        assert_eq!(convert_storybook_glob("**/*.stories.tsx"), "**/*.stories.tsx");

        // Empty content
        assert_eq!(convert_storybook_glob(""), "");
    }
}
