use serde::Deserialize;
use std::path::PathBuf;
use thiserror::Error;

/// Configuration for finding unused files
#[derive(Debug, Clone)]
pub struct MuriConfig {
    /// Entry point files or glob patterns
    pub entry: Vec<String>,

    /// Project files to check (glob patterns)
    pub project: Vec<String>,

    /// Working directory
    pub cwd: PathBuf,

    /// Patterns to ignore
    pub ignore: Vec<String>,
}

impl Default for MuriConfig {
    fn default() -> Self {
        Self {
            entry: Vec::new(),
            project: vec!["**/*.{ts,tsx,js,jsx,mjs,cjs}".to_string()],
            cwd: PathBuf::from("."),
            ignore: Vec::new(),
        }
    }
}

/// Error types for muri operations
#[derive(Error, Debug)]
pub enum MuriError {
    #[error("No entry files found matching patterns: {0:?}")]
    NoEntryFiles(Vec<String>),

    #[error("Invalid working directory: {0}")]
    InvalidCwd(#[from] std::io::Error),
}

/// Config file structure for muri.json / muri.jsonc
#[derive(Debug, Clone, Deserialize)]
pub struct FileConfig {
    #[serde(default)]
    pub entry: Vec<String>,

    #[serde(default)]
    pub project: Vec<String>,

    #[serde(default)]
    pub ignore: Vec<String>,
}
