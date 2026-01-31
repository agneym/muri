use crate::plugin::EntryPattern;
use crate::types::DEFAULT_EXTENSIONS;
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

/// Result of a single filesystem walk that collects both entry and project files
pub struct ProjectIndex {
    pub entry_files: FxHashSet<PathBuf>,
    pub project_files: FxHashSet<PathBuf>,
}

/// A compiled plugin pattern with its resolved base directory
struct CompiledPluginPattern {
    matcher: GlobSet,
    base: PathBuf,
}

/// Precompiled glob matchers for efficient file matching
struct CompiledMatchers {
    entry: GlobSet,
    project: GlobSet,
    ignore: GlobSet,
    plugin_patterns: Vec<CompiledPluginPattern>,
}

impl CompiledMatchers {
    fn new(
        entry_patterns: &[String],
        project_patterns: &[String],
        ignore_patterns: &[String],
        plugin_patterns: &[EntryPattern],
        cwd: &Path,
    ) -> Self {
        // Compile plugin patterns, grouping by base directory
        let mut compiled_plugins = Vec::new();
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

            let matcher = compile_globset(std::slice::from_ref(&pattern.pattern));
            compiled_plugins.push(CompiledPluginPattern { matcher, base: canonical_base });
        }

        Self {
            entry: compile_globset(entry_patterns),
            project: compile_globset(project_patterns),
            ignore: compile_globset(ignore_patterns),
            plugin_patterns: compiled_plugins,
        }
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

/// Expand brace patterns like `**/*.{ts,tsx}` into multiple patterns
fn expand_brace_pattern(pattern: &str) -> Vec<String> {
    if let Some(start) = pattern.find('{') {
        if let Some(end) = pattern[start..].find('}') {
            let end = start + end;
            let prefix = &pattern[..start];
            let suffix = &pattern[end + 1..];
            let alternatives = &pattern[start + 1..end];

            return alternatives
                .split(',')
                .flat_map(|alt| {
                    let expanded = format!("{prefix}{alt}{suffix}");
                    expand_brace_pattern(&expanded)
                })
                .collect();
        }
    }
    vec![pattern.to_string()]
}

/// Compile a list of glob patterns into a GlobSet for efficient matching
fn compile_globset(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        for expanded in expand_brace_pattern(pattern) {
            if let Ok(glob) = Glob::new(&expanded) {
                builder.add(glob);
            }
        }
    }
    builder.build().unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap())
}

pub struct Collector {
    cwd: PathBuf,
    matchers: CompiledMatchers,
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
            matchers: CompiledMatchers::new(
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

            // Check ignore patterns (precompiled)
            if self.matchers.ignore.is_match(&*relative_str) {
                continue;
            }

            // Canonicalize once for both checks
            let canonical = match path.canonicalize() {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Check if file matches project patterns AND has a parseable extension
            // This filters out foreign files (images, fonts, etc.) from project_files
            // while still allowing them to be resolved when imported
            let is_project =
                self.matchers.project.is_match(&*relative_str) && has_parseable_extension(path);

            // Check if file matches entry patterns
            let is_entry = self.matchers.entry.is_match(&*relative_str);

            // Check if file matches any plugin patterns
            let is_plugin_entry = self.check_plugin_patterns(&canonical);

            if is_project {
                project_files.insert(canonical.clone());
            }

            if is_entry || is_plugin_entry {
                entry_files.insert(canonical);
            }
        }

        ProjectIndex { entry_files, project_files }
    }

    /// Check if a file matches any plugin pattern
    fn check_plugin_patterns(&self, canonical_path: &Path) -> bool {
        for compiled in &self.matchers.plugin_patterns {
            // Check if path is under this pattern's base
            if let Ok(relative) = canonical_path.strip_prefix(&compiled.base) {
                let relative_str = relative.to_string_lossy();
                if compiled.matcher.is_match(&*relative_str) {
                    return true;
                }
            }
        }
        false
    }
}
