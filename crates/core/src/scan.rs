use std::{
    fs::File,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rayon::prelude::*;
use sevenz_rust::{ArchiveReader, Password};
use tracing::debug;
use walkdir::WalkDir;

use crate::{Hashes, Result, ScanCache, hash_reader};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    Full,
    Quick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    ZipEntry,
    SevenZEntry,
}

impl EntryKind {
    pub(crate) fn as_db(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::ZipEntry => "zip",
            Self::SevenZEntry => "7z",
        }
    }

    pub(crate) fn from_db(value: &str) -> Self {
        match value {
            "zip" => Self::ZipEntry,
            "7z" => Self::SevenZEntry,
            _ => Self::File,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    pub container_path: String,
    pub container_name: String,
    pub container_size: u64,
    pub container_mtime_ns: i64,
    pub entry_path: String,
    pub entry_name: String,
    pub entry_size: u64,
    pub kind: EntryKind,
    pub hashes: Hashes,
    /// Unix timestamp in nanoseconds for the most recent actual hash operation.
    /// Cache reuse preserves this value.
    pub last_hashed_ns: i64,
    pub reused: bool,
}

#[derive(Debug, Default)]
pub struct ScanReport {
    pub entries: Vec<ScanEntry>,
    pub hashed_containers: usize,
    pub reused_containers: usize,
}

pub struct Scanner {
    cache: ScanCache,
}

impl Scanner {
    pub fn new(cache: ScanCache) -> Self {
        Self { cache }
    }

    pub fn scan(&mut self, root: &Path, mode: ScanMode) -> Result<ScanReport> {
        self.scan_with_progress(root, mode, |_| {})
    }

    /// Scans a collection and reports each container before it is checked or hashed.
    pub fn scan_with_progress<F>(
        &mut self,
        root: &Path,
        mode: ScanMode,
        mut on_container: F,
    ) -> Result<ScanReport>
    where
        F: FnMut(&Path),
    {
        if !root.is_dir() {
            return Err(crate::Error::NotDirectory(root.display().to_string()));
        }
        let mut files: Vec<_> = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|entry| match entry {
                Ok(e) if e.file_type().is_file() => Some(Ok(e.into_path())),
                Ok(_) => None,
                Err(e) => Some(Err(e.into())),
            })
            .collect::<Result<_>>()?;
        files.sort();

        let mut report = ScanReport::default();
        let mut pending = Vec::new();
        for path in files {
            on_container(&path);
            let identity = FileIdentity::read(&path)?;
            if mode == ScanMode::Quick {
                let cached = self.cache.matching(
                    &identity.path,
                    &identity.name,
                    identity.size,
                    identity.mtime_ns,
                )?;
                if !cached.is_empty() {
                    report.reused_containers += 1;
                    report.entries.extend(cached);
                    continue;
                }
            }
            pending.push((path, identity));
        }

        let hashed = pending
            .par_iter()
            .map(|(path, identity)| {
                debug!(path = %identity.path, "hashing file");
                scan_file(path, identity).map(|entries| (identity.path.clone(), entries))
            })
            .collect::<Result<Vec<_>>>()?;
        for (path, entries) in hashed {
            self.cache.replace(&path, &entries)?;
            report.hashed_containers += 1;
            report.entries.extend(entries);
        }
        Ok(report)
    }
}

struct FileIdentity {
    path: String,
    name: String,
    size: u64,
    mtime_ns: i64,
}

impl FileIdentity {
    fn read(path: &Path) -> Result<Self> {
        let metadata = path.metadata()?;
        let modified = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(|_| crate::Error::InvalidTimestamp(path.display().to_string()))?;
        let canonical = path.canonicalize()?;
        Ok(Self {
            path: canonical.to_string_lossy().into_owned(),
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            size: metadata.len(),
            mtime_ns: i64::try_from(modified.as_nanos()).unwrap_or(i64::MAX),
        })
    }
}

fn scan_file(path: &Path, identity: &FileIdentity) -> Result<Vec<ScanEntry>> {
    let last_hashed_ns = unix_time_ns(SystemTime::now());
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("zip") => scan_zip(path, identity, last_hashed_ns),
        Some("7z") => scan_7z(path, identity, last_hashed_ns),
        _ => Ok(vec![make_entry(
            identity,
            &identity.name,
            identity.size,
            EntryKind::File,
            hash_reader(File::open(path)?)?,
            last_hashed_ns,
        )]),
    }
}

fn scan_zip(path: &Path, identity: &FileIdentity, last_hashed_ns: i64) -> Result<Vec<ScanEntry>> {
    let mut archive = zip::ZipArchive::new(File::open(path)?)?;
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_owned();
        let size = file.size();
        let hashes = hash_reader(&mut file)?;
        entries.push(make_entry(
            identity,
            &name,
            size,
            EntryKind::ZipEntry,
            hashes,
            last_hashed_ns,
        ));
    }
    Ok(entries)
}

fn scan_7z(path: &Path, identity: &FileIdentity, last_hashed_ns: i64) -> Result<Vec<ScanEntry>> {
    let mut archive = ArchiveReader::open(path, Password::empty())?;
    let mut entries = Vec::new();
    archive.for_each_entries(|entry, reader| {
        if !entry.is_directory() {
            let hashes = hash_reader(reader)?;
            entries.push(make_entry(
                identity,
                &entry.name,
                entry.size,
                EntryKind::SevenZEntry,
                hashes,
                last_hashed_ns,
            ));
        }
        Ok(true)
    })?;
    Ok(entries)
}

fn make_entry(
    identity: &FileIdentity,
    entry_path: &str,
    size: u64,
    kind: EntryKind,
    hashes: Hashes,
    last_hashed_ns: i64,
) -> ScanEntry {
    ScanEntry {
        container_path: identity.path.clone(),
        container_name: identity.name.clone(),
        container_size: identity.size,
        container_mtime_ns: identity.mtime_ns,
        entry_path: entry_path.to_owned(),
        entry_name: PathBuf::from(entry_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        entry_size: size,
        kind,
        hashes,
        last_hashed_ns,
        reused: false,
    }
}

fn unix_time_ns(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use sevenz_rust::{ArchiveEntry, ArchiveReader, ArchiveWriter, EncoderMethod, Password};
    use tempfile::TempDir;
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    use super::*;

    fn setup() -> (TempDir, PathBuf, Scanner) {
        let temp = tempfile::tempdir().unwrap();
        let collection = temp.path().join("collection");
        std::fs::create_dir(&collection).unwrap();
        let cache = ScanCache::open(&temp.path().join("cache.sqlite3")).unwrap();
        (temp, collection, Scanner::new(cache))
    }

    fn write_zip(path: &Path, method: CompressionMethod) {
        let mut archive = ZipWriter::new(File::create(path).unwrap());
        archive
            .start_file(
                "folder/game.rom",
                SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        archive.write_all(b"abc").unwrap();
        archive.finish().unwrap();
    }

    #[test]
    fn scans_loose_files_recursively_and_reuses_unchanged_hashes() {
        let (_temp, collection, mut scanner) = setup();
        let nested = collection.join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("game.rom"), b"abc").unwrap();

        let first = scanner.scan(&collection, ScanMode::Full).unwrap();
        assert_eq!(first.hashed_containers, 1);
        assert_eq!(first.entries[0].hashes.crc32, "352441c2");
        assert!(first.entries[0].last_hashed_ns > 0);
        let first_hashed_at = first.entries[0].last_hashed_ns;

        let quick = scanner.scan(&collection, ScanMode::Quick).unwrap();
        assert_eq!(quick.hashed_containers, 0);
        assert_eq!(quick.reused_containers, 1);
        assert!(quick.entries[0].reused);
        assert_eq!(quick.entries[0].last_hashed_ns, first_hashed_at);
    }

    #[test]
    fn hashes_stored_deflated_and_zstd_zip_entries() {
        for (filename, method) in [
            ("stored.zip", CompressionMethod::Stored),
            ("deflated.zip", CompressionMethod::Deflated),
            ("zstd.zip", CompressionMethod::Zstd),
        ] {
            let (_temp, collection, mut scanner) = setup();
            write_zip(&collection.join(filename), method);
            let report = scanner.scan(&collection, ScanMode::Full).unwrap();
            assert_eq!(report.entries.len(), 1);
            assert_eq!(report.entries[0].kind, EntryKind::ZipEntry);
            assert_eq!(report.entries[0].entry_path, "folder/game.rom");
            assert_eq!(
                report.entries[0].hashes.md5,
                "900150983cd24fb0d6963f7d28e17f72"
            );
        }
    }

    #[test]
    fn hashes_7z_entries_without_extracting_them() {
        let (temp, collection, mut scanner) = setup();
        let source = temp.path().join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("game.rom"), b"abc").unwrap();
        sevenz_rust::compress_to_path(&source, collection.join("games.7z")).unwrap();

        let report = scanner.scan(&collection, ScanMode::Full).unwrap();
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].kind, EntryKind::SevenZEntry);
        assert_eq!(
            report.entries[0].hashes.sha1,
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert!(!collection.join("game.rom").exists());
    }

    #[test]
    fn hashes_zstd_7z_entries_without_extracting_them() {
        let (_temp, collection, mut scanner) = setup();
        let archive_path = collection.join("zstd-games.7z");
        let mut archive = ArchiveWriter::create(&archive_path).unwrap();
        archive.set_content_methods(vec![EncoderMethod::ZSTD.into()]);
        archive
            .push_archive_entry(
                ArchiveEntry::new_file("folder/game.rom"),
                Some(Cursor::new(b"abc")),
            )
            .unwrap();
        archive.finish().unwrap();

        let reader = ArchiveReader::open(&archive_path, Password::empty()).unwrap();
        let mut methods = Vec::new();
        reader
            .file_compression_methods("folder/game.rom", &mut methods)
            .unwrap();
        assert_eq!(methods, vec![EncoderMethod::ZSTD]);

        let report = scanner.scan(&collection, ScanMode::Full).unwrap();
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].kind, EntryKind::SevenZEntry);
        assert_eq!(report.entries[0].entry_path, "folder/game.rom");
        assert_eq!(
            report.entries[0].hashes.sha1,
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert!(!collection.join("game.rom").exists());
    }
}
