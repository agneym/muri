mod registry;
mod storybook;
mod tailwind;

pub use registry::PluginRegistry;
pub use storybook::StorybookPlugin;
pub use tailwind::TailwindPlugin;

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

    #[error("Glob pattern error: {0}")]
    GlobPattern(#[from] glob::PatternError),

    #[error("Glob error: {0}")]
    GlobError(#[from] glob::GlobError),
}

/// A plugin discovers entry points from tool-specific configurations
pub trait Plugin: Send + Sync {
    /// Plugin identifier (e.g., "storybook", "jest")
    fn name(&self) -> &str;

    /// Check if this plugin should be enabled based on project dependencies
    fn should_enable(&self, cwd: &Path, dependencies: &FxHashSet<String>) -> bool;

    /// Discover entry point files from project configuration
    /// Returns absolute paths to files that should be treated as entry points
    fn detect_entries(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError>;
}
