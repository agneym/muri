use super::{Plugin, PluginError};
use glob::glob;
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

/// Plugin to discover Next.js entry points.
///
/// Next.js has two routing systems:
/// - App Router (app/ directory): Uses file-based routing with special files like
///   page.tsx, layout.tsx, loading.tsx, error.tsx, etc.
/// - Pages Router (pages/ directory): Uses file-based routing where each file
///   becomes a route.
///
/// This plugin discovers entry points for both routers, as well as config files
/// and special files like middleware and instrumentation.
pub struct NextjsPlugin;

impl NextjsPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Find Next.js config files
    fn find_config_files(&self, cwd: &Path) -> Vec<PathBuf> {
        let extensions = ["js", "mjs", "ts"];
        let mut found = Vec::new();

        for ext in &extensions {
            let path = cwd.join(format!("next.config.{}", ext));
            if path.exists() && path.is_file() {
                if let Ok(canonical) = path.canonicalize() {
                    found.push(canonical);
                }
            }
        }

        found
    }

    /// Find App Router entry points (app/ directory)
    fn find_app_router_entries(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        let app_dir = cwd.join("app");
        if !app_dir.exists() || !app_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();
        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());

        // App Router special files
        // See: https://nextjs.org/docs/app/building-your-application/routing
        let special_files = [
            // Route files
            ("page", &["js", "jsx", "ts", "tsx"][..]),
            ("layout", &["js", "jsx", "ts", "tsx"][..]),
            ("loading", &["js", "jsx", "ts", "tsx"][..]),
            ("error", &["js", "jsx", "ts", "tsx"][..]),
            ("not-found", &["js", "jsx", "ts", "tsx"][..]),
            ("template", &["js", "jsx", "ts", "tsx"][..]),
            ("default", &["js", "jsx", "ts", "tsx"][..]),
            // Route handlers (API routes in App Router)
            ("route", &["js", "ts"][..]),
        ];

        for (file_name, extensions) in &special_files {
            for ext in *extensions {
                let pattern = app_dir.join(format!("**/{}.{}", file_name, ext));
                let pattern_str = pattern.to_string_lossy();

                for entry in glob(&pattern_str)? {
                    let path = entry?;
                    if let Ok(canonical) = path.canonicalize() {
                        if canonical.starts_with(&cwd_canonical) {
                            entries.push(canonical);
                        }
                    }
                }
            }
        }

        Ok(entries)
    }

    /// Find Pages Router entry points (pages/ directory)
    fn find_pages_router_entries(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        let pages_dir = cwd.join("pages");
        if !pages_dir.exists() || !pages_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();
        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());

        // All JS/TS files in pages/ are entry points
        let extensions = ["js", "jsx", "ts", "tsx"];

        for ext in &extensions {
            let pattern = pages_dir.join(format!("**/*.{}", ext));
            let pattern_str = pattern.to_string_lossy();

            for entry in glob(&pattern_str)? {
                let path = entry?;
                if let Ok(canonical) = path.canonicalize() {
                    if canonical.starts_with(&cwd_canonical) {
                        entries.push(canonical);
                    }
                }
            }
        }

        Ok(entries)
    }

    /// Find special Next.js files (middleware, instrumentation)
    fn find_special_files(&self, cwd: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();

        // middleware.{js,ts} - must be at project root or src/
        // instrumentation.{js,ts} - must be at project root or src/
        let special_files = ["middleware", "instrumentation"];
        let extensions = ["js", "ts"];
        let directories = [cwd.to_path_buf(), cwd.join("src")];

        for dir in &directories {
            for file_name in &special_files {
                for ext in &extensions {
                    let path = dir.join(format!("{}.{}", file_name, ext));
                    if path.exists() && path.is_file() {
                        if let Ok(canonical) = path.canonicalize() {
                            found.push(canonical);
                        }
                    }
                }
            }
        }

        found
    }
}

impl Default for NextjsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for NextjsPlugin {
    fn name(&self) -> &str {
        "nextjs"
    }

    fn should_enable(&self, _cwd: &Path, dependencies: &FxHashSet<String>) -> bool {
        dependencies.contains("next")
    }

    fn detect_entries(&self, cwd: &Path) -> Result<Vec<PathBuf>, PluginError> {
        let mut entries = Vec::new();

        // Add config files
        entries.extend(self.find_config_files(cwd));

        // Add App Router entries
        entries.extend(self.find_app_router_entries(cwd)?);

        // Add Pages Router entries
        entries.extend(self.find_pages_router_entries(cwd)?);

        // Add special files
        entries.extend(self.find_special_files(cwd));

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_with_next() {
        let plugin = NextjsPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("next".to_string());

        let temp = tempdir().unwrap();
        assert!(plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_should_not_enable_without_next() {
        let plugin = NextjsPlugin::new();
        let mut deps = FxHashSet::default();
        deps.insert("react".to_string());

        let temp = tempdir().unwrap();
        assert!(!plugin.should_enable(temp.path(), &deps));
    }

    #[test]
    fn test_find_config_js() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
/** @type {import('next').NextConfig} */
const nextConfig = {};
module.exports = nextConfig;
"#;
        fs::write(temp.path().join("next.config.js"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("next.config.js"));
    }

    #[test]
    fn test_find_config_mjs() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
/** @type {import('next').NextConfig} */
const nextConfig = {};
export default nextConfig;
"#;
        fs::write(temp.path().join("next.config.mjs"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("next.config.mjs"));
    }

    #[test]
    fn test_find_config_ts() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let config_content = r#"
import type { NextConfig } from 'next';

const nextConfig: NextConfig = {};
export default nextConfig;
"#;
        fs::write(temp.path().join("next.config.ts"), config_content).unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("next.config.ts"));
    }

    #[test]
    fn test_app_router_page_files() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        // Create app directory structure
        let app_dir = temp.path().join("app");
        fs::create_dir(&app_dir).unwrap();
        fs::write(app_dir.join("page.tsx"), "export default function Home() {}").unwrap();

        let about_dir = app_dir.join("about");
        fs::create_dir(&about_dir).unwrap();
        fs::write(about_dir.join("page.tsx"), "export default function About() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);

        let filenames: Vec<_> =
            entries.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(filenames.iter().all(|f| f == "page.tsx"));
    }

    #[test]
    fn test_app_router_layout_files() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let app_dir = temp.path().join("app");
        fs::create_dir(&app_dir).unwrap();
        fs::write(app_dir.join("layout.tsx"), "export default function RootLayout() {}").unwrap();

        let dashboard_dir = app_dir.join("dashboard");
        fs::create_dir(&dashboard_dir).unwrap();
        fs::write(dashboard_dir.join("layout.tsx"), "export default function DashboardLayout() {}")
            .unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_app_router_special_files() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let app_dir = temp.path().join("app");
        fs::create_dir(&app_dir).unwrap();

        // Create various special files
        fs::write(app_dir.join("page.tsx"), "export default function Home() {}").unwrap();
        fs::write(app_dir.join("layout.tsx"), "export default function RootLayout() {}").unwrap();
        fs::write(app_dir.join("loading.tsx"), "export default function Loading() {}").unwrap();
        fs::write(app_dir.join("error.tsx"), "'use client'; export default function Error() {}")
            .unwrap();
        fs::write(app_dir.join("not-found.tsx"), "export default function NotFound() {}").unwrap();
        fs::write(app_dir.join("template.tsx"), "export default function Template() {}").unwrap();
        fs::write(app_dir.join("default.tsx"), "export default function Default() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 7);
    }

    #[test]
    fn test_app_router_route_handlers() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let api_dir = temp.path().join("app").join("api").join("users");
        fs::create_dir_all(&api_dir).unwrap();
        fs::write(api_dir.join("route.ts"), "export async function GET() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("route.ts"));
    }

    #[test]
    fn test_pages_router_files() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let pages_dir = temp.path().join("pages");
        fs::create_dir(&pages_dir).unwrap();
        fs::write(pages_dir.join("index.tsx"), "export default function Home() {}").unwrap();
        fs::write(pages_dir.join("about.tsx"), "export default function About() {}").unwrap();
        fs::write(pages_dir.join("_app.tsx"), "export default function App() {}").unwrap();
        fs::write(pages_dir.join("_document.tsx"), "export default function Document() {}")
            .unwrap();

        let api_dir = pages_dir.join("api");
        fs::create_dir(&api_dir).unwrap();
        fs::write(api_dir.join("hello.ts"), "export default function handler() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn test_middleware_file() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        fs::write(temp.path().join("middleware.ts"), "export function middleware() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("middleware.ts"));
    }

    #[test]
    fn test_middleware_file_in_src() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let src_dir = temp.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("middleware.ts"), "export function middleware() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("middleware.ts"));
    }

    #[test]
    fn test_instrumentation_file() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        fs::write(temp.path().join("instrumentation.ts"), "export function register() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("instrumentation.ts"));
    }

    #[test]
    fn test_instrumentation_file_js() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        fs::write(temp.path().join("instrumentation.js"), "export function register() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("instrumentation.js"));
    }

    #[test]
    fn test_combined_app_and_pages_router() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        // Config file
        fs::write(temp.path().join("next.config.js"), "module.exports = {}").unwrap();

        // App Router
        let app_dir = temp.path().join("app");
        fs::create_dir(&app_dir).unwrap();
        fs::write(app_dir.join("page.tsx"), "export default function Home() {}").unwrap();
        fs::write(app_dir.join("layout.tsx"), "export default function RootLayout() {}").unwrap();

        // Pages Router (some projects use both)
        let pages_dir = temp.path().join("pages");
        fs::create_dir(&pages_dir).unwrap();
        fs::write(pages_dir.join("_app.tsx"), "export default function App() {}").unwrap();

        // Middleware
        fs::write(temp.path().join("middleware.ts"), "export function middleware() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        // 1 config + 2 app router + 1 pages router + 1 middleware = 5
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn test_nested_app_router_structure() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        // Create deeply nested structure
        let app_dir = temp.path().join("app");
        let dashboard_dir = app_dir.join("dashboard").join("settings").join("profile");
        fs::create_dir_all(&dashboard_dir).unwrap();
        fs::write(dashboard_dir.join("page.tsx"), "export default function Profile() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_no_entries_empty_project() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_page_js_extension() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let app_dir = temp.path().join("app");
        fs::create_dir(&app_dir).unwrap();
        fs::write(app_dir.join("page.js"), "export default function Home() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("page.js"));
    }

    #[test]
    fn test_page_jsx_extension() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let app_dir = temp.path().join("app");
        fs::create_dir(&app_dir).unwrap();
        fs::write(app_dir.join("page.jsx"), "export default function Home() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("page.jsx"));
    }

    #[test]
    fn test_route_handler_js_extension() {
        let plugin = NextjsPlugin::new();
        let temp = tempdir().unwrap();

        let api_dir = temp.path().join("app").join("api");
        fs::create_dir_all(&api_dir).unwrap();
        fs::write(api_dir.join("route.js"), "export async function GET() {}").unwrap();

        let entries = plugin.detect_entries(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("route.js"));
    }

    #[test]
    fn test_default_impl() {
        let _: NextjsPlugin = Default::default();
    }
}
