use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "unused-files")]
#[command(about = "Find unused files in JS/TS projects")]
pub struct Cli {
    /// Entry point files or glob patterns
    #[arg(short, long, required = true)]
    pub entry: Vec<String>,

    /// Project files to check (glob patterns)
    #[arg(short, long, default_value = "**/*.{ts,tsx,js,jsx,mjs,cjs}")]
    pub project: Vec<String>,

    /// Working directory
    #[arg(short = 'C', long, default_value = ".")]
    pub cwd: PathBuf,

    /// Output format
    #[arg(long, default_value = "text")]
    pub format: OutputFormat,

    /// Patterns to ignore
    #[arg(long)]
    pub ignore: Vec<String>,

    /// Include files from node_modules
    #[arg(long, default_value = "false")]
    pub include_node_modules: bool,
}

#[derive(Clone, ValueEnum, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}
