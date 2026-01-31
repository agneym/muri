use crate::module_cache::ModuleCache;
use crate::resolver::ModuleResolver;
use crate::types::FOREIGN_FILE_EXTENSIONS;
use dashmap::DashSet;
use rayon::prelude::*;
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Check if a file has a foreign file extension (CSS, images, etc.)
fn is_foreign_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|ext| {
        let ext_with_dot = format!(".{ext}");
        FOREIGN_FILE_EXTENSIONS.contains(&ext_with_dot.as_str())
    })
}

pub struct DependencyGraph {
    project_files: FxHashSet<PathBuf>,
    resolver: Arc<ModuleResolver>,
    module_cache: Arc<ModuleCache>,
    verbose: bool,
}

impl DependencyGraph {
    pub fn new(
        project_files: FxHashSet<PathBuf>,
        resolver: Arc<ModuleResolver>,
        module_cache: Arc<ModuleCache>,
        verbose: bool,
    ) -> Self {
        Self { project_files, resolver, module_cache, verbose }
    }

    pub fn find_reachable(&self, entry_points: &[PathBuf]) -> FxHashSet<PathBuf> {
        let reachable: DashSet<PathBuf> = DashSet::new();
        let queue: DashSet<PathBuf> = DashSet::new();
        let warned_foreign: DashSet<PathBuf> = DashSet::new();

        // Seed with entry points
        for entry in entry_points {
            queue.insert(entry.clone());
        }

        // Process in waves for parallelism
        while !queue.is_empty() {
            let current_wave: Vec<_> = queue.iter().map(|r| r.clone()).collect();
            queue.clear();

            current_wave.par_iter().for_each(|file| {
                if !reachable.insert(file.clone()) {
                    return; // Already processed
                }

                // Use cached module info instead of re-parsing
                let module_info = self.module_cache.get_or_parse(file);
                for import in &module_info.imports {
                    if let Some(resolved) = self.resolver.resolve(file, &import.source) {
                        if self.project_files.contains(&resolved) {
                            if !reachable.contains(&resolved) {
                                queue.insert(resolved);
                            }
                        } else if self.verbose
                            && is_foreign_file(&resolved)
                            && warned_foreign.insert(resolved.clone())
                        {
                            eprintln!(
                                "Warning: Foreign file '{}' will not be analyzed",
                                resolved.display()
                            );
                        }
                    }
                }
            });
        }

        reachable.into_iter().collect()
    }

    pub fn find_unused(&self, entry_points: &[PathBuf]) -> Vec<PathBuf> {
        let reachable = self.find_reachable(entry_points);

        let mut unused: Vec<_> = self.project_files.difference(&reachable).cloned().collect();

        unused.sort();
        unused
    }
}
