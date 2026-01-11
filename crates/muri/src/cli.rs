use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "muri")]
#[command(about = "Find unused files in JS/TS projects")]
pub struct Cli {
    /// Path to config file (muri.json or muri.jsonc)
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Entry point files or glob patterns
    #[arg(short, long)]
    pub entry: Vec<String>,

    /// Project files to check (glob patterns) [default: **/*.{ts,tsx,js,jsx,mjs,cjs}]
    #[arg(short, long)]
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
