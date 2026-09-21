use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread,
};

use anyhow::{Context, Result};
use clap::Parser;
use eframe::egui::{self, RichText, ScrollArea, TextEdit};
use egui_extras::{Column, TableBuilder};
use serde::{Deserialize, Serialize};
use yaromchecker_core::{
    DatFile, DatRomStatus, EntryKind, MatchReport, ScanCache, ScanEntry, ScanMode, ScanReport,
    Scanner, SetStatus, match_collection,
};
use yaromchecker_gui::{set_count_text, should_show_member_pane, source_title};

const DEFAULT_CONFIG: &str = include_str!("../../../yaRomChecker.example.yaml");
const EN_MESSAGES: &str = include_str!("../../../locales/en.yaml");
const JA_MESSAGES: &str = include_str!("../../../locales/ja.yaml");

#[derive(Parser)]
#[command(
    name = "yaRomChecker",
    version,
    about = "Verify locally owned ROM files"
)]
struct Args {
    /// YAML configuration file (defaults to yaRomChecker.yaml beside the executable)
    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default = "default_locale")]
    locale: String,
    cache_path: PathBuf,
    #[serde(default)]
    sources: Vec<Source>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Source {
    dat: PathBuf,
    collection: PathBuf,
}

fn default_locale() -> String {
    "en".to_owned()
}

#[derive(Clone)]
struct Messages {
    fallback: HashMap<String, String>,
    selected: HashMap<String, String>,
}

impl Messages {
    fn load(locale: &str) -> Result<Self> {
        let fallback: HashMap<String, String> = serde_yml::from_str(EN_MESSAGES)?;
        let selected = match locale {
            "en" => fallback.clone(),
            "ja" => serde_yml::from_str(JA_MESSAGES)?,
            _ => fallback.clone(),
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Surface {
    Main,
    Settings,
    Report,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScanChoice {
    Initial,
    Quick,
    Full,
}

#[derive(Default, Clone)]
struct ScanSummary {
    entries: usize,
    hashed: usize,
    reused: usize,
    errors: Vec<String>,
}

impl ScanSummary {
    fn line(&self) -> String {
        format!(
            "Entries: {} | Containers hashed: {} | Containers reused: {} | Errors: {}",
            self.entries,
            self.hashed,
            self.reused,
            self.errors.len()
        )
    }
}

#[derive(Default, Clone)]
struct DatSummary {
    files: usize,
    present: usize,
    missing: usize,
    nodump: usize,
}

impl DatSummary {
    fn line(&self) -> String {
        format!(
            "Files: {} | Present: {} | Missing: {} | nodump: {}",
            self.files, self.present, self.missing, self.nodump
        )
    }
}

enum WorkerMessage {
    Started(PathBuf),
    Finished(PathBuf, std::result::Result<ScanReport, String>),
    Done,
}

struct VerifiedSource {
    report: MatchReport,
    entries: Vec<ScanEntry>,
}

struct App {
    config_path: PathBuf,
    config: Config,
    settings: Config,
    messages: Messages,
    surface: Surface,
    selected_source: Option<usize>,
    selected_set: Option<String>,
    dats: Vec<std::result::Result<DatFile, String>>,
    verified: HashMap<usize, VerifiedSource>,
    scan_popup: bool,
    scan_choice: ScanChoice,
    scan_receiver: Option<Receiver<WorkerMessage>>,
    scan_running: bool,
    current_container: Option<PathBuf>,
    scan_summary: Option<ScanSummary>,
    dat_summary: Option<DatSummary>,
    notice: Option<String>,
}

impl App {
    fn load(config_path: PathBuf) -> Result<Self> {
        ensure_config(&config_path)?;
        let config: Config = serde_yml::from_str(
            &fs::read_to_string(&config_path)
                .with_context(|| format!("Cannot read configuration {}", config_path.display()))?,
        )
        .with_context(|| format!("Invalid configuration {}", config_path.display()))?;
        let messages = Messages::load(&config.locale)?;
        let mut app = Self {
            config_path,
            settings: config.clone(),
            config,
            messages,
            surface: Surface::Main,
            selected_source: None,
            selected_set: None,
            dats: Vec::new(),
            verified: HashMap::new(),
            scan_popup: false,
            scan_choice: ScanChoice::Initial,
            scan_receiver: None,
            scan_running: false,
            current_container: None,
            scan_summary: None,
            dat_summary: None,
            notice: None,
        };
        app.reload_dats();
        Ok(app)
    }

    fn reload_dats(&mut self) {
        self.dats = self
            .config
            .sources
            .iter()
            .map(|source| {
                DatFile::load(&resolve_path(&self.config_path, &source.dat))
                    .map_err(|error| error.to_string())
            })
            .collect();
    }

    fn poll_scan(&mut self) {
        let mut messages = Vec::new();
        if let Some(receiver) = &self.scan_receiver {
            while let Ok(message) = receiver.try_recv() {
                messages.push(message);
            }
        }
        for message in messages {
            match message {
                WorkerMessage::Started(path) => self.current_container = Some(path),
                WorkerMessage::Finished(path, result) => match result {
                    Ok(report) => {
                        let summary = self.scan_summary.get_or_insert_with(ScanSummary::default);
                        summary.entries += report.entries.len();
                        summary.hashed += report.hashed_containers;
                        summary.reused += report.reused_containers;
                    }
                    Err(error) => self
                        .scan_summary
                        .get_or_insert_with(ScanSummary::default)
                        .errors
                        .push(format!("{}: {error}", path.display())),
                },
                WorkerMessage::Done => {
                    self.scan_running = false;
                    self.current_container = None;
                    self.scan_receiver = None;
                    self.verified.clear();
                }
            }
        }
    }

    fn start_scan(&mut self) {
        if self.scan_running {
            return;
        }
        let mut collections = BTreeSet::new();
        for source in &self.config.sources {
            collections.insert(resolve_path(&self.config_path, &source.collection));
        }
        let cache_path = resolve_path(&self.config_path, &self.config.cache_path);
        let mode = match self.scan_choice {
            ScanChoice::Quick => ScanMode::Quick,
            ScanChoice::Initial | ScanChoice::Full => ScanMode::Full,
        };
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let cache = match ScanCache::open(&cache_path) {
                Ok(cache) => cache,
                Err(error) => {
                    let _ =
                        sender.send(WorkerMessage::Finished(cache_path, Err(error.to_string())));
                    let _ = sender.send(WorkerMessage::Done);
                    return;
                }
            };
            let mut scanner = Scanner::new(cache);
            for collection in collections {
                let result = scanner
                    .scan_with_progress(&collection, mode, |container| {
                        let _ = sender.send(WorkerMessage::Started(container.to_owned()));
                    })
                    .map_err(|error| error.to_string());
                let _ = sender.send(WorkerMessage::Finished(collection, result));
            }
            let _ = sender.send(WorkerMessage::Done);
        });
        self.scan_summary = Some(ScanSummary::default());
        self.scan_receiver = Some(receiver);
        self.scan_running = true;
    }

    fn verify(&mut self, all: bool) {
        let indexes: Vec<usize> = if all {
            (0..self.config.sources.len()).collect()
        } else if let Some(index) = self.selected_source {
            vec![index]
        } else {
            self.notice = Some("Select a source before verifying it.".to_owned());
            return;
        };
        let cache_path = resolve_path(&self.config_path, &self.config.cache_path);
        let cache = match ScanCache::open(&cache_path) {
            Ok(cache) => cache,
            Err(error) => {
                self.notice = Some(error.to_string());
                return;
            }
        };
        let mut cached_collections: HashMap<PathBuf, Vec<ScanEntry>> = HashMap::new();
        let mut total = DatSummary::default();
        for index in indexes {
            let source = &self.config.sources[index];
            let collection = resolve_path(&self.config_path, &source.collection);
            let canonical = match collection.canonicalize() {
                Ok(path) => path,
                Err(error) => {
                    self.notice = Some(format!("Cannot resolve {}: {error}", collection.display()));
                    continue;
                }
            };
            if !cached_collections.contains_key(&canonical) {
                match cache.entries_for_collection(&canonical) {
                    Ok(entries) => {
                        cached_collections.insert(canonical.clone(), entries);
                    }
                    Err(error) => {
                        self.notice = Some(error.to_string());
                        continue;
                    }
                }
            }
            let Ok(dat) = &self.dats[index] else {
                continue;
            };
            let entries = cached_collections[&canonical].clone();
            if entries.is_empty() {
                self.notice = Some(format!(
                    "No cached scan entries were found for {}. Scan this collection first.",
                    collection.display()
                ));
                continue;
            }
            let report = match_collection(&entries, dat);
            total.files += report.files.len();
            total.present += report
                .roms
                .iter()
                .filter(|rom| rom.status == DatRomStatus::Present)
                .count();
            total.missing += report
                .roms
                .iter()
                .filter(|rom| {
                    matches!(
                        rom.status,
                        DatRomStatus::Missing | DatRomStatus::MissingInArchive
                    )
                })
                .count();
            total.nodump += report
                .roms
                .iter()
                .filter(|rom| rom.status == DatRomStatus::NoDump)
                .count();
            self.verified
                .insert(index, VerifiedSource { report, entries });
        }
        self.dat_summary = Some(total);
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            if ui.button("Scan").clicked() {
                self.scan_popup = true;
            }
            ui.menu_button("Verify", |ui| {
                if ui
                    .add_enabled(
                        self.selected_source.is_some(),
                        egui::Button::new("Selected source"),
                    )
                    .clicked()
                {
                    self.verify(false);
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        !self.config.sources.is_empty(),
                        egui::Button::new("All sources"),
                    )
                    .clicked()
                {
                    self.verify(true);
                    ui.close_menu();
                }
            });
            if ui.button("Settings").clicked() {
                self.settings = self.config.clone();
                self.surface = Surface::Settings;
            }
            if ui.button("Report").clicked() {
                self.surface = Surface::Report;
            }
            ui.add_enabled(false, egui::Button::new("Organize"));
        });
    }

    fn sources_pane(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sources (DAT + collection)");
        if self.config.sources.is_empty() {
            ui.label(self.messages.text(
                "dat_missing_config",
                "No DAT collection sources configured. Add DAT and collection pairs under sources: in the YAML config.",
            ));
            if ui.button("Open Settings").clicked() {
                self.surface = Surface::Settings;
            }
            return;
        }
        ScrollArea::vertical().show(ui, |ui| {
            for (index, source) in self.config.sources.clone().into_iter().enumerate() {
                let name = self
                    .dats
                    .get(index)
                    .and_then(|dat| dat.as_ref().ok())
                    .and_then(|dat| dat.header.name.as_deref())
                    .map(str::to_owned)
                    .unwrap_or_else(|| source.dat.display().to_string());
                let total = self
                    .dats
                    .get(index)
                    .and_then(|dat| dat.as_ref().ok())
                    .map(|dat| dat.roms.len());
                let found = self.verified.get(&index).map(|verified| {
                    verified
                        .report
                        .roms
                        .iter()
                        .filter(|rom| rom.status == DatRomStatus::Present)
                        .count()
                });
                let title = source_title(&name, total, found);
                // A simple collapsing egui tree is used because egui_ltreeview could not be
                // resolved in the locked/offline dependency environment for Rust 1.93.
                egui::CollapsingHeader::new(title)
                    .default_open(true)
                    .show(ui, |ui| {
                        let selected = self.selected_source == Some(index);
                        if ui
                            .selectable_label(selected, source.collection.display().to_string())
                            .clicked()
                        {
                            self.selected_source = Some(index);
                            self.selected_set = None;
                        }
                        if let Some(Err(error)) = self.dats.get(index) {
                            ui.colored_label(ui.visuals().error_fg_color, error);
                        }
                    });
            }
        });
    }

    fn set_rows(&self) -> Vec<(String, Option<SetStatus>, String, String, String, String)> {
        let Some(index) = self.selected_source else {
            return Vec::new();
        };
        let Ok(dat) = &self.dats[index] else {
            return Vec::new();
        };
        let mut names = Vec::new();
        let mut seen = HashSet::new();
        for rom in &dat.roms {
            let has_required_rom = dat.roms.iter().any(|candidate| {
                candidate.game == rom.game
                    && candidate.dump_status != yaromchecker_core::DumpStatus::NoDump
            });
            if has_required_rom && seen.insert(rom.game.clone()) {
                names.push(rom.game.clone());
            }
        }
        names
            .into_iter()
            .map(|game| {
                if let Some(verified) = self.verified.get(&index) {
                    let status = verified
                        .report
                        .sets
                        .iter()
                        .find(|set| set.game == game)
                        .map(|set| set.status);
                    let count = |wanted| {
                        verified
                            .report
                            .roms
                            .iter()
                            .filter(|rom| rom.game == game && rom.status == wanted)
                            .count()
                    };
                    let present = count(DatRomStatus::Present);
                    let missing = count(DatRomStatus::Missing);
                    let missing_in_archive = count(DatRomStatus::MissingInArchive);
                    let nodump = count(DatRomStatus::NoDump);
                    (
                        game,
                        status,
                        set_count_text(true, present),
                        set_count_text(true, missing),
                        set_count_text(true, missing_in_archive),
                        set_count_text(true, nodump),
                    )
                } else {
                    (
                        game,
                        None,
                        set_count_text(false, 0),
                        set_count_text(false, 0),
                        set_count_text(false, 0),
                        set_count_text(false, 0),
                    )
                }
            })
            .collect()
    }

    fn sets_pane(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sets");
        if self.selected_source.is_none() {
            ui.label("Select a DAT in the tree to list its sets.");
            return;
        }
        let rows = self.set_rows();
        if !self.verified.contains_key(&self.selected_source.unwrap()) {
            ui.label("Run Verify to see statuses.");
        }
        TableBuilder::new(ui)
            .striped(true)
            .column(Column::remainder().at_least(140.0))
            .column(Column::auto())
            .columns(Column::auto(), 4)
            .header(22.0, |mut header| {
                for label in [
                    "Name",
                    "Status",
                    "Present",
                    "Missing",
                    "MissingInArchive",
                    "nodump",
                ] {
                    header.col(|ui| {
                        ui.strong(label);
                    });
                }
            })
            .body(|mut body| {
                for (game, status, present, missing, missing_archive, nodump) in rows {
                    body.row(20.0, |mut row| {
                        row.col(|ui| {
                            if ui
                                .selectable_label(
                                    self.selected_set.as_deref() == Some(&game),
                                    &game,
                                )
                                .clicked()
                            {
                                self.selected_set = Some(game.clone());
                            }
                        });
                        row.col(|ui| {
                            ui.label(status.map(set_status).unwrap_or("Not verified"));
                        });
                        for value in [present, missing, missing_archive, nodump] {
                            row.col(|ui| {
                                ui.label(value);
                            });
                        }
                    });
                }
            });
    }

    fn selected_members(&self) -> Vec<&ScanEntry> {
        let (Some(index), Some(game)) = (self.selected_source, self.selected_set.as_deref()) else {
            return Vec::new();
        };
        let Some(verified) = self.verified.get(&index) else {
            return Vec::new();
        };
        let paths: HashSet<&str> = verified
            .report
            .files
            .iter()
            .filter(|file| file.game.as_deref() == Some(game))
            .map(|file| file.entry_path.as_str())
            .collect();
        verified
            .entries
            .iter()
            .filter(|entry| paths.contains(entry.entry_path.as_str()))
            .collect()
    }

    fn members_pane(&self, ui: &mut egui::Ui) {
        ui.heading("Members");
        let members = self.selected_members();
        let has_archive = members.iter().any(|entry| entry.kind != EntryKind::File);
        let dat_member_count = self
            .selected_source
            .and_then(|index| self.dats.get(index))
            .and_then(|dat| dat.as_ref().ok())
            .zip(self.selected_set.as_deref())
            .map(|(dat, game)| dat.roms.iter().filter(|rom| rom.game == game).count())
            .unwrap_or(0);
        if !should_show_member_pane(members.len().max(dat_member_count), has_archive) {
            ui.label("Select a multi-file or archive set to list members.");
            return;
        }
        TableBuilder::new(ui)
            .striped(true)
            .column(Column::remainder().at_least(100.0))
            .columns(Column::auto(), 6)
            .header(22.0, |mut header| {
                for label in ["Name", "Size", "mtime", "CRC32", "MD5", "SHA1", "Checked"] {
                    header.col(|ui| {
                        ui.strong(label);
                    });
                }
            })
            .body(|mut body| {
                for entry in members {
                    body.row(20.0, |mut row| {
                        for value in [
                            entry.entry_name.clone(),
                            entry.entry_size.to_string(),
                            format_timestamp(entry.container_mtime_ns),
                            entry.hashes.crc32.clone(),
                            entry.hashes.md5.clone(),
                            entry.hashes.sha1.clone(),
                            format_timestamp(entry.last_hashed_ns),
                        ] {
                            row.col(|ui| {
                                ui.label(value);
                            });
                        }
                    });
                }
            });
    }

    fn main_surface(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let left = (available.x * 0.34).max(260.0);
        let top = available.y * 0.55;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(left);
                ui.allocate_ui(egui::vec2(left, top), |ui| self.sources_pane(ui));
                ui.separator();
                ui.heading("External media (later)");
                ui.label("Health check and copy from media are planned. Writing to external media is not supported.");
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.allocate_ui(egui::vec2(ui.available_width(), top), |ui| self.sets_pane(ui));
                ui.separator();
                self.members_pane(ui);
            });
        });
    }

    fn settings_surface(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Settings");
            if ui.button("Back").clicked() {
                self.surface = Surface::Main;
            }
        });
        ui.label("Configuration file (read-only):");
        let mut config_display = self.config_path.display().to_string();
        ui.add(TextEdit::singleline(&mut config_display).interactive(false));
        egui::ComboBox::from_label("Locale")
            .selected_text(&self.settings.locale)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.locale, "en".to_owned(), "en");
                ui.selectable_value(&mut self.settings.locale, "ja".to_owned(), "ja");
            });
        path_row(
            ui,
            "Cache path",
            &mut self.settings.cache_path,
            PathPicker::SaveFile,
        );
        ui.separator();
        ui.heading("DAT and collection sources");
        let mut remove = None;
        for (index, source) in self.settings.sources.iter_mut().enumerate() {
            ui.group(|ui| {
                path_row(ui, "DAT", &mut source.dat, PathPicker::File);
                path_row(ui, "Collection", &mut source.collection, PathPicker::Folder);
                if ui.button("Remove").clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            self.settings.sources.remove(index);
        }
        if ui.button("Add source").clicked() {
            self.settings.sources.push(Source {
                dat: PathBuf::new(),
                collection: PathBuf::new(),
            });
        }
        if ui.button("Save").clicked() {
            match serde_yml::to_string(&self.settings)
                .map_err(anyhow::Error::from)
                .and_then(|yaml| fs::write(&self.config_path, yaml).map_err(anyhow::Error::from))
            {
                Ok(()) => {
                    self.config = self.settings.clone();
                    self.messages = Messages::load(&self.config.locale)
                        .unwrap_or_else(|_| self.messages.clone());
                    self.selected_source = None;
                    self.selected_set = None;
                    self.verified.clear();
                    self.reload_dats();
                    self.notice = Some("Settings saved.".to_owned());
                    self.surface = Surface::Main;
                }
                Err(error) => self.notice = Some(format!("Cannot save settings: {error}")),
            }
        }
    }

    fn report_surface(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Report");
            if ui.button("Back").clicked() {
                self.surface = Surface::Main;
            }
        });
        let scan = self.scan_summary.as_ref().map(ScanSummary::line);
        let dat = self.dat_summary.as_ref().map(DatSummary::line);
        if scan.is_none() && dat.is_none() {
            ui.label("No report is available. Run a scan or verification first.");
        }
        ui.label(RichText::new("Last scan summary (read-only)").strong());
        ui.label(scan.as_deref().unwrap_or("No scan yet"));
        ui.label(RichText::new("Last DAT summary (read-only)").strong());
        ui.label(dat.as_deref().unwrap_or("No verification yet"));
        if ui.button("Copy summaries").clicked() {
            ui.ctx().copy_text(format!(
                "{}\n{}",
                scan.unwrap_or_else(|| "No scan yet".into()),
                dat.unwrap_or_else(|| "No verification yet".into())
            ));
        }
        ui.add_enabled(false, egui::Button::new("Export - later"));
        ui.add_enabled(false, egui::Button::new("Organize preview - later"));
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let collection = self
                .selected_source
                .and_then(|index| self.config.sources.get(index))
                .map(|source| source.collection.display().to_string())
                .unwrap_or_else(|| "No source selected".to_owned());
            ui.label(format!("Collection: {collection}"));
            ui.separator();
            ui.label(
                self.scan_summary
                    .as_ref()
                    .map(ScanSummary::line)
                    .unwrap_or_else(|| "No scan yet".into()),
            );
            ui.separator();
            ui.label(format!("Locale: {}", self.config.locale));
            ui.separator();
            ui.label(format!(
                "Config: {} (read-only)",
                self.config_path.display()
            ));
        });
    }

    fn scan_window(&mut self, ctx: &egui::Context) {
        if !self.scan_popup {
            return;
        }
        egui::Modal::new(egui::Id::new("scan_modal")).show(ctx, |ui| {
            ui.heading("Scan");
            ui.label("Sources: unique collections from YAML");
            ui.radio_value(
                &mut self.scan_choice,
                ScanChoice::Initial,
                "Initial/full scan",
            );
            ui.radio_value(&mut self.scan_choice, ScanChoice::Quick, "Quick rescan");
            ui.radio_value(&mut self.scan_choice, ScanChoice::Full, "Full rescan");
            if ui
                .add_enabled(!self.scan_running, egui::Button::new("Start"))
                .clicked()
            {
                self.start_scan();
            }
            if self.scan_running {
                ui.spinner();
            }
            ui.label(format!(
                "Current container: {}",
                self.current_container
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "None".into())
            ));
            if let Some(summary) = &self.scan_summary {
                ui.label(summary.line());
                for error in &summary.errors {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            } else {
                ui.label("Choose a scan mode. Start hashes configured source collections.");
            }
            if ui
                .add_enabled(!self.scan_running, egui::Button::new("Close"))
                .clicked()
            {
                self.scan_popup = false;
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_scan();
        if self.scan_running {
            ctx.request_repaint();
        }
        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu(ui));
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.status_bar(ui));
        egui::CentralPanel::default().show(ctx, |ui| match self.surface {
            Surface::Main => self.main_surface(ui),
            Surface::Settings => self.settings_surface(ui),
            Surface::Report => self.report_surface(ui),
        });
        if let Some(notice) = self.notice.clone() {
            egui::Window::new("Notice")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(notice);
                    if ui.button("OK").clicked() {
                        self.notice = None;
                    }
                });
        }
        self.scan_window(ctx);
    }
}

#[derive(Clone, Copy)]
enum PathPicker {
    File,
    Folder,
    SaveFile,
}

fn path_row(ui: &mut egui::Ui, label: &str, path: &mut PathBuf, picker: PathPicker) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut value = path.display().to_string();
        if ui.text_edit_singleline(&mut value).changed() {
            *path = value.into();
        }
        if ui.button("Browse...").clicked() {
            let dialog = rfd::FileDialog::new();
            let selected = match picker {
                PathPicker::File => dialog.pick_file(),
                PathPicker::Folder => dialog.pick_folder(),
                PathPicker::SaveFile => dialog.save_file(),
            };
            if let Some(selected) = selected {
                *path = selected;
            }
        }
    });
}

fn set_status(status: SetStatus) -> &'static str {
    match status {
        SetStatus::Complete => "Complete",
        SetStatus::Incomplete => "Incomplete",
        SetStatus::MissingSet => "MissingSet",
    }
}

fn format_timestamp(timestamp_ns: i64) -> String {
    if timestamp_ns <= 0 {
        return "Unknown".to_owned();
    }
    let total_seconds = timestamp_ns / 1_000_000_000;
    let days = total_seconds.div_euclid(86_400);
    let seconds = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        seconds / 3_600,
        seconds % 3_600 / 60,
        seconds % 60
    )
}

// Gregorian conversion based on the era/day decomposition used by civil calendars.
fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let days = days_since_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

fn resolve_path(config_path: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

fn default_config_path() -> Result<PathBuf> {
    let executable = std::env::current_exe().context("Cannot determine executable path")?;
    Ok(executable
        .parent()
        .context("Executable has no parent directory")?
        .join("yaRomChecker.yaml"))
}

fn ensure_config(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, DEFAULT_CONFIG)?;
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config_path = args.config.map(Ok).unwrap_or_else(default_config_path)?;
    let app = App::load(config_path)?;
    eframe::run_native(
        "yaRomChecker",
        eframe::NativeOptions::default(),
        Box::new(|_creation_context| Ok(Box::new(app))),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}
