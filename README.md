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

Copy `yaRomChecker.example.yaml` to `yaRomChecker.yaml` beside the `yarc` executable. Unless `--config` is supplied, configuration is loaded from that location rather than the current working directory. Relative cache paths are resolved from the configuration file's directory.

The default locale is English. Additional languages use resource files under `locales/`.

## Scan

```powershell
yarc scan C:\path\to\collection
yarc quick C:\path\to\collection
yarc full C:\path\to\collection
```

`scan` and `full` stream every loose file and every ZIP/7z entry through CRC32, MD5, and SHA1 hashers. `quick` reuses cached hashes when the container path, file name, size, and modification time all match.

## Planned

- GUI (`yaRomChecker`) sharing the same core as `yarc`
- DAT matching (No-Intro, Redump, TOSEC, MAME, and custom DATs, added in stages)
- Organize (rename / quarantine), after a dry-run preview
- External archive media checks (read-only; no writing to those media)

## Documentation

- [Requirements](docs/REQUIREMENTS.md) — product decisions
- [AGENTS.md](AGENTS.md) — instructions for Codex (implementation) and reviewers

Implementation is done by Codex on pull requests labeled **`codex`**. Direct pushes to `main` are not used for that work.

## License

[MIT](LICENSE)
