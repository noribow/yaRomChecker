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
- Optional `dats:` list of user-supplied Logiqx XML or ClrMamePro text DAT paths (relative to the YAML file, or absolute). Used by `yarc verify`. Do not bundle copyrighted DAT dumps or provide download links.
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

**DAT matching**

- User-supplied No-Intro-family and TOSEC-family DATs in Logiqx XML or ClrMamePro text format. Detect the format from the content. Do not bundle DAT dumps, provide download links, or add downloaders.
- Read Logiqx header name, description, version, homepage, and URL. `yarc verify` prints the header name and version when present.
- Support both `game` and `machine` records. Use their description as the display title when present.
- Treat TOSEC-style names containing `()` and `[]` as opaque names; do not parse their tokens.
- A collection file matches a DAT ROM only if **every hash listed on that ROM** agrees with the scan (CRC32, MD5, and SHA1, whichever the DAT provides). Size, if present in the DAT, must also agree. One matching algorithm is not enough when the DAT lists several.
- ROM names use an exact, case-sensitive comparison against `ScanEntry.entry_name`; no case folding or fuzzy matching is performed.
- Hash identity takes precedence over name identity. After a file hash-matches a DAT ROM, its name is compared only with that ROM's name.
- File statuses are `Have` (hash and name match), `WrongName` (hash matches but the name does not, including case-only differences), `WrongDump` (name matches but hashes or size do not), `Duplicate` (a later hash hit for a DAT ROM already filled by the first hit in scan order), and `Extra` (neither identity matches).
- Each DAT ROM is filled by at most one file. It is `Present` when any file hash-matches it. `WrongDump` does not fill a DAT ROM.
- An unfilled DAT ROM is `MissingInArchive` when another ROM with the same exact DAT `game` title is `Present` because it was hash-filled by a ZIP or 7z inner entry. The filling entry's `container_path` identifies the scanned archive; archive file names are never used to infer DAT games. An unfilled ROM is `Missing` when there is no such archive-backed sibling, including when only a loose-file sibling is present or the entire archive is absent.
- Unreadable archives remain scan errors and do not produce a DAT ROM status.
- A ROM marked `nodump` is not missing and cannot be filled. A ROM marked `baddump` still matches normally by all listed hashes and optional size, and reports visibly identify it as a bad dump.

**Dump definitions with a different data model**

The current matcher is **one collection file ↔ one DAT ROM** (hashes + exact name). The following circulated formats are dump databases, but they must **not** be treated as that model. Do not pretend a green Have/Missing report is complete for them until a dedicated issue implements the extra structure.

| Format | Why the model differs | Status |
| --- | --- | --- |
| MAME ListXML (`mame -listxml`) | Machines, `cloneof` / `romof` / `merge`, BIOS sets; completeness is per machine, not per loose file | Remaining |
| MAME software lists (`mame -getsoftlist`) | Separate XML family for software; not generic Logiqx ROM rows | Remaining |
| MAME `-listinfo` (`emulator (` …) | Same brace grammar family as ClrMamePro, but arcade emulator header and set semantics | Remaining |
| FBNeo / HBMAME (and similar arcade DATs) | Often look like Logiqx/CMP, but clone, BIOS, and samples are part of the set | Remaining (arcade model; generic ROM rows are not enough) |
| Redump disc sets | One game is cue + multiple tracks/files, not one ROM file | Remaining |
| TOSEC-ISO | Disc/ISO sets; TOSEC names are opaque, but the unit of matching is the set | Remaining |
| CHD / ListXML `<disk>` | Compressed disc images with CHD hashes, not CRC/MD5/SHA1 of a raw ROM | Remaining |
| No-Intro XSD / parent-clone DATs | Extra ids (`id`, `cloneofid`) and 1G1R parent/clone grouping | Remaining (file-level Logiqx rows may still parse) |
| RomCenter DAT | Older manager format; not the same as ClrMamePro | Remaining |
| Hardware Target Game Database SMDB | Hash table schema, not a Logiqx/CMP datafile | Remaining |

Headered vs headerless dumps, TorrentZip layout, and cue/gdi pairing are **file interpretation** issues, not extra DAT file formats. They stay out of this table.

## Organize (later, destructive)

Recommended until decided otherwise: dry-run default, confirm in GUI, quarantine instead of delete.

## External archive media (feature, implementation TBD)

Read-only sources: CD/DVD/BD, removable HDD/SSD, **LTFS LTO**.

In scope later: media health; copy **from** media into the working collection to fill missing files.

**Out of scope for now:** writing to those media (no burning, no tape write, no LTFS format).

## GUI

- Design: Markdown/chat wireframes first, then egui. No Figma.
- The approved implementation reference is [GUI wireframes](GUI_WIREFRAMES.md), covering Main, Scan, DAT / Verify, Settings, and Report plus shared application chrome.
- Explorer-like layout is enough (tree + table). Native Win32 look is not required.
- Table: `egui_extras::TableBuilder`. Tree: `egui_ltreeview`.
- Candidate screens: Main, Scan (initial/quick/full), DAT, Organize preview, Settings (writes YAML), Report, External media (later).

## Collaboration

Flow is **issue → Codex PR → Cursor review/test report → human merge**. Details: [AGENTS.md](../AGENTS.md).

| Role | Does |
| --- | --- |
| Human | Prioritizes work; decides merge after the Cursor report |
| Cursor | Opens/refines GitHub issues; does **not** implement product code; as soon as a Codex PR exists, fetches it, runs tests, reviews thoroughly, reports in chat, comments on the PR for Codex |
| Codex | Implements in `C:\Users\shira\yaRomChecker-codex`, tests, PR labeled `codex`, body includes `Fixes #N` |
| Spec/docs PRs | Cursor or human; **no** `codex` label |

## Open items (do not block the first scaffold PR)

- v1 organize scope (rename+quarantine vs folder sort)
- How much DAT support in the first GUI
- Dry-run / rollback logging details
- i18n crate (`rust-i18n` vs Fluent)
- CI details
- Exact cache file location field names
- External media health metrics

## Remaining DAT work (different data models)

Not started. Implement only via GitHub issues (one family per issue when practical). Do not bundle DAT dumps or add downloaders.

Suggested order:

1. Redump disc sets (cue + multiple tracks/files)
2. No-Intro parent-clone / XSD ids (1G1R grouping)
3. MAME ListXML (machine completeness, clone, BIOS, merge)
4. CHD / `<disk>` hashes
5. MAME software lists
6. MAME `-listinfo` (`emulator (` header)
7. Arcade set model for FBNeo / HBMAME (clone, BIOS, samples)
8. TOSEC-ISO disc sets
9. RomCenter DAT
10. SMDB hash lists

Checksum-only lists (`.sfv` / `.md5` / `.sha1`), RetroArch `.rdb`, and launcher XML (HyperList, LaunchBox, EmulationStation) are not dump DATs; they are out of this remaining list unless a later issue says otherwise.
