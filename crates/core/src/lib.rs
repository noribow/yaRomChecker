//! Scanning, streaming hashing, archive support, and the persistent scan cache.

mod cache;
mod dat;
mod hash;
mod scan;

pub use cache::ScanCache;
pub use dat::{DatFile, MatchReport, MatchStatus, MatchedEntry, MissingRom, match_collection};
pub use hash::{Hashes, hash_reader};
pub use scan::{EntryKind, ScanEntry, ScanMode, ScanReport, Scanner};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cache error: {0}")]
    Cache(#[from] rusqlite::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("7z error: {0}")]
    SevenZ(#[from] sevenz_rust::Error),
    #[error("scan path is not a directory: {0}")]
    NotDirectory(String),
    #[error("file timestamp is before the Unix epoch: {0}")]
    InvalidTimestamp(String),
    #[error("directory traversal failed: {0}")]
    Walk(#[from] walkdir::Error),
    #[error("DAT XML error: {0}")]
    Xml(#[from] quick_xml::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
