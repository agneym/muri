use clap::Parser;

use unused_files::cli::{Cli, OutputFormat};
use unused_files::reporter::{report_json, report_text};
use unused_files::{find_unused_files, UnusedFilesConfig, UnusedFilesError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let config = UnusedFilesConfig {
        entry: cli.entry,
        project: cli.project,
        cwd: cli.cwd.clone(),
        ignore: cli.ignore,
        include_node_modules: cli.include_node_modules,
    };

    let cwd = config.cwd.canonicalize()?;

    match find_unused_files(config) {
        Ok(report) => {
            match cli.format {
                OutputFormat::Text => report_text(&report, &cwd),
                OutputFormat::Json => report_json(&report),
            }

            // Exit with error code if unused files found
            if report.unused_count > 0 {
                std::process::exit(1);
            }
        }
        Err(UnusedFilesError::NoEntryFiles(patterns)) => {
            eprintln!("Error: No entry files found matching patterns: {:?}", patterns);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
