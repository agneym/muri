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
use plugin::{
    CypressPlugin, EslintPlugin, HuskyPlugin, JestPlugin, LintStagedPlugin, NextjsPlugin,
    PlaywrightPlugin, Plugin, PostcssPlugin, StorybookPlugin, TailwindPlugin, TypescriptPlugin,
    VitePlugin, VitestPlugin,
};
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

    // Tailwind plugin: check config override, then fall back to auto-detection
    let tailwind_plugin = TailwindPlugin::new();
    let tailwind_enabled =
        plugin_config.tailwind.unwrap_or_else(|| tailwind_plugin.should_enable(cwd, deps));

    if tailwind_enabled {
        registry.register(Arc::new(tailwind_plugin));
    }

    // PostCSS plugin: check config override, then fall back to auto-detection
    let postcss_plugin = PostcssPlugin::new();
    let postcss_enabled =
        plugin_config.postcss.unwrap_or_else(|| postcss_plugin.should_enable(cwd, deps));

    if postcss_enabled {
        registry.register(Arc::new(postcss_plugin));
    }

    // Jest plugin: check config override, then fall back to auto-detection
    let jest_plugin = JestPlugin::new();
    let jest_enabled = plugin_config.jest.unwrap_or_else(|| jest_plugin.should_enable(cwd, deps));

    if jest_enabled {
        registry.register(Arc::new(jest_plugin));
    }

    // Vitest plugin: check config override, then fall back to auto-detection
    let vitest_plugin = VitestPlugin::new();
    let vitest_enabled =
        plugin_config.vitest.unwrap_or_else(|| vitest_plugin.should_enable(cwd, deps));

    if vitest_enabled {
        registry.register(Arc::new(vitest_plugin));
    }

    // ESLint plugin: check config override, then fall back to auto-detection
    let eslint_plugin = EslintPlugin::new();
    let eslint_enabled =
        plugin_config.eslint.unwrap_or_else(|| eslint_plugin.should_enable(cwd, deps));

    if eslint_enabled {
        registry.register(Arc::new(eslint_plugin));
    }

    // Next.js plugin: check config override, then fall back to auto-detection
    let nextjs_plugin = NextjsPlugin::new();
    let nextjs_enabled =
        plugin_config.nextjs.unwrap_or_else(|| nextjs_plugin.should_enable(cwd, deps));

    if nextjs_enabled {
        registry.register(Arc::new(nextjs_plugin));
    }

    // Vite plugin: check config override, then fall back to auto-detection
    let vite_plugin = VitePlugin::new();
    let vite_enabled = plugin_config.vite.unwrap_or_else(|| vite_plugin.should_enable(cwd, deps));

    if vite_enabled {
        registry.register(Arc::new(vite_plugin));
    }

    // TypeScript plugin: check config override, then fall back to auto-detection
    let typescript_plugin = TypescriptPlugin::new();
    let typescript_enabled =
        plugin_config.typescript.unwrap_or_else(|| typescript_plugin.should_enable(cwd, deps));

    if typescript_enabled {
        registry.register(Arc::new(typescript_plugin));
    }

    // Cypress plugin: check config override, then fall back to auto-detection
    let cypress_plugin = CypressPlugin::new();
    let cypress_enabled =
        plugin_config.cypress.unwrap_or_else(|| cypress_plugin.should_enable(cwd, deps));

    if cypress_enabled {
        registry.register(Arc::new(cypress_plugin));
    }

    // Playwright plugin: check config override, then fall back to auto-detection
    let playwright_plugin = PlaywrightPlugin::new();
    let playwright_enabled =
        plugin_config.playwright.unwrap_or_else(|| playwright_plugin.should_enable(cwd, deps));

    if playwright_enabled {
        registry.register(Arc::new(playwright_plugin));
    }

    // husky plugin: check config override, then fall back to auto-detection
    let husky_plugin = HuskyPlugin::new();
    let husky_enabled =
        plugin_config.husky.unwrap_or_else(|| husky_plugin.should_enable(cwd, deps));

    if husky_enabled {
        registry.register(Arc::new(husky_plugin));
    }

    // lint-staged plugin: check config override, then fall back to auto-detection
    let lint_staged_plugin = LintStagedPlugin::new();
    let lint_staged_enabled =
        plugin_config.lint_staged.unwrap_or_else(|| lint_staged_plugin.should_enable(cwd, deps));

    if lint_staged_enabled {
        registry.register(Arc::new(lint_staged_plugin));
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

    // Merge plugin-discovered entries into index.
    // Plugin entries (like config files) may be outside the project directory,
    // but we still need to trace their imports to mark project files as reachable.
    for entry in plugin_entries {
        index.entry_files.insert(entry);
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

    // Merge plugin-discovered entries into index.
    // Plugin entries (like config files) may be outside the project directory,
    // but we still need to trace their imports to mark project files as reachable.
    for entry in plugin_entries {
        index.entry_files.insert(entry);
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
