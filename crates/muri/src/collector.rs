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

/// Precompiled glob matchers for efficient file matching
struct CompiledMatchers {
    entry: GlobSet,
    project: GlobSet,
    ignore: GlobSet,
}

impl CompiledMatchers {
    fn new(
        entry_patterns: &[String],
        project_patterns: &[String],
        ignore_patterns: &[String],
    ) -> Self {
        Self {
            entry: compile_globset(entry_patterns),
            project: compile_globset(project_patterns),
            ignore: compile_globset(ignore_patterns),
        }
    }
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
    ) -> Self {
        Self {
            cwd: cwd.to_path_buf(),
            matchers: CompiledMatchers::new(entry_patterns, project_patterns, ignore_patterns),
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

            // Check if file matches project patterns
            let is_project = self.matchers.project.is_match(&*relative_str);

            // Check if file matches entry patterns
            let is_entry = self.matchers.entry.is_match(&*relative_str);

            if is_project {
                project_files.insert(canonical.clone());
            }

            if is_entry {
                entry_files.insert(canonical);
            }
        }

        ProjectIndex { entry_files, project_files }
    }
}
