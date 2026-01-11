use muri::{MuriConfig, find_reachable_files, find_unused_files};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::path::PathBuf;

/// Options for finding unused files
#[napi(object)]
pub struct UnusedFilesOptions {
    /// Entry point files or glob patterns
    pub entry: Vec<String>,

    /// Project files to check (glob patterns)
    pub project: Option<Vec<String>>,

    /// Working directory (defaults to current directory)
    pub cwd: Option<String>,

    /// Patterns to ignore
    pub ignore: Option<Vec<String>>,
}

/// Report of unused files analysis
#[napi(object)]
pub struct UnusedFilesReport {
    /// List of unused file paths (relative to cwd)
    pub unused_files: Vec<String>,

    /// Total number of project files analyzed
    pub total_files: u32,

    /// Number of unused files found
    pub unused_count: u32,
}

impl From<UnusedFilesOptions> for MuriConfig {
    fn from(opts: UnusedFilesOptions) -> Self {
        MuriConfig {
            entry: opts.entry,
            project: opts
                .project
                .unwrap_or_else(|| vec!["**/*.{ts,tsx,js,jsx,mjs,cjs}".to_string()]),
            cwd: opts.cwd.map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")),
            ignore: opts.ignore.unwrap_or_default(),
            compilers: Default::default(),
        }
    }
}

/// Find unused files in a JavaScript/TypeScript project (async)
///
/// @param options - Configuration options
/// @returns Promise with report containing unused files and statistics
#[napi]
pub async fn find_unused(options: UnusedFilesOptions) -> Result<UnusedFilesReport> {
    let config: MuriConfig = options.into();
    let cwd = config.cwd.clone();

    // Run CPU-bound work in blocking thread pool
    let result = tokio::task::spawn_blocking(move || find_unused_files(config))
        .await
        .map_err(|e| Error::from_reason(format!("Task panicked: {e}")))?
        .map_err(|e| Error::from_reason(e.to_string()))?;

    // Convert PathBuf to relative string paths
    let cwd_canonical =
        cwd.canonicalize().map_err(|e| Error::from_reason(format!("Invalid cwd: {e}")))?;

    let unused_files: Vec<String> = result
        .unused_files
        .iter()
        .map(|p| p.strip_prefix(&cwd_canonical).unwrap_or(p).to_string_lossy().to_string())
        .collect();

    Ok(UnusedFilesReport {
        unused_files,
        total_files: result.total_files as u32,
        unused_count: result.unused_count as u32,
    })
}

/// Find unused files in a JavaScript/TypeScript project (sync)
///
/// @param options - Configuration options
/// @returns Report containing unused files and statistics
#[napi]
pub fn find_unused_sync(options: UnusedFilesOptions) -> Result<UnusedFilesReport> {
    let config: MuriConfig = options.into();
    let cwd = config.cwd.clone();

    let result = find_unused_files(config).map_err(|e| Error::from_reason(e.to_string()))?;

    let cwd_canonical =
        cwd.canonicalize().map_err(|e| Error::from_reason(format!("Invalid cwd: {e}")))?;

    let unused_files: Vec<String> = result
        .unused_files
        .iter()
        .map(|p| p.strip_prefix(&cwd_canonical).unwrap_or(p).to_string_lossy().to_string())
        .collect();

    Ok(UnusedFilesReport {
        unused_files,
        total_files: result.total_files as u32,
        unused_count: result.unused_count as u32,
    })
}

/// Find all files reachable from entry points (async)
///
/// @param options - Configuration options
/// @returns Promise with array of reachable file paths
#[napi]
pub async fn find_reachable(options: UnusedFilesOptions) -> Result<Vec<String>> {
    let config: MuriConfig = options.into();
    let cwd = config.cwd.clone();

    let result = tokio::task::spawn_blocking(move || find_reachable_files(config))
        .await
        .map_err(|e| Error::from_reason(format!("Task panicked: {e}")))?
        .map_err(|e| Error::from_reason(e.to_string()))?;

    let cwd_canonical =
        cwd.canonicalize().map_err(|e| Error::from_reason(format!("Invalid cwd: {e}")))?;

    Ok(result
        .iter()
        .map(|p| p.strip_prefix(&cwd_canonical).unwrap_or(p).to_string_lossy().to_string())
        .collect())
}
