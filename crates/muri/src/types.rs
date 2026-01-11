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

    /// Compiler configuration
    pub compilers: CompilerConfig,
}

impl Default for MuriConfig {
    fn default() -> Self {
        Self {
            entry: Vec::new(),
            project: vec!["**/*.{ts,tsx,js,jsx,mjs,cjs}".to_string()],
            cwd: PathBuf::from("."),
            ignore: Vec::new(),
            compilers: CompilerConfig::default(),
        }
    }
}

/// Configuration for file compilers
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CompilerConfig {
    /// Enable/disable SCSS compiler (None = auto-detect based on dependencies)
    #[serde(default)]
    pub scss: Option<bool>,

    /// Enable/disable Vue SFC compiler (None = auto-detect based on dependencies)
    #[serde(default)]
    pub vue: Option<bool>,

    /// Enable/disable Svelte compiler (None = auto-detect based on dependencies)
    #[serde(default)]
    pub svelte: Option<bool>,

    /// Additional file extensions to treat as JS/TS (passthrough to oxc parser)
    #[serde(default)]
    pub extensions: Vec<String>,
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

    #[serde(default)]
    pub compilers: CompilerConfig,
}
