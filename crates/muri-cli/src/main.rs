use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};

use muri::cli::{Cli, OutputFormat};
use muri::reporter::{report_json, report_text};
use muri::{FileConfig, MuriConfig, MuriError, find_unused_files};

/// Find default config file in directory
fn find_default_config(dir: &Path) -> Option<PathBuf> {
    let json_path = dir.join("muri.json");
    if json_path.exists() {
        return Some(json_path);
    }

    let jsonc_path = dir.join("muri.jsonc");
    if jsonc_path.exists() {
        return Some(jsonc_path);
    }

    None
}

/// Load config from file path, supporting .json and .jsonc
fn load_config_file(path: &Path) -> Result<FileConfig, Box<dyn std::error::Error>> {
    let mut content = fs::read_to_string(path)?;
    json_strip_comments::strip(&mut content)?;
    let config: FileConfig = serde_json::from_str(&content)?;
    Ok(config)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Load config file
    let file_config = if let Some(config_path) = &cli.config {
        // Use specified config file (error if not found)
        if !config_path.exists() {
            eprintln!("Error: Config file not found: {}", config_path.display());
            std::process::exit(1);
        }
        Some(load_config_file(config_path)?)
    } else {
        // Look for default config file in cwd
        match find_default_config(&cli.cwd) {
            Some(path) => match load_config_file(&path) {
                Ok(cfg) => Some(cfg),
                Err(e) => {
                    eprintln!("Warning: Failed to parse config file '{}': {}", path.display(), e);
                    None
                }
            },
            None => None,
        }
    };

    // Merge config: CLI args override file config
    let entry = if !cli.entry.is_empty() {
        cli.entry
    } else if let Some(ref cfg) = file_config {
        if cfg.entry.is_empty() {
            eprintln!("Error: No entry files specified in config or CLI");
            std::process::exit(1);
        }
        cfg.entry.clone()
    } else {
        eprintln!("Error: No entry files specified. Use --entry or provide a config file.");
        std::process::exit(1);
    };

    let default_project = vec!["**/*.{ts,tsx,js,jsx,mjs,cjs}".to_string()];
    let project = if !cli.project.is_empty() {
        cli.project
    } else if let Some(ref cfg) = file_config {
        if !cfg.project.is_empty() { cfg.project.clone() } else { default_project }
    } else {
        default_project
    };

    let ignore = if !cli.ignore.is_empty() {
        cli.ignore
    } else if let Some(ref cfg) = file_config {
        cfg.ignore.clone()
    } else {
        Vec::new()
    };

    let config = MuriConfig {
        entry,
        project,
        cwd: cli.cwd.clone(),
        ignore,
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
        Err(MuriError::NoEntryFiles(patterns)) => {
            eprintln!("Error: No entry files found matching patterns: {patterns:?}");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}
