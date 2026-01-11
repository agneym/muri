pub mod cli;
pub mod collector;
pub mod graph;
pub mod parser;
pub mod reporter;
pub mod resolver;
pub mod types;

use std::sync::Arc;

pub use reporter::Report;
pub use types::{UnusedFilesConfig, UnusedFilesError};

use collector::Collector;
use graph::DependencyGraph;
use resolver::ModuleResolver;

/// Find unused files in a JavaScript/TypeScript project
///
/// # Arguments
/// * `config` - Configuration for the analysis
///
/// # Returns
/// * `Ok(Report)` - Report containing unused files and statistics
/// * `Err(UnusedFilesError)` - Error if entry files not found or invalid cwd
///
/// # Example
/// ```no_run
/// use unused_files::{find_unused_files, UnusedFilesConfig};
/// use std::path::PathBuf;
///
/// let config = UnusedFilesConfig {
///     entry: vec!["src/index.ts".to_string()],
///     cwd: PathBuf::from("."),
///     ..Default::default()
/// };
///
/// let report = find_unused_files(config).unwrap();
/// println!("Found {} unused files", report.unused_count);
/// ```
pub fn find_unused_files(config: UnusedFilesConfig) -> Result<Report, UnusedFilesError> {
    let cwd = config.cwd.canonicalize()?;

    // Collect files
    let collector = Collector::new(&cwd, &config.ignore, config.include_node_modules);
    let entry_files = collector.collect_entry_files(&config.entry);
    let project_files = collector.collect_project_files(&config.project);

    if entry_files.is_empty() {
        return Err(UnusedFilesError::NoEntryFiles(config.entry));
    }

    // Build graph and find unused
    let resolver = Arc::new(ModuleResolver::new(&cwd));
    let graph = DependencyGraph::new(project_files.clone(), resolver);
    let unused = graph.find_unused(&entry_files.into_iter().collect::<Vec<_>>());

    Ok(Report::new(unused, project_files.len()))
}

/// Find all files reachable from entry points
///
/// Returns the set of files that are directly or transitively imported
/// from the specified entry points.
pub fn find_reachable_files(
    config: UnusedFilesConfig,
) -> Result<Vec<std::path::PathBuf>, UnusedFilesError> {
    let cwd = config.cwd.canonicalize()?;

    let collector = Collector::new(&cwd, &config.ignore, config.include_node_modules);
    let entry_files = collector.collect_entry_files(&config.entry);
    let project_files = collector.collect_project_files(&config.project);

    if entry_files.is_empty() {
        return Err(UnusedFilesError::NoEntryFiles(config.entry));
    }

    let resolver = Arc::new(ModuleResolver::new(&cwd));
    let graph = DependencyGraph::new(project_files, resolver);
    let reachable = graph.find_reachable(&entry_files.into_iter().collect::<Vec<_>>());

    let mut result: Vec<_> = reachable.into_iter().collect();
    result.sort();
    Ok(result)
}
