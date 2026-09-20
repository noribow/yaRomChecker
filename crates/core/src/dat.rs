use std::{
    collections::HashSet,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use crate::{Result, ScanEntry};
use quick_xml::{Reader, events::Event};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatRom {
    pub game: String,
    pub name: String,
    pub size: Option<u64>,
    pub crc32: Option<String>,
    pub md5: Option<String>,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DatFile {
    pub path: PathBuf,
    pub roms: Vec<DatRom>,
}

impl DatFile {
    pub fn load(path: &Path) -> Result<Self> {
        let mut reader = Reader::from_reader(BufReader::new(File::open(path)?));
        reader.config_mut().trim_text(true);
        let (mut buf, mut game, mut roms) = (Vec::new(), String::new(), Vec::new());
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                    b"game" | b"machine" => game = attribute(&e, b"name").unwrap_or_default(),
                    b"rom" => {
                        if let Some(rom) = parse_rom(&e, &game) {
                            roms.push(rom);
                        }
                    }
                    _ => {}
                },
                Event::End(e) if matches!(e.local_name().as_ref(), b"game" | b"machine") => {
                    game.clear()
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
        Ok(Self {
            path: path.to_path_buf(),
            roms,
        })
    }
}

fn attribute(tag: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
    tag.attributes()
        .with_checks(false)
        .filter_map(std::result::Result::ok)
        .find(|attr| attr.key.as_ref() == key)
        .and_then(|attr| String::from_utf8(attr.value.into_owned()).ok())
}

fn parse_rom(tag: &quick_xml::events::BytesStart<'_>, game: &str) -> Option<DatRom> {
    Some(DatRom {
        game: game.to_owned(),
        name: attribute(tag, b"name")?,
        size: attribute(tag, b"size").and_then(|value| value.parse().ok()),
        crc32: normalize_hex(attribute(tag, b"crc")),
        md5: normalize_hex(attribute(tag, b"md5")),
        sha1: normalize_hex(attribute(tag, b"sha1")),
    })
}

fn normalize_hex(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_ascii_lowercase();
    if value.is_empty() { None } else { Some(value) }
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMatch {
    pub entry_path: String,
    pub status: FileStatus,
    pub dat_name: Option<String>,
    pub game: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatRomMatch {
    pub game: String,
    pub name: String,
    pub status: DatRomStatus,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MatchReport {
    pub files: Vec<FileMatch>,
    pub roms: Vec<DatRomMatch>,
}

pub fn match_collection(entries: &[ScanEntry], dats: &[DatFile]) -> MatchReport {
    let rom_index: Vec<&DatRom> = dats.iter().flat_map(|dat| &dat.roms).collect();
    let mut filled_roms = HashSet::new();
    let mut files = Vec::with_capacity(entries.len());

    for entry in entries {
        // Hash identity is authoritative: a hash hit is not reassigned based on its name.
        if let Some((index, rom)) = rom_index
            .iter()
            .enumerate()
            .find(|(_, rom)| rom_hashes_match(rom, entry))
        {
            let status = if !filled_roms.insert(index) {
                FileStatus::Duplicate
            } else if entry.entry_name == rom.name {
                FileStatus::Have
            } else {
                FileStatus::WrongName
            };
            files.push(file_match(entry, status, Some(rom)));
        } else if let Some(rom) = rom_index.iter().find(|rom| entry.entry_name == rom.name) {
            files.push(file_match(entry, FileStatus::WrongDump, Some(rom)));
        } else {
            files.push(file_match(entry, FileStatus::Extra, None));
        }
    }

    let roms = rom_index
        .into_iter()
        .enumerate()
        .map(|(index, rom)| DatRomMatch {
            game: rom.game.clone(),
            name: rom.name.clone(),
            status: if filled_roms.contains(&index) {
                DatRomStatus::Present
            } else {
                DatRomStatus::Missing
            },
        })
        .collect();
    MatchReport { files, roms }
}

fn file_match(entry: &ScanEntry, status: FileStatus, rom: Option<&&DatRom>) -> FileMatch {
    FileMatch {
        entry_path: entry.entry_path.clone(),
        status,
        dat_name: rom.map(|rom| rom.name.clone()),
        game: rom.map(|rom| rom.game.clone()),
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
            reused: false,
        }
    }

    fn dat(name: &str) -> DatFile {
        DatFile {
            path: "test.dat".into(),
            roms: vec![DatRom {
                game: "Game".into(),
                name: name.into(),
                size: Some(3),
                crc32: Some(CRC.into()),
                md5: Some(MD5.into()),
                sha1: Some(SHA1.into()),
            }],
        }
    }

    fn statuses(entries: &[ScanEntry], dat: DatFile) -> (Vec<FileStatus>, Vec<DatRomStatus>) {
        let report = match_collection(entries, &[dat]);
        (
            report.files.iter().map(|file| file.status).collect(),
            report.roms.iter().map(|rom| rom.status).collect(),
        )
    }

    #[test]
    fn loads_logiqx_rom() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.dat");
        std::fs::write(&path, r#"<datafile><game name="Set A"><rom name="a.rom" size="3" crc="352441C2" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/></game></datafile>"#).unwrap();
        let loaded = DatFile::load(&path).unwrap();
        let mut expected = dat("a.rom").roms;
        expected[0].game = "Set A".into();
        assert_eq!(loaded.roms, expected);
    }

    #[test]
    fn classifies_have_and_present() {
        assert_eq!(
            statuses(&[entry("game.rom", SHA1, 3)], dat("game.rom")),
            (vec![FileStatus::Have], vec![DatRomStatus::Present])
        );
    }

    #[test]
    fn classifies_wrong_name_including_case_only_difference() {
        assert_eq!(
            statuses(&[entry("other.rom", SHA1, 3)], dat("game.rom")).0,
            vec![FileStatus::WrongName]
        );
        assert_eq!(
            statuses(&[entry("Game.rom", SHA1, 3)], dat("game.rom")).0,
            vec![FileStatus::WrongName]
        );
    }

    #[test]
    fn classifies_wrong_dump_without_filling_rom() {
        assert_eq!(
            statuses(&[entry("game.rom", "bad", 3)], dat("game.rom")),
            (vec![FileStatus::WrongDump], vec![DatRomStatus::Missing])
        );
    }

    #[test]
    fn classifies_extra_and_missing() {
        assert_eq!(
            statuses(&[entry("other.rom", "bad", 3)], dat("game.rom")),
            (vec![FileStatus::Extra], vec![DatRomStatus::Missing])
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
    fn size_mismatch_is_wrong_dump_and_missing() {
        assert_eq!(
            statuses(&[entry("game.rom", SHA1, 4)], dat("game.rom")),
            (vec![FileStatus::WrongDump], vec![DatRomStatus::Missing])
        );
    }

    #[test]
    fn every_listed_hash_must_match() {
        let mut candidate = entry("game.rom", SHA1, 3);
        candidate.hashes.md5 = "ffffffffffffffffffffffffffffffff".into();
        assert_eq!(
            statuses(&[candidate], dat("game.rom")).0,
            vec![FileStatus::WrongDump]
        );
    }

    #[test]
    fn hash_identity_wins_over_name_identity() {
        let mut collection = dat("hash-target.rom");
        collection.roms.push(DatRom {
            name: "name-target.rom".into(),
            sha1: Some("different".into()),
            ..collection.roms[0].clone()
        });
        let report = match_collection(&[entry("name-target.rom", SHA1, 3)], &[collection]);
        assert_eq!(report.files[0].status, FileStatus::WrongName);
        assert_eq!(report.files[0].dat_name.as_deref(), Some("hash-target.rom"));
    }
}
