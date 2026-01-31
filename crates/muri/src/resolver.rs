use crate::types::{DEFAULT_EXTENSIONS, FOREIGN_FILE_EXTENSIONS};
use oxc_resolver::{ResolveOptions, Resolver, TsconfigOptions, TsconfigReferences};
use std::path::{Path, PathBuf};

pub struct ModuleResolver {
    resolver: Resolver,
}

impl ModuleResolver {
    pub fn new(cwd: &Path) -> Self {
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

    pub fn resolve(&self, from: &Path, specifier: &str) -> Option<PathBuf> {
        let dir = from.parent()?;

        if let Ok(resolution) = self.resolver.resolve(dir, specifier) {
            if let Ok(path) = resolution.into_path_buf().canonicalize() {
                return Some(path);
            }
        }

        None
    }
}
