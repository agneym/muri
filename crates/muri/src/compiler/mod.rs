mod registry;
mod scss;

pub use registry::CompilerRegistry;
pub use scss::ScssCompiler;

use crate::parser::ImportInfo;
use std::path::Path;
use thiserror::Error;

/// Error types for compiler operations
#[derive(Error, Debug)]
pub enum CompilerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Compilation failed: {0}")]
    CompileFailed(String),
}

/// Result of compiling a non-JS file
#[derive(Debug, Default)]
pub struct CompilerOutput {
    /// Extracted import information (JS/TS compatible)
    pub imports: Vec<ImportInfo>,
}

/// A compiler transforms non-JS/TS files into import information
pub trait Compiler: Send + Sync {
    /// File extensions this compiler handles (e.g., [".scss", ".sass"])
    fn extensions(&self) -> &[&str];

    /// Check if this compiler should be enabled based on project dependencies
    fn should_enable(&self, dependencies: &rustc_hash::FxHashSet<String>) -> bool;

    /// Extract imports from file content
    fn compile(&self, content: &str, file_path: &Path) -> Result<CompilerOutput, CompilerError>;
}
