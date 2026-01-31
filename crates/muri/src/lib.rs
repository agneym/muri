pub mod cli;
pub mod collector;
pub mod dependencies;
pub mod graph;
pub mod module_cache;
pub mod parser;
pub mod plugin;
pub mod reporter;
pub mod resolver;
pub mod types;

use std::sync::Arc;

pub use plugin::PluginRegistry;
pub use reporter::Report;
pub use types::{
    DEFAULT_EXTENSIONS, FOREIGN_FILE_EXTENSIONS, FileConfig, MuriConfig, MuriError, PluginConfig,
};

use collector::Collector;
use dependencies::detect_dependencies;
use graph::DependencyGraph;
use module_cache::ModuleCache;
use plugin::{Plugin, StorybookPlugin};
use resolver::ModuleResolver;
use rustc_hash::FxHashSet;

/// Create a plugin registry with built-in plugins enabled based on detected dependencies
/// and user configuration
fn create_plugin_registry(
    cwd: &std::path::Path,
    plugin_config: &types::PluginConfig,
    deps: &FxHashSet<String>,
) -> PluginRegistry {
    let mut registry = PluginRegistry::new();

    // Storybook plugin: check config override, then fall back to auto-detection
    let storybook_plugin = StorybookPlugin::new();
    let storybook_enabled =
        plugin_config.storybook.unwrap_or_else(|| storybook_plugin.should_enable(cwd, deps));

    if storybook_enabled {
        registry.register(Arc::new(storybook_plugin));
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

    // Detect dependencies for plugins
    let deps = detect_dependencies(&cwd);

    // Single walk to collect both entry and project files
    let collector = Collector::new(&cwd, &config.entry, &config.project, &config.ignore);
    let mut index = collector.collect();

    // Run plugins to discover additional entry points
    let plugin_registry = create_plugin_registry(&cwd, &config.plugins, &deps);
    let plugin_entries = plugin_registry.detect_all_entries(&cwd);

    // Merge plugin-discovered entries into index (only if they're in project files)
    for entry in plugin_entries {
        if index.project_files.contains(&entry) {
            index.entry_files.insert(entry);
        }
    }

    if index.entry_files.is_empty() {
        return Err(MuriError::NoEntryFiles(config.entry));
    }

    // Build graph and find unused (with shared module cache for parsing)
    let resolver = Arc::new(ModuleResolver::new(&cwd));
    let module_cache = Arc::new(ModuleCache::new());
    let graph =
        DependencyGraph::new(index.project_files.clone(), resolver, module_cache, config.verbose);
    let unused = graph.find_unused(&index.entry_files.into_iter().collect::<Vec<_>>());

    Ok(Report::new(unused, index.project_files.len()))
}

/// Find all files reachable from entry points
///
/// Returns the set of files that are directly or transitively imported
/// from the specified entry points.
pub fn find_reachable_files(config: MuriConfig) -> Result<Vec<std::path::PathBuf>, MuriError> {
    let cwd = config.cwd.canonicalize()?;

    // Detect dependencies for plugins
    let deps = detect_dependencies(&cwd);

    // Single walk to collect both entry and project files
    let collector = Collector::new(&cwd, &config.entry, &config.project, &config.ignore);
    let mut index = collector.collect();

    // Run plugins to discover additional entry points
    let plugin_registry = create_plugin_registry(&cwd, &config.plugins, &deps);
    let plugin_entries = plugin_registry.detect_all_entries(&cwd);

    // Merge plugin-discovered entries into index (only if they're in project files)
    for entry in plugin_entries {
        if index.project_files.contains(&entry) {
            index.entry_files.insert(entry);
        }
    }

    if index.entry_files.is_empty() {
        return Err(MuriError::NoEntryFiles(config.entry));
    }

    let resolver = Arc::new(ModuleResolver::new(&cwd));
    let module_cache = Arc::new(ModuleCache::new());
    let graph = DependencyGraph::new(index.project_files, resolver, module_cache, config.verbose);
    let reachable = graph.find_reachable(&index.entry_files.into_iter().collect::<Vec<_>>());

    let mut result: Vec<_> = reachable.into_iter().collect();
    result.sort();
    Ok(result)
}
