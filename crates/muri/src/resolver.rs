use crate::compiler::CompilerRegistry;
use crate::types::{DEFAULT_EXTENSIONS, FOREIGN_FILE_EXTENSIONS};
use oxc_resolver::{ResolveOptions, Resolver, TsconfigOptions, TsconfigReferences};
use std::path::{Path, PathBuf};

pub struct ModuleResolver {
    resolver: Resolver,
}

impl ModuleResolver {
    pub fn new(cwd: &Path) -> Self {
        Self::with_extensions(cwd, &[])
    }

    /// Create a new resolver with additional extensions from compilers
    pub fn with_extensions(cwd: &Path, additional_extensions: &[String]) -> Self {
        let tsconfig_path = cwd.join("tsconfig.json");
        let tsconfig = if tsconfig_path.exists() {
            Some(TsconfigOptions {
                config_file: tsconfig_path,
                references: TsconfigReferences::Auto,
            })
        } else {
            None
        };

        // Start with default JS/TS extensions
        let mut extensions: Vec<String> =
            DEFAULT_EXTENSIONS.iter().map(|s| (*s).to_string()).collect();

        // Add foreign file extensions (assets like images, fonts, etc.)
        for ext in FOREIGN_FILE_EXTENSIONS {
            extensions.push((*ext).to_string());
        }

        // Add compiler extensions
        for ext in additional_extensions {
            if !extensions.contains(ext) {
                extensions.push(ext.clone());
            }
        }

        let options = ResolveOptions {
            builtin_modules: true,
            tsconfig,
            extensions,
            extension_alias: vec![
                (".js".into(), vec![".js".into(), ".ts".into(), ".tsx".into()]),
                (".jsx".into(), vec![".jsx".into(), ".tsx".into()]),
                (".mjs".into(), vec![".mjs".into(), ".mts".into()]),
                (".cjs".into(), vec![".cjs".into(), ".cts".into()]),
            ],
            condition_names: vec![
                "import".into(),
                "require".into(),
                "node".into(),
                "default".into(),
            ],
            main_fields: vec!["module".into(), "main".into()],
            ..Default::default()
        };

        Self { resolver: Resolver::new(options) }
    }

    /// Create a new resolver with extensions from a compiler registry
    pub fn with_compilers(cwd: &Path, registry: &CompilerRegistry) -> Self {
        let additional: Vec<String> = registry.extensions().cloned().collect();
        Self::with_extensions(cwd, &additional)
    }

    pub fn resolve(&self, from: &Path, specifier: &str) -> Option<PathBuf> {
        let dir = from.parent()?;

        // First, try standard resolution
        if let Ok(resolution) = self.resolver.resolve(dir, specifier) {
            if let Ok(path) = resolution.into_path_buf().canonicalize() {
                return Some(path);
            }
        }

        // For SCSS/Sass files, try partial resolution (prepend underscore)
        if from.extension().is_some_and(|ext| ext == "scss" || ext == "sass") {
            if let Some(resolved) = self.resolve_scss_partial(dir, specifier) {
                return Some(resolved);
            }
        }

        None
    }

    /// Resolve SCSS partial by trying underscore-prefixed variants
    fn resolve_scss_partial(&self, dir: &Path, specifier: &str) -> Option<PathBuf> {
        use std::path::Path as StdPath;

        let spec_path = StdPath::new(specifier);

        // Get the directory and file name parts
        let (spec_dir, file_name) = if let Some(parent) = spec_path.parent() {
            if parent.as_os_str().is_empty() {
                (None, specifier)
            } else {
                (Some(parent), spec_path.file_name()?.to_str()?)
            }
        } else {
            (None, specifier)
        };

        // Don't try partial resolution if already starts with underscore
        if file_name.starts_with('_') {
            return None;
        }

        // Build the partial name by prepending underscore
        let partial_name = format!("_{file_name}");

        // Build the full specifier with the partial name
        let partial_specifier = if let Some(spec_parent) = spec_dir {
            spec_parent.join(&partial_name).to_string_lossy().to_string()
        } else {
            partial_name
        };

        // Try resolving the partial
        if let Ok(resolution) = self.resolver.resolve(dir, &partial_specifier) {
            if let Ok(path) = resolution.into_path_buf().canonicalize() {
                return Some(path);
            }
        }

        // Also try with explicit SCSS extension
        for ext in [".scss", ".sass"] {
            let partial_with_ext = format!("{partial_specifier}{ext}");
            if let Ok(resolution) = self.resolver.resolve(dir, &partial_with_ext) {
                if let Ok(path) = resolution.into_path_buf().canonicalize() {
                    return Some(path);
                }
            }
        }

        None
    }
}
