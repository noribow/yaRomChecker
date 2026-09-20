use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use serde::Deserialize;
use yaromchecker_core::{
    DatFile, DatRomStatus, DumpStatus, FileStatus, ScanCache, ScanMode, Scanner, match_collection,
};

const EN_MESSAGES: &str = include_str!("../../../locales/en.yaml");
const JA_MESSAGES: &str = include_str!("../../../locales/ja.yaml");
const DEFAULT_CONFIG: &str = include_str!("../../../yaRomChecker.example.yaml");

#[derive(Parser)]
#[command(name = "yarc", version)]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Scan { path: PathBuf },
    Quick { path: PathBuf },
    Full { path: PathBuf },
    Verify { path: PathBuf },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default = "default_locale")]
    locale: String,
    cache_path: PathBuf,
    #[serde(default)]
    dats: Vec<PathBuf>,
}

struct Messages {
    fallback: HashMap<String, String>,
    selected: HashMap<String, String>,
}

impl Messages {
    fn load(locale: &str) -> Result<Self> {
        let fallback: HashMap<String, String> =
            serde_yml::from_str(EN_MESSAGES).context("invalid embedded English locale")?;
        let selected = match locale {
            "en" => fallback.clone(),
            "ja" => serde_yml::from_str(JA_MESSAGES).context("invalid embedded Japanese locale")?,
            _ => anyhow::bail!(
                "{}: {locale}",
                fallback
                    .get("unsupported_locale")
                    .map(String::as_str)
                    .unwrap_or("Unsupported locale")
            ),
        };
        Ok(Self { fallback, selected })
    }

    fn text<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.selected
            .get(key)
            .or_else(|| self.fallback.get(key))
            .map(String::as_str)
            .unwrap_or(fallback)
    }

    fn format(&self, key: &str, fallback: &str, values: &[(&str, String)]) -> String {
        let mut message = self.text(key, fallback).to_owned();
        for (name, value) in values {
            message = message.replace(&format!("{{{name}}}"), value);
        }
        message
    }
}

fn default_locale() -> String {
    "en".to_owned()
}

fn default_config_path(messages: &Messages) -> Result<PathBuf> {
    let executable = std::env::current_exe().with_context(|| {
        messages
            .text("executable_path_error", "Cannot determine executable path")
            .to_owned()
    })?;
    Ok(executable
        .parent()
        .with_context(|| {
            messages
                .text(
                    "executable_parent_error",
                    "Executable has no parent directory",
                )
                .to_owned()
        })?
        .join("yaRomChecker.yaml"))
}

fn ensure_config(path: &Path, messages: &Messages) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).with_context(|| {
            messages.format(
                "config_write_error",
                "Cannot write configuration {path}",
                &[("path", path.display().to_string())],
            )
        })?;
    }
    fs::write(path, DEFAULT_CONFIG).with_context(|| {
        messages.format(
            "config_write_error",
            "Cannot write configuration {path}",
            &[("path", path.display().to_string())],
        )
    })?;
    Ok(true)
}

fn load_config(path: &Path, messages: &Messages) -> Result<Config> {
    let contents = fs::read_to_string(path).with_context(|| {
        messages.format(
            "config_read_error",
            "Cannot read configuration {path}",
            &[("path", path.display().to_string())],
        )
    })?;
    serde_yml::from_str(&contents).with_context(|| {
        messages.format(
            "config_invalid_error",
            "Invalid configuration {path}",
            &[("path", path.display().to_string())],
        )
    })
}

fn parse_cli(messages: &Messages) -> Cli {
    let command = Cli::command()
        .about(
            messages
                .text("cli_about", "Verify locally owned ROM files")
                .to_owned(),
        )
        .mut_arg("config", |arg| {
            arg.help(
                messages
                    .text(
                        "config_help",
                        "YAML configuration file (defaults to yaRomChecker.yaml beside the executable)",
                    )
                    .to_owned(),
            )
        })
        .mut_subcommand("scan", |command| {
            command.about(
                messages
                    .text("scan_help", "Perform an initial full scan")
                    .to_owned(),
            )
        })
        .mut_subcommand("quick", |command| {
            command.about(
                messages
                    .text(
                        "quick_help",
                        "Rescan a collection, reusing unchanged cached records",
                    )
                    .to_owned(),
            )
        })
        .mut_subcommand("full", |command| {
            command.about(
                messages
                    .text("full_help", "Rescan and hash every file")
                    .to_owned(),
            )
        })
        .mut_subcommand("verify", |command| {
            command.about(
                messages
                    .text(
                        "verify_help",
                        "Scan a collection and match it against configured DAT files",
                    )
                    .to_owned(),
            )
        });
    Cli::from_arg_matches(&command.get_matches()).expect("clap validated the arguments")
}

fn run() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();
    let english = Messages::load("en")?;
    let cli = parse_cli(&english);
    let config_path = cli
        .config
        .map(Ok)
        .unwrap_or_else(|| default_config_path(&english))?;
    let created = ensure_config(&config_path, &english)?;
    let config = load_config(&config_path, &english)?;
    let messages = Messages::load(&config.locale)?;
    if created {
        println!(
            "{}",
            messages.format(
                "config_created",
                "Created configuration {path}",
                &[("path", config_path.display().to_string())],
            )
        );
    }
    let cache_path = resolve_path(&config_path, config.cache_path.clone());
    let (path, mode, verify) = match cli.command {
        Command::Scan { path } | Command::Full { path } => (path, ScanMode::Full, false),
        Command::Quick { path } => (path, ScanMode::Quick, false),
        Command::Verify { path } => (path, ScanMode::Quick, true),
    };
    let mut scanner = Scanner::new(ScanCache::open(&cache_path).with_context(|| {
        messages.format(
            "cache_open_error",
            "Cannot open scan cache {path}",
            &[("path", cache_path.display().to_string())],
        )
    })?);
    let report = scanner.scan(&path, mode).with_context(|| {
        messages.format(
            "scan_error",
            "Cannot scan {path}",
            &[("path", path.display().to_string())],
        )
    })?;
    println!(
        "{}",
        messages.format(
            "scan_summary",
            "Scanned {entries} entries ({hashed} hashed, {reused} reused containers).",
            &[
                ("entries", report.entries.len().to_string()),
                ("hashed", report.hashed_containers.to_string()),
                ("reused", report.reused_containers.to_string()),
            ],
        )
    );
    for entry in &report.entries {
        println!(
            "{}",
            messages.format(
                "scan_entry",
                "{sha1}  {size}  {path}",
                &[
                    ("sha1", entry.hashes.sha1.clone()),
                    ("size", entry.entry_size.to_string()),
                    ("path", entry.entry_path.clone()),
                ],
            )
        );
    }
    if verify {
        print_dat_report(&config, &config_path, &report.entries, &messages)?;
    }
    Ok(())
}

fn resolve_path(base: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base.parent().unwrap_or_else(|| Path::new(".")).join(path)
    }
}

fn print_dat_report(
    config: &Config,
    config_path: &Path,
    entries: &[yaromchecker_core::ScanEntry],
    messages: &Messages,
) -> Result<()> {
    if config.dats.is_empty() {
        anyhow::bail!(
            "{}",
            messages.text(
                "dat_missing_config",
                "No DAT files configured. Add paths under dats: in the YAML config."
            )
        );
    }
    let mut dats = Vec::new();
    for dat in &config.dats {
        let path = resolve_path(config_path, dat.clone());
        let loaded = DatFile::load(&path).with_context(|| {
            messages.format(
                "dat_read_error",
                "Cannot read DAT {path}",
                &[("path", path.display().to_string())],
            )
        })?;
        println!(
            "{}",
            messages.format(
                "dat_header_line",
                "DAT: {name} {version}",
                &[
                    (
                        "name",
                        loaded
                            .header
                            .name
                            .clone()
                            .unwrap_or_else(|| path.display().to_string()),
                    ),
                    ("version", loaded.header.version.clone().unwrap_or_default()),
                ],
            )
        );
        dats.push(loaded);
    }
    let report = match_collection(entries, &dats);
    let present = report
        .roms
        .iter()
        .filter(|rom| rom.status == DatRomStatus::Present)
        .count();
    let missing = report
        .roms
        .iter()
        .filter(|rom| rom.status == DatRomStatus::Missing)
        .count();
    let nodump = report
        .roms
        .iter()
        .filter(|rom| rom.status == DatRomStatus::NoDump)
        .count();
    println!(
        "{}",
        messages.format(
            "dat_summary",
            "DAT verification: {files} files, {present} present, {missing} missing, {nodump} nodump.",
            &[
                ("files", report.files.len().to_string()),
                ("present", present.to_string()),
                ("missing", missing.to_string()),
                ("nodump", nodump.to_string()),
            ],
        )
    );
    for entry in &report.files {
        let status = marked_file_status(messages, entry.status, entry.dump_status);
        if entry.status == FileStatus::Extra {
            println!(
                "{}",
                messages.format(
                    "dat_extra_line",
                    "{status}  {path}",
                    &[("status", status), ("path", entry.entry_path.clone()),],
                )
            );
            continue;
        }
        println!(
            "{}",
            messages.format(
                "dat_file_line",
                "{status}  {path}  {game}/{name}",
                &[
                    ("status", status),
                    ("path", entry.entry_path.clone()),
                    ("game", entry.game.clone().unwrap_or_default()),
                    ("name", entry.dat_name.clone().unwrap_or_default()),
                ],
            )
        );
    }
    for rom in report
        .roms
        .iter()
        .filter(|rom| rom.status == DatRomStatus::Missing)
    {
        println!(
            "{}",
            messages.format(
                "dat_missing_line",
                "missing  {game}/{name}",
                &[("game", rom.game.clone()), ("name", rom.name.clone()),],
            )
        );
    }
    for rom in report
        .roms
        .iter()
        .filter(|rom| rom.status == DatRomStatus::NoDump)
    {
        println!(
            "{}",
            messages.format(
                "dat_nodump_line",
                "nodump  {game}/{name}",
                &[("game", rom.game.clone()), ("name", rom.name.clone())],
            )
        );
    }
    Ok(())
}

fn marked_file_status(
    messages: &Messages,
    status: FileStatus,
    dump_status: Option<DumpStatus>,
) -> String {
    let status = file_status_text(messages, status);
    if dump_status == Some(DumpStatus::BadDump) {
        messages.format(
            "status_baddump",
            "{status} (baddump)",
            &[("status", status.to_owned())],
        )
    } else {
        status.to_owned()
    }
}

fn file_status_text(messages: &Messages, status: FileStatus) -> &str {
    match status {
        FileStatus::Have => messages.text("status_have", "Have"),
        FileStatus::WrongName => messages.text("status_wrong_name", "WrongName"),
        FileStatus::WrongDump => messages.text("status_wrong_dump", "WrongDump"),
        FileStatus::Duplicate => messages.text("status_duplicate", "Duplicate"),
        FileStatus::Extra => messages.text("status_extra", "Extra"),
    }
}

fn main() {
    if let Err(error) = run() {
        let messages = Messages::load("en").expect("embedded English locale must be valid");
        eprintln!("{}: {error:#}", messages.text("error", "Error"));
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_beside_executable() {
        let messages = Messages::load("en").unwrap();
        let path = default_config_path(&messages).unwrap();
        assert_eq!(path.file_name().unwrap(), "yaRomChecker.yaml");
        assert_eq!(path.parent(), std::env::current_exe().unwrap().parent());
    }

    #[test]
    fn selected_locale_falls_back_to_english_for_missing_keys() {
        let messages = Messages::load("ja").unwrap();
        assert_eq!(messages.text("scan_complete", "fallback"), "Scan complete");
    }

    #[test]
    fn formats_scan_summary_from_locale_resource() {
        let messages = Messages::load("en").unwrap();
        assert_eq!(
            messages.format(
                "scan_summary",
                "fallback",
                &[
                    ("entries", "3".to_owned()),
                    ("hashed", "2".to_owned()),
                    ("reused", "1".to_owned()),
                ],
            ),
            "Scanned 3 entries (2 hashed, 1 reused containers)."
        );
    }

    #[test]
    fn creates_missing_config_and_does_not_overwrite_existing() {
        let messages = Messages::load("en").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        let path = nested.join("yaRomChecker.yaml");

        assert!(ensure_config(&path, &messages).unwrap());
        let created = fs::read_to_string(&path).unwrap();
        assert!(created.contains("locale: en"));
        assert!(created.contains("cache_path:"));

        fs::write(&path, "locale: en\ncache_path: keep-me.sqlite3\n").unwrap();
        assert!(!ensure_config(&path, &messages).unwrap());
        let kept = fs::read_to_string(&path).unwrap();
        assert!(kept.contains("keep-me.sqlite3"));
    }

    #[test]
    fn provides_english_text_for_every_file_status() {
        let messages = Messages::load("en").unwrap();
        assert_eq!(file_status_text(&messages, FileStatus::Have), "Have");
        assert_eq!(
            file_status_text(&messages, FileStatus::WrongName),
            "WrongName"
        );
        assert_eq!(
            file_status_text(&messages, FileStatus::WrongDump),
            "WrongDump"
        );
        assert_eq!(
            file_status_text(&messages, FileStatus::Duplicate),
            "Duplicate"
        );
        assert_eq!(file_status_text(&messages, FileStatus::Extra), "Extra");
        assert_eq!(
            marked_file_status(&messages, FileStatus::Have, Some(DumpStatus::BadDump)),
            "Have (baddump)"
        );
    }
}
