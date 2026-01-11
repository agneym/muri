use super::{Compiler, CompilerError, CompilerOutput};
use crate::parser::{ImportInfo, ImportKind};
use regex::Regex;
use rustc_hash::FxHashSet;
use std::path::Path;
use std::sync::OnceLock;

/// SCSS/Sass compiler that extracts @use, @import, and @forward statements
pub struct ScssCompiler {
    // Regex is compiled once and cached
}

impl ScssCompiler {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ScssCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the compiled regex for SCSS imports (compiled once, cached)
fn scss_import_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        // Matches @use, @import, and @forward with quoted strings
        // Examples:
        //   @use './variables';
        //   @use "sass:math";
        //   @import 'mixins';
        //   @forward './helpers' as helper-*;
        Regex::new(r#"@(?:use|import|forward)\s+['"]([^'"]+)['"]"#).unwrap()
    })
}

impl Compiler for ScssCompiler {
    fn extensions(&self) -> &[&str] {
        &[".scss", ".sass"]
    }

    fn should_enable(&self, deps: &FxHashSet<String>) -> bool {
        deps.contains("sass") || deps.contains("sass-embedded") || deps.contains("node-sass")
    }

    fn compile(&self, content: &str, _file_path: &Path) -> Result<CompilerOutput, CompilerError> {
        let regex = scss_import_regex();
        let mut imports = Vec::new();

        for cap in regex.captures_iter(content) {
            let source = &cap[1];

            // Skip built-in sass modules (sass:math, sass:color, etc.)
            if source.starts_with("sass:") {
                continue;
            }

            imports.push(ImportInfo { source: source.to_string(), kind: ImportKind::Static });
        }

        Ok(CompilerOutput { imports })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scss_use_imports() {
        let compiler = ScssCompiler::new();
        let content = r#"
@use './variables';
@use "./mixins";
@use 'partials/buttons';
"#;

        let output = compiler.compile(content, Path::new("test.scss")).unwrap();

        assert_eq!(output.imports.len(), 3);
        assert_eq!(output.imports[0].source, "./variables");
        assert_eq!(output.imports[1].source, "./mixins");
        assert_eq!(output.imports[2].source, "partials/buttons");
    }

    #[test]
    fn test_scss_import_statements() {
        let compiler = ScssCompiler::new();
        let content = r#"
@import './base';
@import "utilities";
"#;

        let output = compiler.compile(content, Path::new("test.scss")).unwrap();

        assert_eq!(output.imports.len(), 2);
        assert_eq!(output.imports[0].source, "./base");
        assert_eq!(output.imports[1].source, "utilities");
    }

    #[test]
    fn test_scss_forward_statements() {
        let compiler = ScssCompiler::new();
        let content = r#"
@forward './helpers' as helper-*;
@forward "functions";
"#;

        let output = compiler.compile(content, Path::new("test.scss")).unwrap();

        assert_eq!(output.imports.len(), 2);
        assert_eq!(output.imports[0].source, "./helpers");
        assert_eq!(output.imports[1].source, "functions");
    }

    #[test]
    fn test_scss_skips_builtin_modules() {
        let compiler = ScssCompiler::new();
        let content = r#"
@use 'sass:math';
@use "sass:color";
@use './variables';
"#;

        let output = compiler.compile(content, Path::new("test.scss")).unwrap();

        assert_eq!(output.imports.len(), 1);
        assert_eq!(output.imports[0].source, "./variables");
    }

    #[test]
    fn test_scss_should_enable() {
        let compiler = ScssCompiler::new();

        let mut deps = FxHashSet::default();
        assert!(!compiler.should_enable(&deps));

        deps.insert("sass".to_string());
        assert!(compiler.should_enable(&deps));

        deps.clear();
        deps.insert("sass-embedded".to_string());
        assert!(compiler.should_enable(&deps));

        deps.clear();
        deps.insert("node-sass".to_string());
        assert!(compiler.should_enable(&deps));
    }

    #[test]
    fn test_scss_empty_content() {
        let compiler = ScssCompiler::new();
        let output = compiler.compile("", Path::new("test.scss")).unwrap();
        assert!(output.imports.is_empty());
    }

    #[test]
    fn test_scss_css_only() {
        let compiler = ScssCompiler::new();
        let content = r#"
.button {
    color: red;
}
"#;
        let output = compiler.compile(content, Path::new("test.scss")).unwrap();
        assert!(output.imports.is_empty());
    }
}
