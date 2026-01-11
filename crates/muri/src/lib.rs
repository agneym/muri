pub mod cli;
pub mod collector;
pub mod graph;
pub mod module_cache;
pub mod parser;
pub mod reporter;
pub mod resolver;
pub mod types;

use std::sync::Arc;

pub use reporter::Report;
pub use types::{FileConfig, MuriConfig, MuriError};

use collector::Collector;
use graph::DependencyGraph;
use module_cache::ModuleCache;
use resolver::ModuleResolver;

/// Find unused files in a JavaScript/TypeScript project
///
/// # Arguments
/// * `config` - Configuration for the analysis
///
/// # Returns
/// * `Ok(Report)` - Report containing unused files and statistics
/// * `Err(MuriError)` - Error if entry files not found or invalid cwd
///
/// # Example
/// ```no_run
/// use muri::{find_unused_files, MuriConfig};
/// use std::path::PathBuf;
///
/// let config = MuriConfig {
///     entry: vec!["src/index.ts".to_string()],
///     cwd: PathBuf::from("."),
///     ..Default::default()
/// };
///
/// let report = find_unused_files(config).unwrap();
/// println!("Found {} unused files", report.unused_count);
/// ```
pub fn find_unused_files(config: MuriConfig) -> Result<Report, MuriError> {
    let cwd = config.cwd.canonicalize()?;

    // Single walk to collect both entry and project files
    let collector = Collector::new(&cwd, &config.entry, &config.project, &config.ignore);
    let index = collector.collect();

    if index.entry_files.is_empty() {
        return Err(MuriError::NoEntryFiles(config.entry));
    }

    // Build graph and find unused (with shared module cache for parsing)
    let resolver = Arc::new(ModuleResolver::new(&cwd));
    let module_cache = Arc::new(ModuleCache::new());
    let graph = DependencyGraph::new(index.project_files.clone(), resolver, module_cache);
    let unused = graph.find_unused(&index.entry_files.into_iter().collect::<Vec<_>>());

    Ok(Report::new(unused, index.project_files.len()))
}

/// Find all files reachable from entry points
///
/// Returns the set of files that are directly or transitively imported
/// from the specified entry points.
pub fn find_reachable_files(config: MuriConfig) -> Result<Vec<std::path::PathBuf>, MuriError> {
    let cwd = config.cwd.canonicalize()?;

    // Single walk to collect both entry and project files
    let collector = Collector::new(&cwd, &config.entry, &config.project, &config.ignore);
    let index = collector.collect();

    if index.entry_files.is_empty() {
        return Err(MuriError::NoEntryFiles(config.entry));
    }

    let resolver = Arc::new(ModuleResolver::new(&cwd));
    let module_cache = Arc::new(ModuleCache::new());
    let graph = DependencyGraph::new(index.project_files, resolver, module_cache);
    let reachable = graph.find_reachable(&index.entry_files.into_iter().collect::<Vec<_>>());

    let mut result: Vec<_> = reachable.into_iter().collect();
    result.sort();
    Ok(result)
}
