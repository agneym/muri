use clap::Parser;
use std::sync::Arc;

use unused_files::cli::{Cli, OutputFormat};
use unused_files::collector::Collector;
use unused_files::graph::DependencyGraph;
use unused_files::reporter::{report_json, report_text, Report};
use unused_files::resolver::ModuleResolver;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let cwd = cli.cwd.canonicalize()?;

    // Collect files
    let collector = Collector::new(&cwd, &cli.ignore, cli.include_node_modules);
    let entry_files = collector.collect_entry_files(&cli.entry);
    let project_files = collector.collect_project_files(&cli.project);

    if entry_files.is_empty() {
        eprintln!("Error: No entry files found matching patterns: {:?}", cli.entry);
        std::process::exit(1);
    }

    // Validate entry points exist in project
    for entry in &entry_files {
        if !project_files.contains(entry) {
            eprintln!(
                "Warning: entry point not in project files: {}",
                entry.display()
            );
        }
    }

    // Build graph and find unused
    let resolver = Arc::new(ModuleResolver::new(&cwd));
    let graph = DependencyGraph::new(project_files.clone(), resolver);
    let unused = graph.find_unused(&entry_files.into_iter().collect::<Vec<_>>());

    // Report
    let report = Report::new(unused, project_files.len());

    match cli.format {
        OutputFormat::Text => report_text(&report, &cwd),
        OutputFormat::Json => report_json(&report),
    }

    // Exit with error code if unused files found
    if report.unused_count > 0 {
        std::process::exit(1);
    }

    Ok(())
}
