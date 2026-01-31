use serde::Deserialize;
use std::path::PathBuf;
use thiserror::Error;

/// Default extensions for JavaScript/TypeScript module resolution
pub const DEFAULT_EXTENSIONS: &[&str] =
    &[".ts", ".tsx", ".d.ts", ".js", ".jsx", ".mjs", ".cjs", ".mts", ".cts", ".json"];

/// Foreign file extensions - assets that can be imported but don't contain JS/TS code.
/// These files can be resolved but are not added to the reachable set or parsed for imports.
pub const FOREIGN_FILE_EXTENSIONS: &[&str] = &[
    ".avif", ".css", ".eot", ".gif", ".html", ".ico", ".jpeg", ".jpg", ".less", ".mp3", ".png",
    ".sass", ".scss", ".sh", ".svg", ".ttf", ".webp", ".woff", ".woff2", ".yaml", ".yml",
];

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

    /// Plugin configuration
    pub plugins: PluginConfig,

    /// Enable verbose output
    pub verbose: bool,
}

impl Default for MuriConfig {
    fn default() -> Self {
        Self {
            entry: Vec::new(),
            project: vec!["**/*.{ts,tsx,js,jsx,mjs,cjs}".to_string()],
            cwd: PathBuf::from("."),
            ignore: Vec::new(),
            plugins: PluginConfig::default(),
            verbose: false,
        }
    }
}

/// Configuration for plugins that discover entry points
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginConfig {
    /// Enable/disable Storybook plugin (None = auto-detect based on dependencies)
    #[serde(default)]
    pub storybook: Option<bool>,

    /// Enable/disable Tailwind CSS plugin (None = auto-detect based on dependencies)
    #[serde(default)]
    pub tailwind: Option<bool>,

    /// Enable/disable PostCSS plugin (None = auto-detect based on dependencies)
    #[serde(default)]
    pub postcss: Option<bool>,

    /// Enable/disable Jest plugin (None = auto-detect based on dependencies)
    #[serde(default)]
    pub jest: Option<bool>,

    /// Enable/disable Vitest plugin (None = auto-detect based on dependencies)
    #[serde(default)]
    pub vitest: Option<bool>,

    /// Enable/disable Next.js plugin (None = auto-detect based on dependencies)
    #[serde(default)]
    pub nextjs: Option<bool>,
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
    pub plugins: PluginConfig,
}
