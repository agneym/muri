use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct Report {
    pub unused_files: Vec<PathBuf>,
    pub total_files: usize,
    pub unused_count: usize,
}

impl Report {
    pub fn new(unused_files: Vec<PathBuf>, total_files: usize) -> Self {
        let unused_count = unused_files.len();
        Self { unused_files, total_files, unused_count }
    }
}

pub fn report_text(report: &Report, cwd: &Path) {
    if report.unused_files.is_empty() {
        println!("No unused files found.");
        return;
    }

    println!("Unused files ({}):", report.unused_count);
    for file in &report.unused_files {
        let relative = file.strip_prefix(cwd).unwrap_or(file);
        println!("  {}", relative.display());
    }
    println!("\n{}/{} files unused", report.unused_count, report.total_files);
}

pub fn report_json(report: &Report) {
    println!("{}", serde_json::to_string_pretty(report).unwrap());
}
