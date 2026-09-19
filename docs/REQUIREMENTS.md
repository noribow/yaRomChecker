# yaRomChecker requirements

Decided product rules. Codex implements against this document. Cursor reviews PRs.

This application is created with AI. License: MIT. Repository: https://github.com/noribow/yaRomChecker

## Purpose

Verify and organize **locally owned** ROM dumps. Match against user-provided DAT files. Do not download or distribute ROMs or DAT collections.

## Platform

| Item | Decision |
| --- | --- |
| v1 OS | Windows |
| Later | Linux in a future release |
| Other | macOS, lowest priority |
| Language | Rust, stable, edition 2024 |
| License | MIT |
| GUI | Separate binary `yaRomChecker`, **egui + eframe** first |
| CLI | Separate binary `yarc`, clap, `--config` |
| Core | Shared library crate |

Avoid Win32-only APIs where a portable path exists (Linux is planned). Do not commit to macOS.

## Configuration

- YAML only. **Never store settings in a database.**
- Default file: `yaRomChecker.yaml` in the **same directory as the executable** (not the current working directory).
- CLI may pass `--config <path>`.
- If the config file **does not exist**, create it at that path with the default contents (`locale: en` and a default `cache_path`) and then load it. Do not overwrite an existing file.
- Scan caches (hashes, sizes, mtimes) may use SQLite or another derived store. Paths for cache/logs belong in YAML.

## i18n

- Default locale `en`. UI, CLI, logs shown to users: English resources.
- Switch via YAML `locale` and resource files (`locales/en.yaml`, `locales/ja.yaml`).
- v1: complete `en`; `ja` is the first extra locale.
- Public README and GitHub pages stay English. Optional `README.ja.md` later.

## Archives

- Containers: ZIP and 7z (first class). RAR later. Standalone `.zst` later.
- ZIP compression: stored, deflate, **Zstandard (APPNOTE method 93)**.
- 7z: include zstd codec when present.
- Implementation: **pure Rust**. Do not require 7-Zip.

## Scanning

**Initial / full scan**

- Do not treat archive *listing* as verification.
- Decompress **in memory**, stream into hashers. Do not extract to disk. Do not load huge images as one `Vec<u8>` if a stream works.
- Hash CRC32, MD5, SHA1. Loose files: same streaming hash.
- Store results (path, name, size, mtime, hashes, inner entries for archives).

**Quick rescan**

- Compare each on-disk file to the stored record using **file name + path + size + mtime**.
- All four match: reuse stored hashes (for archives: four fields on the **container**; reuse inner hashes).
- Any mismatch: re-hash that file like an initial scan.

**DAT matching (staged)**

- No-Intro, Redump, TOSEC, MAME, custom DATs, in stages.
- User supplies DAT files. Do not bundle copyrighted DAT dumps.

## Organize (later, destructive)

Recommended until decided otherwise: dry-run default, confirm in GUI, quarantine instead of delete.

## External archive media (feature, implementation TBD)

Read-only sources: CD/DVD/BD, removable HDD/SSD, **LTFS LTO**.

In scope later: media health; copy **from** media into the working collection to fill missing files.

**Out of scope for now:** writing to those media (no burning, no tape write, no LTFS format).

## GUI

- Design: Markdown/chat wireframes first, then egui. No Figma.
- Explorer-like layout is enough (tree + table). Native Win32 look is not required.
- Table: `egui_extras::TableBuilder`. Tree: `egui_ltreeview`.
- Candidate screens: Main, Scan (initial/quick/full), DAT, Organize preview, Settings (writes YAML), Report, External media (later).

## Collaboration

| Role | Does |
| --- | --- |
| Codex | Implementation and tests in independent clone `C:\Users\shira\yaRomChecker-codex`, PR labeled `codex`, body includes `Fixes #N` when an issue is completed |
| Cursor | Review in `C:\Users\shira\yaRomChecker`, run tests, accept or request changes |
| Humans / Cursor spec PRs | No `codex` label |

## Open items (do not block the first scaffold PR)

- v1 organize scope (rename+quarantine vs folder sort)
- How much DAT support in the first GUI
- Dry-run / rollback logging details
- i18n crate (`rust-i18n` vs Fluent)
- CI details
- Exact cache file location field names
- External media health metrics
