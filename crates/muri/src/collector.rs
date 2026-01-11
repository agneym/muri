use glob::Pattern;
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct Collector {
    cwd: PathBuf,
    ignore_patterns: Vec<Pattern>,
    include_node_modules: bool,
}

/// Expand brace patterns like `**/*.{ts,tsx}` into multiple patterns
fn expand_brace_pattern(pattern: &str) -> Vec<String> {
    // Find brace group
    if let Some(start) = pattern.find('{') {
        if let Some(end) = pattern[start..].find('}') {
            let end = start + end;
            let prefix = &pattern[..start];
            let suffix = &pattern[end + 1..];
            let alternatives = &pattern[start + 1..end];

            return alternatives
                .split(',')
                .flat_map(|alt| {
                    let expanded = format!("{}{}{}", prefix, alt, suffix);
                    expand_brace_pattern(&expanded)
                })
                .collect();
        }
    }
    vec![pattern.to_string()]
}

impl Collector {
    pub fn new(cwd: &Path, ignore_patterns: &[String], include_node_modules: bool) -> Self {
        let patterns = ignore_patterns
            .iter()
            .flat_map(|p| expand_brace_pattern(p))
            .filter_map(|p| Pattern::new(&p).ok())
            .collect();

        Self {
            cwd: cwd.to_path_buf(),
            ignore_patterns: patterns,
            include_node_modules,
        }
    }

    fn should_ignore(&self, path: &Path) -> bool {
        let relative = path.strip_prefix(&self.cwd).unwrap_or(path);
        let path_str = relative.to_string_lossy();

        // Check ignore patterns
        for pattern in &self.ignore_patterns {
            if pattern.matches(&path_str) {
                return true;
            }
        }

        // Check node_modules
        if !self.include_node_modules {
            for component in relative.components() {
                if component.as_os_str() == "node_modules" {
                    return true;
                }
            }
        }

        false
    }

    fn matches_glob(&self, path: &Path, patterns: &[String]) -> bool {
        let relative = path.strip_prefix(&self.cwd).unwrap_or(path);
        let path_str = relative.to_string_lossy();

        // Expand brace patterns and check
        for pattern_str in patterns {
            for expanded in expand_brace_pattern(pattern_str) {
                if let Ok(pattern) = Pattern::new(&expanded) {
                    if pattern.matches(&path_str) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn collect_files(&self, patterns: &[String]) -> HashSet<PathBuf> {
        let mut files = HashSet::new();

        let walker = WalkBuilder::new(&self.cwd)
            .hidden(false)
            .git_ignore(true)
            .build();

        for entry in walker.flatten() {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            if self.should_ignore(path) {
                continue;
            }

            if self.matches_glob(path, patterns) {
                if let Ok(canonical) = path.canonicalize() {
                    files.insert(canonical);
                }
            }
        }

        files
    }

    pub fn collect_entry_files(&self, patterns: &[String]) -> HashSet<PathBuf> {
        self.collect_files(patterns)
    }

    pub fn collect_project_files(&self, patterns: &[String]) -> HashSet<PathBuf> {
        self.collect_files(patterns)
    }
}
