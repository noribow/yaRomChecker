use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{EntryKind, Result, ScanEntry};
use quick_xml::{Reader, events::Event};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DatHeader {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub homepage: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DumpStatus {
    #[default]
    Good,
    NoDump,
    BadDump,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatRom {
    pub game: String,
    pub name: String,
    pub size: Option<u64>,
    pub crc32: Option<String>,
    pub md5: Option<String>,
    pub sha1: Option<String>,
    pub dump_status: DumpStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DatFile {
    pub path: PathBuf,
    pub header: DatHeader,
    pub roms: Vec<DatRom>,
}

impl DatFile {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let (header, roms) = if contents.trim_start().starts_with('<') {
            parse_xml(&contents)?
        } else {
            parse_clrmamepro(&contents)
        };
        Ok(Self {
            path: path.to_path_buf(),
            header,
            roms,
        })
    }
}

#[derive(Default)]
struct XmlGame {
    name: String,
    description: Option<String>,
    roms: Vec<DatRom>,
}

fn parse_xml(contents: &str) -> Result<(DatHeader, Vec<DatRom>)> {
    let mut reader = Reader::from_str(contents);
    reader.config_mut().trim_text(true);
    let mut header = DatHeader::default();
    let mut roms = Vec::new();
    let mut game: Option<XmlGame> = None;
    let mut in_header = false;
    let mut text_field: Option<Vec<u8>> = None;
    loop {
        match reader.read_event()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"header" => in_header = true,
                b"game" | b"machine" => {
                    game = Some(XmlGame {
                        name: attribute(&e, b"name").unwrap_or_default(),
                        ..Default::default()
                    });
                }
                b"rom" => {
                    if let (Some(parsed), Some(game)) = (parse_xml_rom(&e), &mut game) {
                        game.roms.push(parsed);
                    }
                }
                field @ (b"name" | b"description" | b"version" | b"homepage" | b"url")
                    if in_header || (field == b"description" && game.is_some()) =>
                {
                    text_field = Some(field.to_vec());
                }
                _ => {}
            },
            Event::Empty(e) if e.local_name().as_ref() == b"rom" => {
                if let (Some(parsed), Some(game)) = (parse_xml_rom(&e), &mut game) {
                    game.roms.push(parsed);
                }
            }
            Event::Text(text) => {
                if let Some(field) = &text_field {
                    let value = text.unescape()?.into_owned();
                    if in_header {
                        set_header_field(&mut header, field, value);
                    } else if field == b"description"
                        && let Some(game) = &mut game
                    {
                        game.description = Some(value);
                    }
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"header" => {
                    in_header = false;
                    text_field = None;
                }
                b"game" | b"machine" => {
                    if let Some(mut finished) = game.take() {
                        let title = finished
                            .description
                            .take()
                            .filter(|value| !value.is_empty())
                            .unwrap_or(finished.name);
                        for rom in &mut finished.roms {
                            rom.game.clone_from(&title);
                        }
                        roms.extend(finished.roms);
                    }
                }
                b"name" | b"description" | b"version" | b"homepage" | b"url" => {
                    text_field = None;
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok((header, roms))
}

fn set_header_field(header: &mut DatHeader, field: &[u8], value: String) {
    match field {
        b"name" => header.name = Some(value),
        b"description" => header.description = Some(value),
        b"version" => header.version = Some(value),
        b"homepage" => header.homepage = Some(value),
        b"url" => header.url = Some(value),
        _ => {}
    }
}

fn attribute(tag: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
    tag.attributes()
        .with_checks(false)
        .filter_map(std::result::Result::ok)
        .find(|attr| attr.key.as_ref() == key)
        .and_then(|attr| String::from_utf8(attr.value.into_owned()).ok())
}

fn parse_xml_rom(tag: &quick_xml::events::BytesStart<'_>) -> Option<DatRom> {
    Some(DatRom {
        game: String::new(),
        name: attribute(tag, b"name")?,
        size: attribute(tag, b"size").and_then(|value| value.parse().ok()),
        crc32: normalize_hex(attribute(tag, b"crc")),
        md5: normalize_hex(attribute(tag, b"md5")),
        sha1: normalize_hex(attribute(tag, b"sha1")),
        dump_status: parse_dump_status(attribute(tag, b"status").as_deref()),
    })
}

#[derive(Debug)]
enum Token {
    Word(String),
    Open,
    Close,
}

#[derive(Default)]
struct Node {
    name: String,
    fields: Vec<(String, String)>,
    children: Vec<Node>,
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '(' => tokens.push(Token::Open),
            ')' => tokens.push(Token::Close),
            '"' => {
                let mut value = String::new();
                while let Some(character) = chars.next() {
                    if character == '"' {
                        break;
                    }
                    if character == '\\' {
                        if let Some(next) = chars.next() {
                            value.push(next);
                        }
                    } else {
                        value.push(character);
                    }
                }
                tokens.push(Token::Word(value));
            }
            character if character.is_whitespace() => {}
            character => {
                let mut value = String::from(character);
                while let Some(&next) = chars.peek() {
                    if next.is_whitespace() || matches!(next, '(' | ')') {
                        break;
                    }
                    value.push(next);
                    chars.next();
                }
                tokens.push(Token::Word(value));
            }
        }
    }
    tokens
}

fn parse_nodes(tokens: &[Token]) -> Vec<Node> {
    let mut index = 0;
    let mut nodes = Vec::new();
    while index + 1 < tokens.len() {
        if let Token::Word(name) = &tokens[index]
            && matches!(tokens[index + 1], Token::Open)
        {
            index += 2;
            nodes.push(parse_node(name.clone(), tokens, &mut index));
            continue;
        }
        index += 1;
    }
    nodes
}

fn parse_node(name: String, tokens: &[Token], index: &mut usize) -> Node {
    let mut node = Node {
        name,
        ..Default::default()
    };
    while *index < tokens.len() {
        match &tokens[*index] {
            Token::Close => {
                *index += 1;
                break;
            }
            Token::Word(key)
                if *index + 1 < tokens.len() && matches!(tokens[*index + 1], Token::Open) =>
            {
                let name = key.clone();
                *index += 2;
                node.children.push(parse_node(name, tokens, index));
            }
            Token::Word(key) if *index + 1 < tokens.len() => {
                if let Token::Word(value) = &tokens[*index + 1] {
                    node.fields.push((key.to_ascii_lowercase(), value.clone()));
                    *index += 2;
                } else {
                    *index += 1;
                }
            }
            _ => *index += 1,
        }
    }
    node
}

fn parse_clrmamepro(contents: &str) -> (DatHeader, Vec<DatRom>) {
    let nodes = parse_nodes(&tokenize(contents));
    let mut header = DatHeader::default();
    let mut roms = Vec::new();
    for node in nodes {
        if node.name.eq_ignore_ascii_case("clrmamepro") {
            header.name = field(&node, "name");
            header.description = field(&node, "description");
            header.version = field(&node, "version");
            header.homepage = field(&node, "homepage");
            header.url = field(&node, "url");
        } else if node.name.eq_ignore_ascii_case("game")
            || node.name.eq_ignore_ascii_case("machine")
        {
            let title = field(&node, "description")
                .or_else(|| field(&node, "name"))
                .unwrap_or_default();
            for rom in node
                .children
                .iter()
                .filter(|child| child.name.eq_ignore_ascii_case("rom"))
            {
                if let Some(name) = field(rom, "name") {
                    roms.push(DatRom {
                        game: title.clone(),
                        name,
                        size: field(rom, "size").and_then(|value| value.parse().ok()),
                        crc32: normalize_hex(field(rom, "crc")),
                        md5: normalize_hex(field(rom, "md5")),
                        sha1: normalize_hex(field(rom, "sha1")),
                        dump_status: parse_dump_status(field(rom, "status").as_deref()),
                    });
                }
            }
        }
    }
    (header, roms)
}

fn field(node: &Node, key: &str) -> Option<String> {
    node.fields
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.clone())
}

fn parse_dump_status(value: Option<&str>) -> DumpStatus {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("nodump") => DumpStatus::NoDump,
        Some("baddump") => DumpStatus::BadDump,
        _ => DumpStatus::Good,
    }
}

fn normalize_hex(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_ascii_lowercase();
    (!value.is_empty()).then_some(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Have,
    WrongName,
    WrongDump,
    Duplicate,
    Extra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatRomStatus {
    Present,
    Missing,
    MissingInArchive,
    NoDump,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetStatus {
    Complete,
    Incomplete,
    MissingSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetMatch {
    pub game: String,
    pub status: SetStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMatch {
    pub entry_path: String,
    pub status: FileStatus,
    pub dat_name: Option<String>,
    pub game: Option<String>,
    pub dump_status: Option<DumpStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatRomMatch {
    pub game: String,
    pub name: String,
    pub status: DatRomStatus,
    pub dump_status: DumpStatus,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MatchReport {
    pub files: Vec<FileMatch>,
    pub roms: Vec<DatRomMatch>,
    pub sets: Vec<SetMatch>,
}

pub fn match_collection(entries: &[ScanEntry], dat: &DatFile) -> MatchReport {
    let rom_index: Vec<&DatRom> = dat.roms.iter().collect();
    let mut filled_roms = HashSet::new();
    let mut archive_fills = HashSet::new();
    let mut files = Vec::with_capacity(entries.len());
    for entry in entries {
        if let Some((index, rom)) = rom_index
            .iter()
            .enumerate()
            .find(|(_, rom)| rom.dump_status != DumpStatus::NoDump && rom_hashes_match(rom, entry))
        {
            let newly_filled = filled_roms.insert(index);
            if newly_filled && matches!(entry.kind, EntryKind::ZipEntry | EntryKind::SevenZEntry) {
                archive_fills.insert((rom.game.clone(), entry.container_path.clone()));
            }
            let status = if !newly_filled {
                FileStatus::Duplicate
            } else if entry.entry_name == rom.name {
                FileStatus::Have
            } else {
                FileStatus::WrongName
            };
            files.push(file_match(entry, status, Some(rom)));
        } else if let Some(rom) = rom_index
            .iter()
            .find(|rom| rom.dump_status != DumpStatus::NoDump && entry.entry_name == rom.name)
        {
            files.push(file_match(entry, FileStatus::WrongDump, Some(rom)));
        } else {
            files.push(file_match(entry, FileStatus::Extra, None));
        }
    }
    let roms: Vec<DatRomMatch> = rom_index
        .into_iter()
        .enumerate()
        .map(|(index, rom)| DatRomMatch {
            game: rom.game.clone(),
            name: rom.name.clone(),
            status: if rom.dump_status == DumpStatus::NoDump {
                DatRomStatus::NoDump
            } else if filled_roms.contains(&index) {
                DatRomStatus::Present
            } else if archive_fills
                .iter()
                .any(|(game, _container_path)| game == &rom.game)
            {
                DatRomStatus::MissingInArchive
            } else {
                DatRomStatus::Missing
            },
            dump_status: rom.dump_status,
        })
        .collect();
    let sets = score_sets(&roms);
    MatchReport { files, roms, sets }
}

fn score_sets(roms: &[DatRomMatch]) -> Vec<SetMatch> {
    let mut indexes = HashMap::new();
    let mut counts: Vec<(String, usize, usize)> = Vec::new();
    for rom in roms.iter().filter(|rom| rom.status != DatRomStatus::NoDump) {
        let index = *indexes.entry(rom.game.clone()).or_insert_with(|| {
            counts.push((rom.game.clone(), 0, 0));
            counts.len() - 1
        });
        counts[index].1 += 1;
        if rom.status == DatRomStatus::Present {
            counts[index].2 += 1;
        }
    }
    counts
        .into_iter()
        .map(|(game, required, filled)| SetMatch {
            game,
            status: if filled == required {
                SetStatus::Complete
            } else if filled == 0 {
                SetStatus::MissingSet
            } else {
                SetStatus::Incomplete
            },
        })
        .collect()
}

fn file_match(entry: &ScanEntry, status: FileStatus, rom: Option<&&DatRom>) -> FileMatch {
    FileMatch {
        entry_path: entry.entry_path.clone(),
        status,
        dat_name: rom.map(|rom| rom.name.clone()),
        game: rom.map(|rom| rom.game.clone()),
        dump_status: rom.map(|rom| rom.dump_status),
    }
}

fn rom_hashes_match(rom: &DatRom, entry: &ScanEntry) -> bool {
    if rom
        .size
        .is_some_and(|expected| expected != entry.entry_size)
    {
        return false;
    }
    let mut compared = false;
    for (expected, actual) in [
        (rom.sha1.as_deref(), entry.hashes.sha1.as_str()),
        (rom.md5.as_deref(), entry.hashes.md5.as_str()),
        (rom.crc32.as_deref(), entry.hashes.crc32.as_str()),
    ] {
        if let Some(expected) = expected {
            compared = true;
            if !expected.eq_ignore_ascii_case(actual) {
                return false;
            }
        }
    }
    compared
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntryKind, Hashes};

    const SHA1: &str = "a9993e364706816aba3e25717850c26c9cd0d89d";
    const MD5: &str = "900150983cd24fb0d6963f7d28e17f72";
    const CRC: &str = "352441c2";

    fn entry(name: &str, sha1: &str, size: u64) -> ScanEntry {
        ScanEntry {
            container_path: name.into(),
            container_name: name.into(),
            container_size: size,
            container_mtime_ns: 0,
            entry_path: name.into(),
            entry_name: name.into(),
            entry_size: size,
            kind: EntryKind::File,
            hashes: Hashes {
                crc32: CRC.into(),
                md5: MD5.into(),
                sha1: sha1.into(),
            },
            last_hashed_ns: 0,
            reused: false,
        }
    }

    fn archive_entry(name: &str, sha1: &str, kind: EntryKind) -> ScanEntry {
        ScanEntry {
            container_path: "collection/archive.bin".into(),
            container_name: "archive.bin".into(),
            entry_path: format!("collection/archive.bin/{name}"),
            kind,
            ..entry(name, sha1, 3)
        }
    }

    fn dat(name: &str) -> DatFile {
        DatFile {
            path: "test.dat".into(),
            header: DatHeader::default(),
            roms: vec![DatRom {
                game: "Game".into(),
                name: name.into(),
                size: Some(3),
                crc32: Some(CRC.into()),
                md5: Some(MD5.into()),
                sha1: Some(SHA1.into()),
                dump_status: DumpStatus::Good,
            }],
        }
    }

    fn statuses(entries: &[ScanEntry], dat: DatFile) -> (Vec<FileStatus>, Vec<DatRomStatus>) {
        let report = match_collection(entries, &dat);
        (
            report.files.iter().map(|file| file.status).collect(),
            report.roms.iter().map(|rom| rom.status).collect(),
        )
    }

    fn two_rom_game() -> DatFile {
        let mut collection = dat("present.rom");
        collection.roms.push(DatRom {
            name: "absent.rom".into(),
            crc32: None,
            md5: None,
            sha1: Some("1111111111111111111111111111111111111111".into()),
            ..collection.roms[0].clone()
        });
        collection
    }

    fn disc_game(include_nodump: bool) -> DatFile {
        let mut collection = DatFile {
            path: "invented-disc.dat".into(),
            header: DatHeader::default(),
            roms: vec![
                DatRom {
                    game: "Invented Disc".into(),
                    name: "disc.cue".into(),
                    size: Some(3),
                    crc32: None,
                    md5: None,
                    sha1: Some(SHA1.into()),
                    dump_status: DumpStatus::Good,
                },
                DatRom {
                    game: "Invented Disc".into(),
                    name: "track01.bin".into(),
                    size: Some(3),
                    crc32: None,
                    md5: None,
                    sha1: Some("1111111111111111111111111111111111111111".into()),
                    dump_status: DumpStatus::Good,
                },
            ],
        };
        if include_nodump {
            collection.roms.push(DatRom {
                game: "Invented Disc".into(),
                name: "unknown.bin".into(),
                size: None,
                crc32: None,
                md5: None,
                sha1: None,
                dump_status: DumpStatus::NoDump,
            });
        }
        collection
    }

    #[test]
    fn loads_logiqx_header_machine_description_and_statuses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.dat");
        fs::write(&path, r#"<datafile><header><name>Invented</name><description>Tiny fixture</description><version>1</version><homepage>Example</homepage><url>https://example.invalid/</url></header><machine name="set"><description>Display title</description><rom name="Good (World) [!].rom" size="3" crc="352441C2" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d" status="baddump"/><rom name="unknown.rom" status="nodump"/></machine></datafile>"#).unwrap();
        let loaded = DatFile::load(&path).unwrap();
        assert_eq!(loaded.header.name.as_deref(), Some("Invented"));
        assert_eq!(loaded.header.version.as_deref(), Some("1"));
        assert_eq!(loaded.roms[0].game, "Display title");
        assert_eq!(loaded.roms[0].name, "Good (World) [!].rom");
        assert_eq!(loaded.roms[0].dump_status, DumpStatus::BadDump);
        assert_eq!(loaded.roms[1].dump_status, DumpStatus::NoDump);
    }

    #[test]
    fn loads_clrmamepro_and_preserves_tosec_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.dat");
        fs::write(&path, r#"clrmamepro ( name "Invented CMP" description "Fixture" version "2026" homepage "Home" url "https://example.invalid/" ) game ( name internal description "Display Game" rom ( name "Title (1999)(Publisher) [a].bin" size 3 crc 352441C2 md5 900150983cd24fb0d6963f7d28e17f72 sha1 a9993e364706816aba3e25717850c26c9cd0d89d status baddump ) )"#).unwrap();
        let loaded = DatFile::load(&path).unwrap();
        assert_eq!(loaded.header.name.as_deref(), Some("Invented CMP"));
        assert_eq!(loaded.header.version.as_deref(), Some("2026"));
        assert_eq!(loaded.roms[0].game, "Display Game");
        assert_eq!(loaded.roms[0].name, "Title (1999)(Publisher) [a].bin");
        assert_eq!(loaded.roms[0].dump_status, DumpStatus::BadDump);
    }

    #[test]
    fn nodump_is_not_missing_and_cannot_be_filled() {
        let mut collection = dat("unknown.rom");
        collection.roms[0].dump_status = DumpStatus::NoDump;
        assert_eq!(
            statuses(&[entry("unknown.rom", SHA1, 3)], collection),
            (vec![FileStatus::Extra], vec![DatRomStatus::NoDump])
        );
    }

    #[test]
    fn baddump_matches_and_is_marked() {
        let mut collection = dat("game.rom");
        collection.roms[0].dump_status = DumpStatus::BadDump;
        let report = match_collection(&[entry("game.rom", SHA1, 3)], &collection);
        assert_eq!(report.files[0].status, FileStatus::Have);
        assert_eq!(report.files[0].dump_status, Some(DumpStatus::BadDump));
    }

    #[test]
    fn classifies_have_and_present() {
        assert_eq!(
            statuses(&[entry("game.rom", SHA1, 3)], dat("game.rom")),
            (vec![FileStatus::Have], vec![DatRomStatus::Present])
        );
    }

    #[test]
    fn exact_case_sensitive_name() {
        assert_eq!(
            statuses(&[entry("Game.rom", SHA1, 3)], dat("game.rom")).0,
            vec![FileStatus::WrongName]
        );
    }

    #[test]
    fn wrong_dump_does_not_fill() {
        assert_eq!(
            statuses(&[entry("game.rom", "bad", 3)], dat("game.rom")),
            (vec![FileStatus::WrongDump], vec![DatRomStatus::Missing])
        );
    }

    #[test]
    fn missing_sibling_of_zip_filled_rom_is_missing_in_archive() {
        assert_eq!(
            statuses(
                &[archive_entry("present.rom", SHA1, EntryKind::ZipEntry)],
                two_rom_game()
            ),
            (
                vec![FileStatus::Have],
                vec![DatRomStatus::Present, DatRomStatus::MissingInArchive]
            )
        );
    }

    #[test]
    fn game_without_collection_files_is_missing() {
        assert_eq!(
            statuses(&[], two_rom_game()).1,
            vec![DatRomStatus::Missing, DatRomStatus::Missing]
        );
    }

    #[test]
    fn wrong_dump_in_archive_does_not_create_archive_evidence() {
        assert_eq!(
            statuses(
                &[archive_entry("present.rom", "bad", EntryKind::SevenZEntry)],
                two_rom_game()
            ),
            (
                vec![FileStatus::WrongDump],
                vec![DatRomStatus::Missing, DatRomStatus::Missing]
            )
        );
    }

    #[test]
    fn loose_file_fill_does_not_create_archive_evidence() {
        assert_eq!(
            statuses(&[entry("present.rom", SHA1, 3)], two_rom_game()).1,
            vec![DatRomStatus::Present, DatRomStatus::Missing]
        );
    }

    #[test]
    fn archive_fill_does_not_affect_a_different_game() {
        let mut collection = two_rom_game();
        collection.roms[1].game = "Other Game".into();
        assert_eq!(
            statuses(
                &[archive_entry("present.rom", SHA1, EntryKind::ZipEntry)],
                collection
            )
            .1,
            vec![DatRomStatus::Present, DatRomStatus::Missing]
        );
    }

    #[test]
    fn nodump_sibling_of_archive_fill_remains_nodump() {
        let mut collection = two_rom_game();
        collection.roms[1].dump_status = DumpStatus::NoDump;
        assert_eq!(
            statuses(
                &[archive_entry("present.rom", SHA1, EntryKind::ZipEntry)],
                collection
            )
            .1,
            vec![DatRomStatus::Present, DatRomStatus::NoDump]
        );
    }

    #[test]
    fn later_hash_hit_is_duplicate() {
        assert_eq!(
            statuses(
                &[entry("game.rom", SHA1, 3), entry("copy.rom", SHA1, 3)],
                dat("game.rom")
            ),
            (
                vec![FileStatus::Have, FileStatus::Duplicate],
                vec![DatRomStatus::Present]
            )
        );
    }

    #[test]
    fn every_listed_hash_and_size_must_match() {
        let mut candidate = entry("game.rom", SHA1, 3);
        candidate.hashes.md5 = "bad".into();
        assert_eq!(
            statuses(&[candidate], dat("game.rom")).0,
            vec![FileStatus::WrongDump]
        );
        assert_eq!(
            statuses(&[entry("game.rom", SHA1, 4)], dat("game.rom")).1,
            vec![DatRomStatus::Missing]
        );
    }

    #[test]
    fn hash_identity_wins() {
        let mut collection = dat("hash-target.rom");
        collection.roms.push(DatRom {
            name: "name-target.rom".into(),
            sha1: Some("different".into()),
            ..collection.roms[0].clone()
        });
        let report = match_collection(&[entry("name-target.rom", SHA1, 3)], &collection);
        assert_eq!(report.files[0].status, FileStatus::WrongName);
        assert_eq!(report.files[0].dat_name.as_deref(), Some("hash-target.rom"));
    }

    #[test]
    fn cue_and_bin_filled_is_a_complete_set() {
        let report = match_collection(
            &[
                entry("disc.cue", SHA1, 3),
                entry("track01.bin", "1111111111111111111111111111111111111111", 3),
            ],
            &disc_game(false),
        );
        assert_eq!(
            report.sets,
            vec![SetMatch {
                game: "Invented Disc".into(),
                status: SetStatus::Complete,
            }]
        );
    }

    #[test]
    fn bin_filled_and_cue_missing_is_an_incomplete_set() {
        let report = match_collection(
            &[entry(
                "track01.bin",
                "1111111111111111111111111111111111111111",
                3,
            )],
            &disc_game(false),
        );
        assert_eq!(report.sets[0].status, SetStatus::Incomplete);
    }

    #[test]
    fn game_without_collection_files_is_a_missing_set() {
        let report = match_collection(&[], &disc_game(false));
        assert_eq!(report.sets[0].status, SetStatus::MissingSet);
    }

    #[test]
    fn nodump_rom_does_not_make_a_filled_set_incomplete() {
        let report = match_collection(
            &[
                entry("disc.cue", SHA1, 3),
                entry("track01.bin", "1111111111111111111111111111111111111111", 3),
            ],
            &disc_game(true),
        );
        assert_eq!(report.sets[0].status, SetStatus::Complete);
    }

    #[test]
    fn filled_baddump_rom_counts_toward_set_completion() {
        let mut collection = dat("game.rom");
        collection.roms[0].dump_status = DumpStatus::BadDump;
        let report = match_collection(&[entry("game.rom", SHA1, 3)], &collection);
        assert_eq!(report.sets[0].status, SetStatus::Complete);
        assert_eq!(report.files[0].dump_status, Some(DumpStatus::BadDump));
    }

    #[test]
    fn nodump_only_game_is_omitted_from_set_scores() {
        let mut collection = dat("unknown.rom");
        collection.roms[0].dump_status = DumpStatus::NoDump;
        assert!(match_collection(&[], &collection).sets.is_empty());
    }

    #[test]
    fn separate_dat_collection_pairs_do_not_share_files_or_extras() {
        let a_entries = vec![entry("a.rom", SHA1, 3)];
        let a_report = match_collection(&a_entries, &dat("a.rom"));
        let b_report = match_collection(&[], &dat("b.rom"));

        assert_eq!(a_report.files[0].status, FileStatus::Have);
        assert_eq!(a_report.roms[0].status, DatRomStatus::Present);
        assert!(b_report.files.is_empty());
        assert_eq!(b_report.roms[0].status, DatRomStatus::Missing);
    }

    #[test]
    fn identical_hashes_in_different_pairs_do_not_steal_first_hit() {
        let a_report = match_collection(&[entry("a.rom", SHA1, 3)], &dat("a.rom"));
        let b_report = match_collection(&[entry("b.rom", SHA1, 3)], &dat("b.rom"));

        assert_eq!(a_report.files[0].status, FileStatus::Have);
        assert_eq!(b_report.files[0].status, FileStatus::Have);
        assert_eq!(b_report.roms[0].status, DatRomStatus::Present);
    }

    #[test]
    fn two_dats_can_report_independently_for_one_collection() {
        let entries = vec![entry("shared.rom", SHA1, 3)];
        let matching = match_collection(&entries, &dat("shared.rom"));
        let other = match_collection(&entries, &dat("other.rom"));

        assert_eq!(matching.files[0].status, FileStatus::Have);
        assert_eq!(matching.roms[0].status, DatRomStatus::Present);
        assert_eq!(other.files[0].status, FileStatus::WrongName);
        assert_eq!(other.roms[0].status, DatRomStatus::Present);
    }
}
