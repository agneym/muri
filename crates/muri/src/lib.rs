pub mod cli;
pub mod collector;
pub mod compiler;
pub mod dependencies;
pub mod graph;
pub mod module_cache;
pub mod parser;
pub mod reporter;
pub mod resolver;
pub mod types;

use std::sync::Arc;

pub use compiler::CompilerRegistry;
pub use reporter::Report;
pub use types::{CompilerConfig, FileConfig, MuriConfig, MuriError};

use collector::Collector;
use dependencies::detect_dependencies;
use graph::DependencyGraph;
use module_cache::ModuleCache;
use resolver::ModuleResolver;

/// Create a compiler registry with built-in compilers enabled based on detected dependencies
/// and user configuration
fn create_compiler_registry(
    cwd: &std::path::Path,
    compiler_config: &types::CompilerConfig,
) -> CompilerRegistry {
    let deps = detect_dependencies(cwd);
    let mut registry = CompilerRegistry::new();

    // SCSS compiler: check config override, then fall back to auto-detection
    let scss_enabled = compiler_config.scss.unwrap_or_else(|| {
        deps.contains("sass") || deps.contains("sass-embedded") || deps.contains("node-sass")
    });

    if scss_enabled {
        registry.register(Arc::new(compiler::ScssCompiler::new()));
    }

    registry
}

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

    // Setup compiler registry based on detected dependencies and user config
    let registry = Arc::new(create_compiler_registry(&cwd, &config.compilers));
    let compiler_extensions: Vec<String> = registry.extensions().cloned().collect();

    // Single walk to collect both entry and project files (with compiler extensions)
    let collector = Collector::with_compiler_extensions(
        &cwd,
        &config.entry,
        &config.project,
        &config.ignore,
        &compiler_extensions,
    );
    let index = collector.collect();

    if index.entry_files.is_empty() {
        return Err(MuriError::NoEntryFiles(config.entry));
    }

    // Build graph and find unused (with shared module cache for parsing)
    let resolver = Arc::new(ModuleResolver::with_compilers(&cwd, &registry));
    let module_cache = Arc::new(ModuleCache::with_compilers(Arc::clone(&registry)));
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

    // Setup compiler registry based on detected dependencies and user config
    let registry = Arc::new(create_compiler_registry(&cwd, &config.compilers));
    let compiler_extensions: Vec<String> = registry.extensions().cloned().collect();

    // Single walk to collect both entry and project files (with compiler extensions)
    let collector = Collector::with_compiler_extensions(
        &cwd,
        &config.entry,
        &config.project,
        &config.ignore,
        &compiler_extensions,
    );
    let index = collector.collect();

    if index.entry_files.is_empty() {
        return Err(MuriError::NoEntryFiles(config.entry));
    }

    let resolver = Arc::new(ModuleResolver::with_compilers(&cwd, &registry));
    let module_cache = Arc::new(ModuleCache::with_compilers(Arc::clone(&registry)));
    let graph = DependencyGraph::new(index.project_files, resolver, module_cache);
    let reachable = graph.find_reachable(&index.entry_files.into_iter().collect::<Vec<_>>());

    let mut result: Vec<_> = reachable.into_iter().collect();
    result.sort();
    Ok(result)
}
