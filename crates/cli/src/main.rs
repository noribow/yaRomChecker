use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use yaromchecker_core::{ScanCache, ScanMode, Scanner};

#[derive(Parser)]
#[command(name = "yarc", version, about = "Verify locally owned ROM files")]
struct Cli {
    /// YAML configuration file (defaults to yaRomChecker.yaml beside the executable)
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Perform an initial full scan
    Scan { path: PathBuf },
    /// Rescan a collection, reusing unchanged cached records
    Quick { path: PathBuf },
    /// Rescan and hash every file
    Full { path: PathBuf },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default = "default_locale")]
    locale: String,
    cache_path: PathBuf,
}

fn default_locale() -> String {
    "en".to_owned()
}

fn default_config_path() -> Result<PathBuf> {
    let executable = std::env::current_exe().context("cannot determine executable path")?;
    Ok(executable
        .parent()
        .context("executable has no parent directory")?
        .join("yaRomChecker.yaml"))
}

fn load_config(path: &Path) -> Result<Config> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("cannot read configuration {}", path.display()))?;
    serde_yaml::from_str(&contents)
        .with_context(|| format!("invalid configuration {}", path.display()))
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();
    let cli = Cli::parse();
    let config_path = cli.config.map(Ok).unwrap_or_else(default_config_path)?;
    let config = load_config(&config_path)?;
    if config.locale != "en" {
        anyhow::bail!("unsupported locale: {}", config.locale);
    }
    let cache_path = if config.cache_path.is_absolute() {
        config.cache_path
    } else {
        config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(config.cache_path)
    };
    let (path, mode) = match cli.command {
        Command::Scan { path } | Command::Full { path } => (path, ScanMode::Full),
        Command::Quick { path } => (path, ScanMode::Quick),
    };
    let mut scanner = Scanner::new(ScanCache::open(&cache_path)?);
    let report = scanner.scan(&path, mode)?;
    println!(
        "Scanned {} entries ({} hashed, {} reused containers).",
        report.entries.len(),
        report.hashed_containers,
        report.reused_containers
    );
    for entry in report.entries {
        println!(
            "{}  {}  {}",
            entry.hashes.sha1, entry.entry_size, entry.entry_path
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_beside_executable() {
        let path = default_config_path().unwrap();
        assert_eq!(path.file_name().unwrap(), "yaRomChecker.yaml");
        assert_eq!(path.parent(), std::env::current_exe().unwrap().parent());
    }
}
