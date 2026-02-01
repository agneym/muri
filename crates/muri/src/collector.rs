use crate::plugin::EntryPattern;
use crate::types::DEFAULT_EXTENSIONS;
use fast_glob::glob_match;
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

/// Result of a single filesystem walk that collects both entry and project files
pub struct ProjectIndex {
    pub entry_files: FxHashSet<PathBuf>,
    pub project_files: FxHashSet<PathBuf>,
    pub ignored_files: FxHashSet<PathBuf>,
}

/// A plugin pattern with its resolved base directory
struct PluginPattern {
    pattern: String,
    base: PathBuf,
}

/// Stores glob patterns for matching
struct Matchers {
    entry: Vec<String>,
    project: Vec<String>,
    ignore: Vec<String>,
    plugin_patterns: Vec<PluginPattern>,
}

impl Matchers {
    fn new(
        entry_patterns: &[String],
        project_patterns: &[String],
        ignore_patterns: &[String],
        plugin_patterns: &[EntryPattern],
        cwd: &Path,
    ) -> Self {
        // Resolve plugin pattern base directories
        let mut resolved_plugins = Vec::new();
        for pattern in plugin_patterns {
            let base = match &pattern.base {
                Some(b) => cwd.join(b),
                None => cwd.to_path_buf(),
            };

            // Skip if base doesn't exist
            let canonical_base = match base.canonicalize() {
                Ok(p) => p,
                Err(_) => continue,
            };

            resolved_plugins
                .push(PluginPattern { pattern: pattern.pattern.clone(), base: canonical_base });
        }

        Self {
            entry: entry_patterns.to_vec(),
            project: project_patterns.to_vec(),
            ignore: ignore_patterns.to_vec(),
            plugin_patterns: resolved_plugins,
        }
    }

    /// Check if path matches any pattern in the list
    fn matches_any(patterns: &[String], path: &str) -> bool {
        patterns.iter().any(|p| glob_match(p, path))
    }
}

/// Check if a file has a parseable extension (JS/TS only)
fn has_parseable_extension(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => format!(".{e}"),
        None => return false,
    };

    DEFAULT_EXTENSIONS.iter().any(|&default_ext| default_ext == ext)
}

pub struct Collector {
    cwd: PathBuf,
    matchers: Matchers,
}

impl Collector {
    pub fn new(
        cwd: &Path,
        entry_patterns: &[String],
        project_patterns: &[String],
        ignore_patterns: &[String],
        plugin_patterns: &[EntryPattern],
    ) -> Self {
        Self {
            cwd: cwd.to_path_buf(),
            matchers: Matchers::new(
                entry_patterns,
                project_patterns,
                ignore_patterns,
                plugin_patterns,
                cwd,
            ),
        }
    }

    /// Collect all files in a single walk, categorizing them as entry/project files
    pub fn collect(&self) -> ProjectIndex {
        let mut entry_files = FxHashSet::default();
        let mut project_files = FxHashSet::default();
        let mut ignored_files = FxHashSet::default();

        let mut walker_builder = WalkBuilder::new(&self.cwd);
        walker_builder.hidden(false).git_ignore(true);

        // Always exclude node_modules directories during traversal
        let mut overrides = OverrideBuilder::new(&self.cwd);
        overrides.add("!**/node_modules/").ok();
        if let Ok(built) = overrides.build() {
            walker_builder.overrides(built);
        }

        let walker = walker_builder.build();

        for entry in walker.flatten() {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let relative = path.strip_prefix(&self.cwd).unwrap_or(path);
            let relative_str = relative.to_string_lossy();

            // Check ignore patterns - track separately instead of skipping
            let is_ignored = Matchers::matches_any(&self.matchers.ignore, &relative_str);

            // Canonicalize once for all checks
            let canonical = match path.canonicalize() {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Check if file matches project patterns AND has a parseable extension
            // This filters out foreign files (images, fonts, etc.) from project_files
            // while still allowing them to be resolved when imported
            let is_project = Matchers::matches_any(&self.matchers.project, &relative_str)
                && has_parseable_extension(path);

            // Check if file matches entry patterns
            let is_entry = Matchers::matches_any(&self.matchers.entry, &relative_str);

            // Check if file matches any plugin patterns
            let is_plugin_entry = self.check_plugin_patterns(&canonical);

            if is_project {
                if is_ignored {
                    ignored_files.insert(canonical.clone());
                } else {
                    project_files.insert(canonical.clone());
                }
            }

            // Entry files should NOT come from ignored patterns
            if !is_ignored && (is_entry || is_plugin_entry) {
                entry_files.insert(canonical);
            }
        }

        ProjectIndex { entry_files, project_files, ignored_files }
    }

    /// Check if a file matches any plugin pattern
    fn check_plugin_patterns(&self, canonical_path: &Path) -> bool {
        for plugin in &self.matchers.plugin_patterns {
            // Check if path is under this pattern's base
            if let Ok(relative) = canonical_path.strip_prefix(&plugin.base) {
                let relative_str = relative.to_string_lossy();
                if glob_match(&plugin.pattern, &*relative_str) {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nested_brace_patterns() {
        // This is the pattern Storybook uses - nested braces must work correctly
        let pattern = "**/*.{mdx,stories.{tsx,ts,jsx,js}}";
        let patterns = vec![pattern.to_string()];

        // All these should match
        assert!(Matchers::matches_any(&patterns, "components/Button.stories.tsx"));
        assert!(Matchers::matches_any(&patterns, "components/Button.stories.ts"));
        assert!(Matchers::matches_any(&patterns, "components/Button.stories.jsx"));
        assert!(Matchers::matches_any(&patterns, "components/Button.stories.js"));
        assert!(Matchers::matches_any(&patterns, "components/Button.mdx"));
        assert!(Matchers::matches_any(&patterns, "deep/nested/path/Component.stories.jsx"));

        // These should NOT match
        assert!(!Matchers::matches_any(&patterns, "components/Button.tsx"));
        assert!(!Matchers::matches_any(&patterns, "components/Button.ts"));
        assert!(!Matchers::matches_any(&patterns, "components/Button.stories.css"));
    }

    #[test]
    fn test_simple_brace_patterns() {
        let pattern = "**/*.{ts,tsx}";
        let patterns = vec![pattern.to_string()];

        assert!(Matchers::matches_any(&patterns, "src/index.ts"));
        assert!(Matchers::matches_any(&patterns, "src/App.tsx"));
        assert!(!Matchers::matches_any(&patterns, "src/index.js"));
    }

    #[test]
    fn test_deeply_nested_brace_patterns() {
        // Three levels of nesting
        let pattern = "**/*.{a,b.{c,d.{e,f}}}";
        let patterns = vec![pattern.to_string()];

        assert!(Matchers::matches_any(&patterns, "file.a"));
        assert!(Matchers::matches_any(&patterns, "file.b.c"));
        assert!(Matchers::matches_any(&patterns, "file.b.d.e"));
        assert!(Matchers::matches_any(&patterns, "file.b.d.f"));
        assert!(!Matchers::matches_any(&patterns, "file.b.d.g"));
    }
}
