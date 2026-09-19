# yaRomChecker

A ROM verification and organization tool written in Rust.

yaRomChecker scans local ROM collections, matches files against DAT databases, and helps you check completeness and tidy names. It is intended for dumps you already own. It does not download or distribute ROMs.

This application is created with AI.

## Status

Early development. The first release targets Windows. Linux is planned for a later release. macOS is not a current goal.

## Features (planned)

- Shared core used by both the GUI and the CLI
- ZIP and 7z archives, including ZIP entries compressed with Zstandard
- DAT matching (No-Intro, Redump, TOSEC, MAME, and custom DATs, added in stages)
- YAML configuration next to the executable (`yaRomChecker.yaml`); the CLI can pass `--config`
- English UI by default, with locale resource files for other languages (including Japanese)

## License

[MIT](LICENSE)
