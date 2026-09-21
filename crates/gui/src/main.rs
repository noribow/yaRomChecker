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
use yaromchecker_gui::{dat_member_rows, set_count_text, should_show_member_pane, source_title};

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

    fn text<'a>(&'a self, key: &str) -> &'a str {
        self.selected
            .get(key)
            .or_else(|| self.fallback.get(key))
            .map(String::as_str)
            .unwrap_or_else(|| panic!("missing English locale key: {key}"))
    }

    fn format(&self, key: &str, values: &[(&str, String)]) -> String {
        let mut text = self.text(key).to_owned();
        for (name, value) in values {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
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
    fn line(&self, messages: &Messages) -> String {
        messages.format(
            "gui_scan_summary",
            &[
                ("entries", self.entries.to_string()),
                ("hashed", self.hashed.to_string()),
                ("reused", self.reused.to_string()),
                ("errors", self.errors.len().to_string()),
            ],
        )
    }
}

#[derive(Default, Clone)]
struct DatSummary {
    files: usize,
    present: usize,
    missing: usize,
    missing_in_archive: usize,
    nodump: usize,
}

impl DatSummary {
    fn line(&self, messages: &Messages) -> String {
        messages.format(
            "gui_dat_summary",
            &[
                ("files", self.files.to_string()),
                ("present", self.present.to_string()),
                ("missing", self.missing.to_string()),
                ("missing_in_archive", self.missing_in_archive.to_string()),
                ("nodump", self.nodump.to_string()),
            ],
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

struct MemberRow {
    name: String,
    entry: Option<ScanEntry>,
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
            self.notice = Some(self.messages.text("gui_notice_select_source").to_owned());
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
                    self.notice = Some(self.messages.format(
                        "gui_notice_resolve",
                        &[
                            ("path", collection.display().to_string()),
                            ("error", error.to_string()),
                        ],
                    ));
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
                self.notice = Some(self.messages.format(
                    "gui_notice_no_cache",
                    &[("path", collection.display().to_string())],
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
                .filter(|rom| rom.status == DatRomStatus::Missing)
                .count();
            total.missing_in_archive += report
                .roms
                .iter()
                .filter(|rom| rom.status == DatRomStatus::MissingInArchive)
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
        let verify_label = self.messages.text("gui_menu_verify").to_owned();
        egui::menu::bar(ui, |ui| {
            ui.menu_button(self.messages.text("gui_menu_file"), |ui| {
                if ui.button(self.messages.text("gui_menu_exit")).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            if ui.button(self.messages.text("gui_menu_scan")).clicked() {
                self.scan_popup = true;
            }
            ui.menu_button(verify_label, |ui| {
                if ui
                    .add_enabled(
                        self.selected_source.is_some(),
                        egui::Button::new(self.messages.text("gui_verify_selected")),
                    )
                    .clicked()
                {
                    self.verify(false);
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        !self.config.sources.is_empty(),
                        egui::Button::new(self.messages.text("gui_verify_all")),
                    )
                    .clicked()
                {
                    self.verify(true);
                    ui.close_menu();
                }
            });
            if ui.button(self.messages.text("gui_menu_settings")).clicked() {
                self.settings = self.config.clone();
                self.surface = Surface::Settings;
            }
            if ui.button(self.messages.text("gui_menu_report")).clicked() {
                self.surface = Surface::Report;
            }
            ui.add_enabled(
                false,
                egui::Button::new(self.messages.text("gui_menu_organize")),
            );
        });
    }

    fn sources_pane(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.messages.text("gui_sources"));
        if self.config.sources.is_empty() {
            ui.label(self.messages.text("dat_missing_config"));
            if ui.button(self.messages.text("gui_open_settings")).clicked() {
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
        ui.heading(self.messages.text("gui_sets"));
        if self.selected_source.is_none() {
            ui.label(self.messages.text("gui_select_dat"));
            return;
        }
        let rows = self.set_rows();
        if !self.verified.contains_key(&self.selected_source.unwrap()) {
            ui.label(self.messages.text("gui_run_verify"));
        }
        TableBuilder::new(ui)
            .striped(true)
            .column(Column::remainder().at_least(140.0))
            .column(Column::auto())
            .columns(Column::auto(), 4)
            .header(22.0, |mut header| {
                for label in [
                    self.messages.text("gui_column_name"),
                    self.messages.text("gui_column_status"),
                    self.messages.text("gui_status_present"),
                    self.messages.text("gui_status_missing"),
                    self.messages.text("gui_status_missing_in_archive"),
                    self.messages.text("gui_status_nodump"),
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
                            ui.label(
                                status
                                    .map(|value| set_status(&self.messages, value))
                                    .unwrap_or_else(|| self.messages.text("gui_not_verified")),
                            );
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

    fn selected_members(&self) -> Vec<MemberRow> {
        let (Some(index), Some(game)) = (self.selected_source, self.selected_set.as_deref()) else {
            return Vec::new();
        };
        let Ok(dat) = &self.dats[index] else {
            return Vec::new();
        };
        let names: Vec<String> = dat
            .roms
            .iter()
            .filter(|rom| rom.game == game)
            .map(|rom| rom.name.clone())
            .collect();
        let Some(verified) = self.verified.get(&index) else {
            return names
                .into_iter()
                .map(|name| MemberRow { name, entry: None })
                .collect();
        };
        let hits: Vec<(String, String)> = verified
            .report
            .files
            .iter()
            .filter(|file| file.game.as_deref() == Some(game))
            .filter_map(|file| {
                file.dat_name
                    .as_ref()
                    .map(|name| (name.clone(), file.entry_path.clone()))
            })
            .collect();
        dat_member_rows(&names, &hits)
            .into_iter()
            .map(|(name, hit)| MemberRow {
                name,
                entry: hit.and_then(|hit| {
                    verified
                        .entries
                        .iter()
                        .find(|entry| entry.entry_path == hits[hit].1)
                        .cloned()
                }),
            })
            .collect()
    }

    fn members_pane(&self, ui: &mut egui::Ui) {
        ui.heading(self.messages.text("gui_members"));
        let members = self.selected_members();
        let has_archive = members.iter().any(|row| {
            row.entry
                .as_ref()
                .is_some_and(|entry| entry.kind != EntryKind::File)
        });
        if !should_show_member_pane(members.len(), has_archive) {
            ui.label(self.messages.text("gui_select_members"));
            return;
        }
        TableBuilder::new(ui)
            .striped(true)
            .column(Column::remainder().at_least(100.0))
            .columns(Column::auto(), 6)
            .header(22.0, |mut header| {
                for label in [
                    self.messages.text("gui_column_name"),
                    self.messages.text("gui_column_size"),
                    self.messages.text("gui_column_mtime"),
                    self.messages.text("gui_column_crc32"),
                    self.messages.text("gui_column_md5"),
                    self.messages.text("gui_column_sha1"),
                    self.messages.text("gui_column_checked"),
                ] {
                    header.col(|ui| {
                        ui.strong(label);
                    });
                }
            })
            .body(|mut body| {
                for member in members {
                    body.row(20.0, |mut row| {
                        let values = if let Some(entry) = member.entry {
                            [
                                member.name,
                                entry.entry_size.to_string(),
                                format_timestamp(
                                    entry.container_mtime_ns,
                                    self.messages.text("gui_unknown"),
                                ),
                                entry.hashes.crc32,
                                entry.hashes.md5,
                                entry.hashes.sha1,
                                format_timestamp(
                                    entry.last_hashed_ns,
                                    self.messages.text("gui_unknown"),
                                ),
                            ]
                        } else {
                            [
                                member.name,
                                String::new(),
                                String::new(),
                                String::new(),
                                String::new(),
                                String::new(),
                                String::new(),
                            ]
                        };
                        for value in values {
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
                ui.heading(self.messages.text("gui_external_media"));
                ui.label(self.messages.text("gui_external_media_help"));
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.allocate_ui(egui::vec2(ui.available_width(), top), |ui| {
                    self.sets_pane(ui)
                });
                ui.separator();
                self.members_pane(ui);
            });
        });
    }

    fn settings_surface(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(self.messages.text("gui_menu_settings"));
            if ui.button(self.messages.text("gui_back")).clicked() {
                self.surface = Surface::Main;
            }
        });
        ui.label(self.messages.text("gui_config_read_only"));
        let mut config_display = self.config_path.display().to_string();
        ui.add(TextEdit::singleline(&mut config_display).interactive(false));
        egui::ComboBox::from_label(self.messages.text("gui_locale"))
            .selected_text(&self.settings.locale)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.locale, "en".to_owned(), "en");
                ui.selectable_value(&mut self.settings.locale, "ja".to_owned(), "ja");
            });
        path_row(
            ui,
            self.messages.text("gui_cache_path"),
            &mut self.settings.cache_path,
            PathPicker::SaveFile,
            self.messages.text("gui_browse"),
        );
        ui.separator();
        ui.heading(self.messages.text("gui_sources_settings"));
        let mut remove = None;
        for (index, source) in self.settings.sources.iter_mut().enumerate() {
            ui.group(|ui| {
                path_row(
                    ui,
                    self.messages.text("gui_dat"),
                    &mut source.dat,
                    PathPicker::File,
                    self.messages.text("gui_browse"),
                );
                path_row(
                    ui,
                    self.messages.text("gui_collection"),
                    &mut source.collection,
                    PathPicker::Folder,
                    self.messages.text("gui_browse"),
                );
                if ui.button(self.messages.text("gui_remove")).clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            self.settings.sources.remove(index);
        }
        if ui.button(self.messages.text("gui_add_source")).clicked() {
            self.settings.sources.push(Source {
                dat: PathBuf::new(),
                collection: PathBuf::new(),
            });
        }
        if ui.button(self.messages.text("gui_save")).clicked() {
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
                    self.notice = Some(self.messages.text("gui_notice_saved").to_owned());
                    self.surface = Surface::Main;
                }
                Err(error) => {
                    self.notice = Some(
                        self.messages
                            .format("gui_notice_save_error", &[("error", error.to_string())]),
                    )
                }
            }
        }
    }

    fn report_surface(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(self.messages.text("gui_menu_report"));
            if ui.button(self.messages.text("gui_back")).clicked() {
                self.surface = Surface::Main;
            }
        });
        let scan = self
            .scan_summary
            .as_ref()
            .map(|summary| summary.line(&self.messages));
        let dat = self
            .dat_summary
            .as_ref()
            .map(|summary| summary.line(&self.messages));
        if scan.is_none() && dat.is_none() {
            ui.label(self.messages.text("gui_report_empty"));
        }
        ui.label(RichText::new(self.messages.text("gui_last_scan_summary")).strong());
        ui.label(
            scan.as_deref()
                .unwrap_or_else(|| self.messages.text("gui_no_scan")),
        );
        ui.label(RichText::new(self.messages.text("gui_last_dat_summary")).strong());
        ui.label(
            dat.as_deref()
                .unwrap_or_else(|| self.messages.text("gui_no_verification")),
        );
        if ui
            .button(self.messages.text("gui_copy_summaries"))
            .clicked()
        {
            ui.ctx().copy_text(format!(
                "{}\n{}",
                scan.unwrap_or_else(|| self.messages.text("gui_no_scan").into()),
                dat.unwrap_or_else(|| self.messages.text("gui_no_verification").into())
            ));
        }
        ui.add_enabled(
            false,
            egui::Button::new(self.messages.text("gui_export_later")),
        );
        ui.add_enabled(
            false,
            egui::Button::new(self.messages.text("gui_organize_preview_later")),
        );
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let collection = self
                .selected_source
                .and_then(|index| self.config.sources.get(index))
                .map(|source| source.collection.display().to_string())
                .unwrap_or_else(|| self.messages.text("gui_no_source").to_owned());
            ui.label(
                self.messages
                    .format("gui_status_collection", &[("path", collection)]),
            );
            ui.separator();
            ui.label(
                self.scan_summary
                    .as_ref()
                    .map(|summary| summary.line(&self.messages))
                    .unwrap_or_else(|| self.messages.text("gui_no_scan").into()),
            );
            ui.separator();
            ui.label(self.messages.format(
                "gui_status_locale",
                &[("locale", self.config.locale.clone())],
            ));
            ui.separator();
            ui.label(self.messages.format(
                "gui_status_config",
                &[("path", self.config_path.display().to_string())],
            ));
        });
    }

    fn scan_window(&mut self, ctx: &egui::Context) {
        if !self.scan_popup {
            return;
        }
        egui::Modal::new(egui::Id::new("scan_modal")).show(ctx, |ui| {
            ui.heading(self.messages.text("gui_menu_scan"));
            ui.label(self.messages.text("gui_scan_sources"));
            ui.radio_value(
                &mut self.scan_choice,
                ScanChoice::Initial,
                self.messages.text("gui_scan_initial"),
            );
            ui.radio_value(
                &mut self.scan_choice,
                ScanChoice::Quick,
                self.messages.text("gui_scan_quick"),
            );
            ui.radio_value(
                &mut self.scan_choice,
                ScanChoice::Full,
                self.messages.text("gui_scan_full"),
            );
            if ui
                .add_enabled(
                    !self.scan_running,
                    egui::Button::new(self.messages.text("gui_start")),
                )
                .clicked()
            {
                self.start_scan();
            }
            if self.scan_running {
                ui.spinner();
            }
            let current = self
                .current_container
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| self.messages.text("gui_none").into());
            ui.label(
                self.messages
                    .format("gui_current_container", &[("path", current)]),
            );
            if let Some(summary) = &self.scan_summary {
                ui.label(summary.line(&self.messages));
                for error in &summary.errors {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            } else {
                ui.label(self.messages.text("gui_scan_help"));
            }
            if ui
                .add_enabled(
                    !self.scan_running,
                    egui::Button::new(self.messages.text("gui_close")),
                )
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
            egui::Window::new(self.messages.text("gui_notice"))
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(notice);
                    if ui.button(self.messages.text("gui_ok")).clicked() {
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

fn path_row(ui: &mut egui::Ui, label: &str, path: &mut PathBuf, picker: PathPicker, browse: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut value = path.display().to_string();
        if ui.text_edit_singleline(&mut value).changed() {
            *path = value.into();
        }
        if ui.button(browse).clicked() {
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

fn set_status(messages: &Messages, status: SetStatus) -> &str {
    match status {
        SetStatus::Complete => messages.text("set_status_complete"),
        SetStatus::Incomplete => messages.text("set_status_incomplete"),
        SetStatus::MissingSet => messages.text("set_status_missing"),
    }
}

fn format_timestamp(timestamp_ns: i64, unknown: &str) -> String {
    if timestamp_ns <= 0 {
        return unknown.to_owned();
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
    let title = app.messages.text("gui_title").to_owned();
    eframe::run_native(
        &title,
        eframe::NativeOptions::default(),
        Box::new(|_creation_context| Ok(Box::new(app))),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_locale_contains_every_gui_and_status_key_used_by_gui() {
        let english: HashMap<String, String> = serde_yml::from_str(EN_MESSAGES).unwrap();
        let source = include_str!("main.rs");
        for marker in [".text(\"", ".format(\""] {
            for remainder in source.split(marker).skip(1) {
                let key = remainder.split('"').next().unwrap();
                if key.starts_with("gui_")
                    || key.starts_with("status_")
                    || key.starts_with("set_status_")
                {
                    assert!(
                        english.contains_key(key),
                        "missing locales/en.yaml key {key}"
                    );
                }
            }
        }
    }

    #[test]
    fn dat_summary_keeps_missing_categories_separate() {
        let messages = Messages::load("en").unwrap();
        let summary = DatSummary {
            files: 4,
            present: 1,
            missing: 2,
            missing_in_archive: 3,
            nodump: 4,
        };
        let line = summary.line(&messages);
        assert!(line.contains("Missing: 2"));
        assert!(line.contains("MissingInArchive: 3"));
    }
}
