use super::{Compiler, ScssCompiler};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

/// Registry of compilers mapped by file extension
pub struct CompilerRegistry {
    /// Map of extension (with dot) -> compiler
    compilers: FxHashMap<String, Arc<dyn Compiler>>,
}

impl CompilerRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self { compilers: FxHashMap::default() }
    }

    /// Register a compiler for all its supported extensions
    pub fn register(&mut self, compiler: Arc<dyn Compiler>) {
        for ext in compiler.extensions() {
            self.compilers.insert(ext.to_string(), Arc::clone(&compiler));
        }
    }

    /// Register built-in compilers based on detected dependencies
    pub fn register_builtins(&mut self, dependencies: &FxHashSet<String>) {
        let builtins: Vec<Arc<dyn Compiler>> = vec![Arc::new(ScssCompiler::new())];

        for compiler in builtins {
            if compiler.should_enable(dependencies) {
                self.register(compiler);
            }
        }
    }

    /// Get compiler for a file extension (with dot, e.g., ".scss")
    pub fn get(&self, extension: &str) -> Option<&Arc<dyn Compiler>> {
        self.compilers.get(extension)
    }

    /// Get all registered extensions
    pub fn extensions(&self) -> impl Iterator<Item = &String> {
        self.compilers.keys()
    }

    /// Check if a compiler is registered for the given extension
    pub fn has_compiler(&self, extension: &str) -> bool {
        self.compilers.contains_key(extension)
    }
}

impl Default for CompilerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
