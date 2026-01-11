use std::path::PathBuf;
use thiserror::Error;

/// Configuration for finding unused files
#[derive(Debug, Clone)]
pub struct UnusedFilesConfig {
    /// Entry point files or glob patterns
    pub entry: Vec<String>,

    /// Project files to check (glob patterns)
    pub project: Vec<String>,

    /// Working directory
    pub cwd: PathBuf,

    /// Patterns to ignore
    pub ignore: Vec<String>,

    /// Include files from node_modules
    pub include_node_modules: bool,
}

impl Default for UnusedFilesConfig {
    fn default() -> Self {
        Self {
            entry: Vec::new(),
            project: vec!["**/*.{ts,tsx,js,jsx,mjs,cjs}".to_string()],
            cwd: PathBuf::from("."),
            ignore: Vec::new(),
            include_node_modules: false,
        }
    }
}

/// Error types for unused-files operations
#[derive(Error, Debug)]
pub enum UnusedFilesError {
    #[error("No entry files found matching patterns: {0:?}")]
    NoEntryFiles(Vec<String>),

    #[error("Invalid working directory: {0}")]
    InvalidCwd(#[from] std::io::Error),

    #[error("Entry point not in project files: {0}")]
    EntryNotInProject(PathBuf),
}
