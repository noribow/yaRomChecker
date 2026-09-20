use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::{EntryKind, Hashes, Result, ScanEntry};

pub struct ScanCache {
    connection: Connection,
}

impl ScanCache {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS scan_entries (
                container_path TEXT NOT NULL,
                container_name TEXT NOT NULL,
                container_size INTEGER NOT NULL,
                container_mtime_ns INTEGER NOT NULL,
                entry_path TEXT NOT NULL,
                entry_name TEXT NOT NULL,
                entry_size INTEGER NOT NULL,
                entry_kind TEXT NOT NULL,
                crc32 TEXT NOT NULL,
                md5 TEXT NOT NULL,
                sha1 TEXT NOT NULL,
                last_hashed_ns INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (container_path, entry_path)
             );",
        )?;
        let has_last_hashed = {
            let mut statement = connection.prepare("PRAGMA table_info(scan_entries)")?;
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<std::result::Result<Vec<_>, _>>()?
                .iter()
                .any(|column| column == "last_hashed_ns")
        };
        if !has_last_hashed {
            connection.execute(
                "ALTER TABLE scan_entries ADD COLUMN last_hashed_ns INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        Ok(Self { connection })
    }

    pub(crate) fn matching(
        &self,
        path: &str,
        name: &str,
        size: u64,
        mtime_ns: i64,
    ) -> Result<Vec<ScanEntry>> {
        let identity: Option<i64> = self
            .connection
            .query_row(
                "SELECT 1 FROM scan_entries WHERE container_path=?1 AND container_name=?2
                 AND container_size=?3 AND container_mtime_ns=?4 LIMIT 1",
                params![path, name, size as i64, mtime_ns],
                |row| row.get(0),
            )
            .optional()?;
        if identity.is_none() {
            return Ok(Vec::new());
        }

        let mut statement = self.connection.prepare(
            "SELECT entry_path, entry_name, entry_size, entry_kind, crc32, md5, sha1, last_hashed_ns
             FROM scan_entries WHERE container_path=?1 ORDER BY entry_path",
        )?;
        let entries = statement
            .query_map([path], |row| {
                let kind: String = row.get(3)?;
                Ok(ScanEntry {
                    container_path: path.to_owned(),
                    container_name: name.to_owned(),
                    container_size: size,
                    container_mtime_ns: mtime_ns,
                    entry_path: row.get(0)?,
                    entry_name: row.get(1)?,
                    entry_size: row.get::<_, i64>(2)? as u64,
                    kind: EntryKind::from_db(&kind),
                    hashes: Hashes {
                        crc32: row.get(4)?,
                        md5: row.get(5)?,
                        sha1: row.get(6)?,
                    },
                    last_hashed_ns: row.get(7)?,
                    reused: true,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    pub(crate) fn replace(&mut self, path: &str, entries: &[ScanEntry]) -> Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM scan_entries WHERE container_path=?1", [path])?;
        for entry in entries {
            transaction.execute(
                "INSERT INTO scan_entries (
                    container_path, container_name, container_size, container_mtime_ns,
                    entry_path, entry_name, entry_size, entry_kind, crc32, md5, sha1,
                    last_hashed_ns
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![
                    entry.container_path,
                    entry.container_name,
                    entry.container_size as i64,
                    entry.container_mtime_ns,
                    entry.entry_path,
                    entry.entry_name,
                    entry.entry_size as i64,
                    entry.kind.as_db(),
                    entry.hashes.crc32,
                    entry.hashes.md5,
                    entry.hashes.sha1,
                    entry.last_hashed_ns,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_cache_created_before_last_hashed_timestamp() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("old.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE scan_entries (
                    container_path TEXT NOT NULL, container_name TEXT NOT NULL,
                    container_size INTEGER NOT NULL, container_mtime_ns INTEGER NOT NULL,
                    entry_path TEXT NOT NULL, entry_name TEXT NOT NULL, entry_size INTEGER NOT NULL,
                    entry_kind TEXT NOT NULL, crc32 TEXT NOT NULL, md5 TEXT NOT NULL,
                    sha1 TEXT NOT NULL, PRIMARY KEY (container_path, entry_path)
                 );",
            )
            .unwrap();
        drop(connection);

        let cache = ScanCache::open(&path).unwrap();
        let columns: Vec<String> = cache
            .connection
            .prepare("PRAGMA table_info(scan_entries)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert!(columns.iter().any(|column| column == "last_hashed_ns"));
    }
}
