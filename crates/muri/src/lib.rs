pub mod cli;
pub mod collector;
pub mod compiler;
pub mod dependencies;
pub mod graph;
pub mod module_cache;
pub mod parser;
pub mod plugin;
pub mod reporter;
pub mod resolver;
pub mod types;

use std::sync::Arc;

pub use compiler::CompilerRegistry;
pub use plugin::PluginRegistry;
pub use reporter::Report;
pub use types::{
    CompilerConfig, DEFAULT_EXTENSIONS, FOREIGN_FILE_EXTENSIONS, FileConfig, MuriConfig, MuriError,
    PluginConfig,
};

use collector::Collector;
use dependencies::detect_dependencies;
use graph::DependencyGraph;
use module_cache::ModuleCache;
use plugin::{Plugin, StorybookPlugin};
use resolver::ModuleResolver;
use rustc_hash::FxHashSet;

/// Create a compiler registry with built-in compilers enabled based on detected dependencies
/// and user configuration
fn create_compiler_registry(
    compiler_config: &types::CompilerConfig,
    deps: &FxHashSet<String>,
) -> CompilerRegistry {
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

    // Detect dependencies once for both compilers and plugins
    let deps = detect_dependencies(&cwd);

    // Setup compiler registry based on detected dependencies and user config
    let registry = Arc::new(create_compiler_registry(&config.compilers, &deps));
    let compiler_extensions: Vec<String> = registry.extensions().cloned().collect();

    // Single walk to collect both entry and project files (with compiler extensions)
    let collector = Collector::with_compiler_extensions(
        &cwd,
        &config.entry,
        &config.project,
        &config.ignore,
        &compiler_extensions,
    );
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

    // Detect dependencies once for both compilers and plugins
    let deps = detect_dependencies(&cwd);

    // Setup compiler registry based on detected dependencies and user config
    let registry = Arc::new(create_compiler_registry(&config.compilers, &deps));
    let compiler_extensions: Vec<String> = registry.extensions().cloned().collect();

    // Single walk to collect both entry and project files (with compiler extensions)
    let collector = Collector::with_compiler_extensions(
        &cwd,
        &config.entry,
        &config.project,
        &config.ignore,
        &compiler_extensions,
    );
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

    let resolver = Arc::new(ModuleResolver::with_compilers(&cwd, &registry));
    let module_cache = Arc::new(ModuleCache::with_compilers(Arc::clone(&registry)));
    let graph = DependencyGraph::new(index.project_files, resolver, module_cache);
    let reachable = graph.find_reachable(&index.entry_files.into_iter().collect::<Vec<_>>());

    let mut result: Vec<_> = reachable.into_iter().collect();
    result.sort();
    Ok(result)
}
