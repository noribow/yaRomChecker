# yaRomChecker

A ROM verification and organization tool written in Rust.

yaRomChecker scans locally owned ROM collections, matches files against DAT databases, and helps you check completeness and tidy names. It does not download or distribute ROMs.

This application is created with AI.

## Status

Early development. The first release targets Windows. Linux is planned for a later release. macOS is not a current goal.

This milestone ships the shared core library and the `yarc` CLI scanner. The GUI is not included yet.

## Build

Rust stable is required.

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Configuration

If `yaRomChecker.yaml` (or the path given to `--config`) does not exist, `yarc` creates it with default contents and then loads it. Existing files are never overwritten. Relative cache, DAT, and collection paths are resolved from the configuration file's directory. Configure ordered DAT and collection pairs that share one scan cache:

```yaml
locale: en
cache_path: yaRomChecker-cache.sqlite3
sources:
  - dat: dats/nointro-nes.dat
    collection: collections/nes
  - dat: dats/tosec.dat
    collection: collections/tosec
```

The default locale is English. Additional languages use resource files under `locales/`.

## Scan

```powershell
yarc scan C:\path\to\collection
yarc quick C:\path\to\collection
yarc verify
```

`verify` quick-scans each unique collection directory under `sources:` into the shared cache, then matches every DAT only against its paired directory. Sources that share a collection get independent reports from the same cached scan rows. Logiqx XML and ClrMamePro text DATs are detected by content. Every hash the DAT lists for a ROM (CRC32, MD5, SHA1) must match; size, if present, must match too. Names, including long TOSEC-style names, are compared exactly and case-sensitively to the DAT `rom` name.

File statuses: **Have** (hash and name), **WrongName** (hash only, including case-only name differences), **WrongDump** (name only), **Duplicate** (later hash hit for a ROM already filled), **Extra** (neither). A DAT ROM is **Present** after any hash hit and **Missing** otherwise; WrongDump does not fill it.

DAT files are not bundled or downloaded by yaRomChecker, and this project does not provide DAT download links. Supply DATs you are authorized to use. A `nodump` entry is informational rather than missing and cannot be filled; a matching `baddump` entry is clearly marked in the report. The verifier also prints each DAT's header name and version when available.

`scan` and `full` stream every loose file and every ZIP/7z entry through CRC32, MD5, and SHA1 hashers. `quick` reuses cached hashes when the container path, file name, size, and modification time all match.

## Planned

- GUI (`yaRomChecker`) sharing the same core as `yarc`
- DAT families that need a different data model than one file ↔ one ROM (see Remaining DAT work in [Requirements](docs/REQUIREMENTS.md))
- Organize (rename / quarantine), after a dry-run preview
- External archive media checks (read-only; no writing to those media)

## Documentation

- [Requirements](docs/REQUIREMENTS.md) — product decisions
- [GUI wireframes](docs/GUI_WIREFRAMES.md) — planned menu bar, four-pane DAT explorer, scan popup, Settings, and Report
- [AGENTS.md](AGENTS.md) — issue → Codex PR → Cursor review/test report

Implementation: GitHub issue first, then Codex (`codex` PRs). Cursor reviews and tests; it does not land product code. Direct pushes to `main` are not used for that work.

## License

[MIT](LICENSE)
