use oxc_resolver::{ResolveError, ResolveOptions, Resolver, TsconfigOptions, TsconfigReferences};
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

        let options = ResolveOptions {
            builtin_modules: true,
            tsconfig,
            extensions: vec![
                ".ts".into(),
                ".tsx".into(),
                ".d.ts".into(),
                ".js".into(),
                ".jsx".into(),
                ".mjs".into(),
                ".cjs".into(),
                ".mts".into(),
                ".cts".into(),
                ".json".into(),
            ],
            extension_alias: vec![
                (".js".into(), vec![".js".into(), ".ts".into(), ".tsx".into()]),
                (
                    ".jsx".into(),
                    vec![".jsx".into(), ".tsx".into()],
                ),
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

        Self {
            resolver: Resolver::new(options),
        }
    }

    pub fn resolve(&self, from: &Path, specifier: &str) -> Option<PathBuf> {
        let dir = from.parent()?;
        match self.resolver.resolve(dir, specifier) {
            Ok(resolution) => resolution.into_path_buf().canonicalize().ok(),
            Err(ResolveError::Builtin { .. }) => None,
            Err(_) => None,
        }
    }
}
