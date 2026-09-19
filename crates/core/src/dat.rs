use std::{
    collections::HashSet,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use quick_xml::{Reader, events::Event};

use crate::{Result, ScanEntry};

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
        let mut buf = Vec::new();
        let mut game = String::new();
        let mut roms = Vec::new();

        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                    b"game" | b"machine" => {
                        game = attribute(&e, b"name").unwrap_or_default();
                    }
                    b"rom" => {
                        if let Some(rom) = parse_rom(&e, &game) {
                            roms.push(rom);
                        }
                    }
                    _ => {}
                },
                Event::End(e) if matches!(e.local_name().as_ref(), b"game" | b"machine") => {
                    game.clear();
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
    let name = attribute(tag, b"name")?;
    Some(DatRom {
        game: game.to_owned(),
        name,
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
pub enum MatchStatus {
    Matched,
    Extra,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedEntry {
    pub entry_path: String,
    pub status: MatchStatus,
    pub dat_name: Option<String>,
    pub game: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingRom {
    pub game: String,
    pub name: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MatchReport {
    pub matched: Vec<MatchedEntry>,
    pub extra: Vec<MatchedEntry>,
    pub missing: Vec<MissingRom>,
}

pub fn match_collection(entries: &[ScanEntry], dats: &[DatFile]) -> MatchReport {
    let mut rom_index: Vec<&DatRom> = Vec::new();
    for dat in dats {
        for rom in &dat.roms {
            rom_index.push(rom);
        }
    }

    let mut report = MatchReport::default();
    // Hash (+ DAT size) hits only. Filename matching, if added later, must not reuse this set.
    let mut hash_matched_roms = HashSet::new();

    for entry in entries {
        let sha1 = entry.hashes.sha1.to_ascii_lowercase();
        let md5 = entry.hashes.md5.to_ascii_lowercase();
        let crc = entry.hashes.crc32.to_ascii_lowercase();
        let hash_hit = rom_index
            .iter()
            .enumerate()
            .find(|(_, rom)| rom_hashes_match(rom, &sha1, &md5, &crc, entry.entry_size));
        if let Some((index, rom)) = hash_hit {
            hash_matched_roms.insert(index);
            report.matched.push(MatchedEntry {
                entry_path: entry.entry_path.clone(),
                status: MatchStatus::Matched,
                dat_name: Some(rom.name.clone()),
                game: Some(rom.game.clone()),
            });
        } else {
            report.extra.push(MatchedEntry {
                entry_path: entry.entry_path.clone(),
                status: MatchStatus::Extra,
                dat_name: None,
                game: None,
            });
        }
    }

    for (index, rom) in rom_index.iter().enumerate() {
        if !hash_matched_roms.contains(&index) {
            report.missing.push(MissingRom {
                game: rom.game.clone(),
                name: rom.name.clone(),
            });
        }
    }

    report
}

fn rom_hashes_match(rom: &DatRom, sha1: &str, md5: &str, crc: &str, size: u64) -> bool {
    if let Some(expected) = rom.size
        && expected != size
    {
        return false;
    }
    let mut compared = false;
    if let Some(expected) = &rom.sha1 {
        compared = true;
        if expected != sha1 {
            return false;
        }
    }
    if let Some(expected) = &rom.md5 {
        compared = true;
        if expected != md5 {
            return false;
        }
    }
    if let Some(expected) = &rom.crc32 {
        compared = true;
        if expected != crc {
            return false;
        }
    }
    compared
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntryKind, Hashes, ScanEntry};

    fn entry(path: &str, sha1: &str, md5: &str, crc: &str, size: u64) -> ScanEntry {
        ScanEntry {
            container_path: path.to_owned(),
            container_name: path.to_owned(),
            container_size: size,
            container_mtime_ns: 0,
            entry_path: path.to_owned(),
            entry_name: path.to_owned(),
            entry_size: size,
            kind: EntryKind::File,
            hashes: Hashes {
                crc32: crc.to_owned(),
                md5: md5.to_owned(),
                sha1: sha1.to_owned(),
            },
            reused: false,
        }
    }

    #[test]
    fn loads_logiqx_roms_and_classifies_matched_extra_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        let dat_path = dir.path().join("sample.dat");
        std::fs::write(
            &dat_path,
            r#"<?xml version="1.0"?>
<datafile>
  <game name="Set A">
    <rom name="a.rom" size="3" crc="352441c2" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/>
    <rom name="missing.rom" size="1" crc="00000000" md5="d41d8cd98f00b204e9800998ecf8427e" sha1="da39a3ee5e6b4b0d3255bfef95601890afd80709"/>
  </game>
</datafile>
"#,
        )
        .unwrap();

        let dat = DatFile::load(&dat_path).unwrap();
        assert_eq!(dat.roms.len(), 2);

        let entries = vec![
            entry(
                "a.rom",
                "a9993e364706816aba3e25717850c26c9cd0d89d",
                "900150983cd24fb0d6963f7d28e17f72",
                "352441c2",
                3,
            ),
            entry(
                "extra.rom",
                "deadbeef",
                "deadbeefdeadbeefdeadbeefdeadbeef",
                "ffffffff",
                4,
            ),
        ];
        let report = match_collection(&entries, &[dat]);
        assert_eq!(report.matched.len(), 1);
        assert_eq!(report.extra.len(), 1);
        assert_eq!(report.missing.len(), 1);
        assert_eq!(report.missing[0].name, "missing.rom");
    }

    #[test]
    fn size_mismatch_does_not_count_as_matched() {
        let rom = DatRom {
            game: "Set".into(),
            name: "a.rom".into(),
            size: Some(3),
            crc32: Some("352441c2".into()),
            md5: Some("900150983cd24fb0d6963f7d28e17f72".into()),
            sha1: Some("a9993e364706816aba3e25717850c26c9cd0d89d".into()),
        };
        let dat = DatFile {
            path: PathBuf::from("x.dat"),
            roms: vec![rom],
        };
        let entries = vec![entry(
            "a.rom",
            "a9993e364706816aba3e25717850c26c9cd0d89d",
            "900150983cd24fb0d6963f7d28e17f72",
            "352441c2",
            99,
        )];
        let report = match_collection(&entries, &[dat]);
        assert!(report.matched.is_empty());
        assert_eq!(report.extra.len(), 1);
        assert_eq!(report.missing.len(), 1);
    }

    #[test]
    fn every_hash_listed_in_the_dat_must_match() {
        let rom = DatRom {
            game: "Set".into(),
            name: "a.rom".into(),
            size: Some(3),
            crc32: Some("352441c2".into()),
            md5: Some("900150983cd24fb0d6963f7d28e17f72".into()),
            sha1: Some("a9993e364706816aba3e25717850c26c9cd0d89d".into()),
        };
        let dat = DatFile {
            path: PathBuf::from("x.dat"),
            roms: vec![rom],
        };
        let only_sha1 = entry(
            "a.rom",
            "a9993e364706816aba3e25717850c26c9cd0d89d",
            "ffffffffffffffffffffffffffffffff",
            "00000000",
            3,
        );
        let report = match_collection(&[only_sha1], std::slice::from_ref(&dat));
        assert!(report.matched.is_empty());
        assert_eq!(report.extra.len(), 1);
        assert_eq!(report.missing.len(), 1);

        let all_hashes = entry(
            "a.rom",
            "a9993e364706816aba3e25717850c26c9cd0d89d",
            "900150983cd24fb0d6963f7d28e17f72",
            "352441c2",
            3,
        );
        let report = match_collection(&[all_hashes], &[dat]);
        assert_eq!(report.matched.len(), 1);
        assert!(report.extra.is_empty());
        assert!(report.missing.is_empty());
    }
}
