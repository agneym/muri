use super::{Plugin, PluginError};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, Expression, ModuleDeclaration, ObjectPropertyKind, PropertyKey, Statement,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use rustc_hash::FxHashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Extensions to try when resolving module paths
const RESOLVE_EXTENSIONS: &[&str] = &[".js", ".ts", ".mjs", ".cjs"];

/// Index file names to try for directory imports
const INDEX_FILES: &[&str] = &["index.js", "index.ts", "index.mjs", "index.cjs"];

/// Plugin to discover Tailwind CSS config files and their local dependencies as entry points
pub struct TailwindPlugin;

impl TailwindPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find Tailwind config files in the project
    fn find_config_files(&self, cwd: &Path) -> Vec<PathBuf> {
        let extensions = ["js", "ts", "mjs", "cjs"];
        let directories = [cwd.to_path_buf(), cwd.join("config")];
        let mut found = Vec::new();

        for dir in &directories {
            for ext in &extensions {
                let path = dir.join(format!("tailwind.config.{}", ext));
                if path.exists() {
                    found.push(path);
                }
            }
        }

        found
    }

    /// Parse a config file and extract local require()/import paths
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

        let mut paths = FxHashSet::default();

        // Extract import declarations (ESM)
        for stmt in &parsed.program.body {
            if let Some(ModuleDeclaration::ImportDeclaration(import)) = stmt.as_module_declaration()
            {
                let source = import.source.value.as_str();
                if is_local_path(source) {
                    paths.insert(source.to_string());
                }
            }
        }

        // Extract require() calls from statements
        for stmt in &parsed.program.body {
            self.extract_requires_from_statement(stmt, &mut paths);
        }

        Ok(paths.into_iter().collect())
    }

    /// Extract require() calls from a statement
    fn extract_requires_from_statement(&self, stmt: &Statement, paths: &mut FxHashSet<String>) {
        match stmt {
            // Handle: const x = require('./file')
            Statement::VariableDeclaration(var_decl) => {
                for decl in &var_decl.declarations {
                    if let Some(init) = &decl.init {
                        self.extract_requires_from_expression(init, paths);
                    }
                }
            }
            // Handle: module.exports = { ... }
            Statement::ExpressionStatement(expr_stmt) => {
                self.extract_requires_from_expression(&expr_stmt.expression, paths);
            }
            _ => {
                // Handle: export default { ... }
                if let Some(ModuleDeclaration::ExportDefaultDeclaration(export)) =
                    stmt.as_module_declaration()
                {
                    match &export.declaration {
                        oxc_ast::ast::ExportDefaultDeclarationKind::ObjectExpression(obj) => {
                            self.extract_requires_from_object(obj, paths);
                        }
                        oxc_ast::ast::ExportDefaultDeclarationKind::CallExpression(call) => {
                            // Handle: export default defineConfig({ ... })
                            for arg in &call.arguments {
                                if let Argument::ObjectExpression(obj) = arg {
                                    self.extract_requires_from_object(obj, paths);
                                }
                            }
                            // Also check callee and arguments for require calls
                            self.extract_requires_from_call(call, paths);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Extract require() calls from an expression
    fn extract_requires_from_expression(&self, expr: &Expression, paths: &mut FxHashSet<String>) {
        match expr {
            Expression::CallExpression(call) => {
                self.extract_requires_from_call(call, paths);
            }
            Expression::ObjectExpression(obj) => {
                self.extract_requires_from_object(obj, paths);
            }
            Expression::ArrayExpression(arr) => {
                for elem in &arr.elements {
                    if let Some(elem_expr) = elem.as_expression() {
                        self.extract_requires_from_expression(elem_expr, paths);
                    }
                }
            }
            Expression::AssignmentExpression(assign) => {
                self.extract_requires_from_expression(&assign.right, paths);
            }
            Expression::ConditionalExpression(cond) => {
                self.extract_requires_from_expression(&cond.consequent, paths);
                self.extract_requires_from_expression(&cond.alternate, paths);
            }
            _ => {}
        }
    }

    /// Extract require() from a call expression
    fn extract_requires_from_call(
        &self,
        call: &oxc_ast::ast::CallExpression,
        paths: &mut FxHashSet<String>,
    ) {
        // Check if this is a require() call
        if let Expression::Identifier(ident) = &call.callee {
            if ident.name == "require" {
                if let Some(Argument::StringLiteral(lit)) = call.arguments.first() {
                    let path = lit.value.as_str();
                    if is_local_path(path) {
                        paths.insert(path.to_string());
                    }
                }
            }
        }

        // Also recurse into arguments (e.g., require(...).default or nested calls)
        for arg in &call.arguments {
            match arg {
                Argument::ObjectExpression(obj) => {
                    self.extract_requires_from_object(obj, paths);
                }
                Argument::ArrayExpression(arr) => {
                    for elem in &arr.elements {
                        if let Some(elem_expr) = elem.as_expression() {
                            self.extract_requires_from_expression(elem_expr, paths);
                        }
                    }
                }
                _ => {
                    if let Some(expr) = arg.as_expression() {
                        self.extract_requires_from_expression(expr, paths);
                    }
                }
            }
        }
    }

    /// Extract require() calls from an object expression
    fn extract_requires_from_object(
        &self,
        obj: &oxc_ast::ast::ObjectExpression,
        paths: &mut FxHashSet<String>,
    ) {
        for prop in &obj.properties {
            match prop {
                ObjectPropertyKind::ObjectProperty(property) => {
                    // Check property key names to find plugins, presets, theme
                    let key_name = match &property.key {
                        PropertyKey::StaticIdentifier(ident) => Some(ident.name.as_str()),
                        PropertyKey::StringLiteral(lit) => Some(lit.value.as_str()),
                        _ => None,
                    };

                    // Extract from all properties, with special attention to plugins/presets/theme
                    let is_important_key = matches!(
                        key_name,
                        Some("plugins") | Some("presets") | Some("theme") | Some("extend")
                    );

                    if is_important_key || key_name.is_some() {
                        self.extract_requires_from_expression(&property.value, paths);
                    }
                }
                ObjectPropertyKind::SpreadProperty(spread) => {
                    self.extract_requires_from_expression(&spread.argument, paths);
                }
            }
        }
    }

    /// Resolve a relative path to an absolute path, trying extensions and index files
    fn resolve_path(&self, base_dir: &Path, relative_path: &str) -> Option<PathBuf> {
        let target = base_dir.join(relative_path);

        // Try exact path first
        if target.exists() && target.is_file() {
            return target.canonicalize().ok();
        }

        // Try with extensions
        for ext in RESOLVE_EXTENSIONS {
            let with_ext = base_dir.join(format!("{}{}", relative_path, ext));
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

impl Default for TailwindPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a path is a local relative path (starts with ./ or ../)
fn is_local_path(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../")
}

impl Plugin for TailwindPlugin {
    fn name(&self) -> &str {
        "tailwind"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("tailwindcss")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        let mut entries = Vec::new();

        for config_path in self.find_config_files(cwd) {
            // Add the config file itself as an entry point
            if let Ok(canonical) = config_path.canonicalize() {
                entries.push(canonical);
            }

            // Parse and resolve local dependencies
            let config_dir = config_path.parent().unwrap_or(cwd);
            if let Ok(paths) = self.parse_config(&config_path) {
                for path in paths {
                    if let Some(resolved) = self.resolve_path(config_dir, &path) {
                        entries.push(resolved);
                    }
                }
            }
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_with_tailwindcss() {
        let plugin = TailwindPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("tailwindcss".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_tailwindcss() {
        let plugin = TailwindPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("react".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_config_file_as_entry_point() {
        let plugin = TailwindPlugin::new();
        let temp = tempdir().unwrap();

        // Create tailwind.config.js
        let config_content = r#"
module.exports = {
  content: ['./src/**/*.{js,ts,jsx,tsx}'],
};
"#;
        fs::write(temp.path().join("tailwind.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("tailwind.config.js"));
    }

    #[test]
    fn test_extract_plugin_requires() {
        let plugin = TailwindPlugin::new();
        let temp = tempdir().unwrap();

        // Create a custom plugin file
        let plugin_dir = temp.path().join("plugins");
        fs::create_dir(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("custom.js"), "module.exports = {}").unwrap();

        // Create tailwind.config.js with plugin requires
        let config_content = r#"
module.exports = {
  content: ['./src/**/*.{js,ts,jsx,tsx}'],
  plugins: [
    require('./plugins/custom'),
    require('@tailwindcss/forms'),
  ],
};
"#;
        fs::write(temp.path().join("tailwind.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        // Should include config file and local plugin (not npm package)
        assert_eq!(entries.len(), 2);
        let paths: Vec<_> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("tailwind.config.js")));
        assert!(paths.iter().any(|p| p.ends_with("custom.js")));
    }

    #[test]
    fn test_extract_preset_requires() {
        let plugin = TailwindPlugin::new();
        let temp = tempdir().unwrap();

        // Create a preset file
        let presets_dir = temp.path().join("presets");
        fs::create_dir(&presets_dir).unwrap();
        fs::write(presets_dir.join("base.js"), "module.exports = {}").unwrap();

        // Create tailwind.config.js with preset requires
        let config_content = r#"
module.exports = {
  presets: [
    require('./presets/base'),
  ],
  content: ['./src/**/*.tsx'],
};
"#;
        fs::write(temp.path().join("tailwind.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);
        let paths: Vec<_> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("base.js")));
    }

    #[test]
    fn test_extract_top_level_theme_requires() {
        let plugin = TailwindPlugin::new();
        let temp = tempdir().unwrap();

        // Create a theme file
        fs::write(temp.path().join("theme.js"), "module.exports = { colors: {} }").unwrap();

        // Create tailwind.config.js with theme require
        let config_content = r#"
const theme = require('./theme');

module.exports = {
  content: ['./src/**/*.tsx'],
  theme: theme,
};
"#;
        fs::write(temp.path().join("tailwind.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);
        let paths: Vec<_> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("theme.js")));
    }

    #[test]
    fn test_ignore_npm_packages() {
        let plugin = TailwindPlugin::new();
        let temp = tempdir().unwrap();

        // Create tailwind.config.js with only npm package requires
        let config_content = r#"
module.exports = {
  content: ['./src/**/*.tsx'],
  plugins: [
    require('@tailwindcss/forms'),
    require('@tailwindcss/typography'),
  ],
};
"#;
        fs::write(temp.path().join("tailwind.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        // Should only include the config file, not npm packages
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("tailwind.config.js"));
    }

    #[test]
    fn test_esm_import_syntax() {
        let plugin = TailwindPlugin::new();
        let temp = tempdir().unwrap();

        // Create a theme file
        fs::write(temp.path().join("theme.mjs"), "export default { colors: {} }").unwrap();

        // Create ESM tailwind.config.mjs
        let config_content = r#"
import theme from './theme.mjs';

export default {
  content: ['./src/**/*.tsx'],
  theme: theme,
};
"#;
        fs::write(temp.path().join("tailwind.config.mjs"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);
        let paths: Vec<_> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("tailwind.config.mjs")));
        assert!(paths.iter().any(|p| p.ends_with("theme.mjs")));
    }

    #[test]
    fn test_directory_imports_resolve_to_index() {
        let plugin = TailwindPlugin::new();
        let temp = tempdir().unwrap();

        // Create a directory with index.js
        let plugins_dir = temp.path().join("plugins");
        fs::create_dir(&plugins_dir).unwrap();
        fs::write(plugins_dir.join("index.js"), "module.exports = {}").unwrap();

        // Create tailwind.config.js with directory import
        let config_content = r#"
module.exports = {
  content: ['./src/**/*.tsx'],
  plugins: [
    require('./plugins'),
  ],
};
"#;
        fs::write(temp.path().join("tailwind.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);
        let paths: Vec<_> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("index.js")));
    }

    #[test]
    fn test_config_in_config_subdirectory() {
        let plugin = TailwindPlugin::new();
        let temp = tempdir().unwrap();

        // Create config subdirectory with tailwind.config.js
        let config_dir = temp.path().join("config");
        fs::create_dir(&config_dir).unwrap();

        let config_content = r#"
module.exports = {
  content: ['./src/**/*.tsx'],
};
"#;
        fs::write(config_dir.join("tailwind.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].to_string_lossy().contains("config/tailwind.config.js"));
    }

    #[test]
    fn test_parse_config_extracts_local_paths() {
        let plugin = TailwindPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
const colors = require('./colors');
const spacing = require('../shared/spacing');

module.exports = {
  theme: {
    colors: colors,
    spacing: spacing,
  },
  plugins: [
    require('./plugins/custom'),
    require('@tailwindcss/forms'),
  ],
};
"#;
        let config_path = temp.path().join("tailwind.config.js");
        fs::write(&config_path, config_content).unwrap();

        let paths = plugin.parse_config(&config_path).unwrap();
        // Should extract local paths but not npm packages
        assert!(paths.contains(&"./colors".to_string()));
        assert!(paths.contains(&"../shared/spacing".to_string()));
        assert!(paths.contains(&"./plugins/custom".to_string()));
        assert!(!paths.iter().any(|p| p.contains("@tailwindcss")));
    }
}
