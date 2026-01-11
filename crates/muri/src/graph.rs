use crate::parser::extract_imports;
use crate::resolver::ModuleResolver;
use dashmap::DashSet;
use rayon::prelude::*;
use rustc_hash::FxHashSet;
use std::path::PathBuf;
use std::sync::Arc;

pub struct DependencyGraph {
    project_files: FxHashSet<PathBuf>,
    resolver: Arc<ModuleResolver>,
}

impl DependencyGraph {
    pub fn new(project_files: FxHashSet<PathBuf>, resolver: Arc<ModuleResolver>) -> Self {
        Self { project_files, resolver }
    }

    pub fn find_reachable(&self, entry_points: &[PathBuf]) -> FxHashSet<PathBuf> {
        let reachable: DashSet<PathBuf> = DashSet::new();
        let queue: DashSet<PathBuf> = DashSet::new();

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

                if let Ok(imports) = extract_imports(file) {
                    for import in imports {
                        if let Some(resolved) = self.resolver.resolve(file, &import.source) {
                            if self.project_files.contains(&resolved)
                                && !reachable.contains(&resolved)
                            {
                                queue.insert(resolved);
                            }
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
