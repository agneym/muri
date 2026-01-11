use crate::parser::{ImportInfo, ImportKind, ParseError, extract_imports};
use dashmap::DashMap;
use std::path::{Path, PathBuf};

/// Information about a parsed module, stored in the cache.
/// This struct is extensible for future features (unused exports, etc.)
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// Raw import specifiers and their kinds
    pub imports: Vec<ImportInfo>,
    /// Whether this module has any dynamic imports
    pub has_dynamic_imports: bool,
    /// Parse errors, if any
    pub parse_error: Option<String>,
}

impl ModuleInfo {
    /// Create a ModuleInfo from successfully parsed imports
    pub fn from_imports(imports: Vec<ImportInfo>) -> Self {
        let has_dynamic_imports = imports.iter().any(|i| i.kind == ImportKind::Dynamic);
        Self { imports, has_dynamic_imports, parse_error: None }
    }

    /// Create a ModuleInfo representing a parse failure
    pub fn from_error(error: ParseError) -> Self {
        Self {
            imports: Vec::new(),
            has_dynamic_imports: false,
            parse_error: Some(format!("{error:?}")),
        }
    }
}

/// Thread-safe cache for parsed module information.
/// Allows reusing parse results across the analysis and for future extensions.
pub struct ModuleCache {
    cache: DashMap<PathBuf, ModuleInfo>,
}

impl ModuleCache {
    pub fn new() -> Self {
        Self { cache: DashMap::new() }
    }

    /// Get or compute the ModuleInfo for a file
    pub fn get_or_parse(&self, path: &Path) -> ModuleInfo {
        // Fast path: already cached
        if let Some(info) = self.cache.get(path) {
            return info.clone();
        }

        // Parse the file
        let info = match extract_imports(path) {
            Ok(imports) => ModuleInfo::from_imports(imports),
            Err(e) => ModuleInfo::from_error(e),
        };

        // Insert and return
        self.cache.insert(path.to_path_buf(), info.clone());
        info
    }

    /// Get cached info without parsing (returns None if not cached)
    pub fn get(&self, path: &Path) -> Option<ModuleInfo> {
        self.cache.get(path).map(|r| r.clone())
    }

    /// Number of cached entries
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for ModuleCache {
    fn default() -> Self {
        Self::new()
    }
}
