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
