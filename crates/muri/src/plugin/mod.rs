mod cypress;
mod eslint;
mod husky;
mod jest;
mod lint_staged;
mod nextjs;
mod playwright;
mod postcss;
mod registry;
mod storybook;
mod tailwind;
mod typescript;
mod vite;
mod vitest;

pub use cypress::CypressPlugin;
pub use eslint::EslintPlugin;
pub use husky::HuskyPlugin;
pub use jest::JestPlugin;
pub use lint_staged::LintStagedPlugin;
pub use nextjs::NextjsPlugin;
pub use playwright::PlaywrightPlugin;
pub use postcss::PostcssPlugin;
pub use registry::PluginRegistry;
pub use storybook::StorybookPlugin;
pub use tailwind::TailwindPlugin;
pub use typescript::TypescriptPlugin;
pub use vite::VitePlugin;
pub use vitest::VitestPlugin;

use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Error types for plugin operations
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config parse error: {0}")]
    ConfigParse(String),
}

/// A glob pattern with an optional base directory for matching.
///
/// Patterns are matched relative to the base directory (or cwd if None).
/// This allows plugins like Storybook to specify patterns like `**/*.stories.tsx`
/// with a base of `src/` to match files in `src/**/*.stories.tsx`.
#[derive(Debug, Clone)]
pub struct EntryPattern {
    /// The glob pattern (e.g., "**/*.stories.tsx")
    pub pattern: String,
    /// Base directory relative to cwd (None = cwd itself)
    pub base: Option<PathBuf>,
}

impl EntryPattern {
    /// Create a new pattern with default base (cwd)
    pub fn new(pattern: impl Into<String>) -> Self {
        Self { pattern: pattern.into(), base: None }
    }

    /// Create a new pattern with a specific base directory
    pub fn with_base(pattern: impl Into<String>, base: impl Into<PathBuf>) -> Self {
        Self { pattern: pattern.into(), base: Some(base.into()) }
    }
}

/// What a plugin returns: patterns to match during collection, or already-resolved paths.
///
/// - `Patterns`: Glob patterns that will be matched during the single filesystem walk.
///   This is the preferred return type for plugins that need to find many files.
/// - `Paths`: Already-resolved absolute paths (e.g., config files found by checking
///   specific locations). Used by plugins that return a fixed set of known files.
/// - `Mixed`: Both patterns and paths. Used by plugins that need both behaviors
///   (e.g., Jest returns config files as paths and test patterns as patterns).
#[derive(Debug, Clone, Default)]
pub enum PluginEntries {
    #[default]
    Empty,
    Patterns(Vec<EntryPattern>),
    Paths(Vec<PathBuf>),
    Mixed {
        patterns: Vec<EntryPattern>,
        paths: Vec<PathBuf>,
    },
}

impl PluginEntries {
    /// Create an empty result
    pub fn empty() -> Self {
        Self::Empty
    }

    /// Create from patterns only
    pub fn patterns(patterns: Vec<EntryPattern>) -> Self {
        if patterns.is_empty() { Self::Empty } else { Self::Patterns(patterns) }
    }

    /// Create from paths only
    pub fn paths(paths: Vec<PathBuf>) -> Self {
        if paths.is_empty() { Self::Empty } else { Self::Paths(paths) }
    }

    /// Create from both patterns and paths
    pub fn mixed(patterns: Vec<EntryPattern>, paths: Vec<PathBuf>) -> Self {
        match (patterns.is_empty(), paths.is_empty()) {
            (true, true) => Self::Empty,
            (true, false) => Self::Paths(paths),
            (false, true) => Self::Patterns(patterns),
            (false, false) => Self::Mixed { patterns, paths },
        }
    }

    /// Get the paths from this PluginEntries (for testing/inspection)
    pub fn get_paths(&self) -> Vec<&PathBuf> {
        match self {
            Self::Empty => vec![],
            Self::Patterns(_) => vec![],
            Self::Paths(paths) => paths.iter().collect(),
            Self::Mixed { paths, .. } => paths.iter().collect(),
        }
    }

    /// Get the patterns from this PluginEntries (for testing/inspection)
    pub fn get_patterns(&self) -> Vec<&EntryPattern> {
        match self {
            Self::Empty => vec![],
            Self::Patterns(patterns) => patterns.iter().collect(),
            Self::Paths(_) => vec![],
            Self::Mixed { patterns, .. } => patterns.iter().collect(),
        }
    }

    /// Check if this PluginEntries is empty
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Get total count of patterns and paths (for testing)
    pub fn total_count(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Patterns(patterns) => patterns.len(),
            Self::Paths(paths) => paths.len(),
            Self::Mixed { patterns, paths } => patterns.len() + paths.len(),
        }
    }
}

/// A plugin discovers entry points from tool-specific configurations
pub trait Plugin: Send + Sync {
    /// Plugin identifier (e.g., "storybook", "jest")
    fn name(&self) -> &str;

    /// Check if this plugin should be enabled based on project dependencies
    fn should_enable(&self, cwd: &Path, dependencies: &FxHashSet<String>) -> bool;

    /// Discover entry point files from project configuration.
    ///
    /// Returns either:
    /// - Patterns to match during the collector's filesystem walk
    /// - Already-resolved absolute paths (for config files, etc.)
    /// - A mix of both
    fn detect_entries(&self, cwd: &Path) -> Result<PluginEntries, PluginError>;
}
