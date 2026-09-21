# yaRomChecker requirements

This is the **only** product-requirements source. Chat, GitHub issues, and Cursor plans are not in force until the same rule is written here. Codex implements against this file. Cursor reviews PRs against it.

This application is created with AI. License: MIT. Repository: https://github.com/noribow/yaRomChecker

Layout and screens: [GUI wireframes](GUI_WIREFRAMES.md). Delivery process: [AGENTS.md](../AGENTS.md).

## Maintaining this document

When a behavior is decided (including while drafting a GitHub issue):

1. Update **this file** so the decided rule is stated in the matching section. If the UI layout changes, update [GUI wireframes](GUI_WIREFRAMES.md) in the same change.
2. Open or refine the issue. Point at this document. Do not leave the decision only in chat or only in the issue body.
3. Codex implements what this file says. If the issue and this file disagree, **this file wins** after Cursor amends it.

Do not delete historical matching rules when adding GUI behavior. Mark later work as later, not as silent omissions.

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
| GUI | Separate binary `yaRomChecker`, **egui + eframe** |
| CLI | Separate binary `yarc`, clap, `--config` |
| Core | Shared library crate `yaromchecker-core` |

Avoid Win32-only APIs where a portable path exists (Linux is planned). Do not commit to macOS. Do not merge GUI and CLI into one binary. Do not add a `--cli` flag on the GUI.

**Implementation defaults:** parallel hashing `rayon`; logging `tracing`; YAML `serde_yml`.

## Configuration (YAML)

Settings are YAML only. **Never store settings in a database.** Scan hashes live in a separate cache (SQLite is allowed). Unknown YAML keys are rejected (including a legacy `dats:` list).

Default file: `yaRomChecker.yaml` in the **same directory as the executable** (not the process cwd). CLI may pass `--config <path>`.

If the file **does not exist**, create it with defaults and then load it. Do not overwrite an existing file. GUI Save may create a missing file with the displayed values; if the file exists, only an explicit Save updates it.

Example shape:

```yaml
locale: en
cache_path: yaRomChecker-cache.sqlite3
sources:
  - dat: C:\DATs\Nintendo\NES.dat
    collection: C:\ROMs\Nintendo\Nintendo - Nintendo Entertainment System
```

| Key | Meaning |
| --- | --- |
| `locale` | `en` (default) or `ja` |
| `cache_path` | Scan cache file (not settings). May be relative to the YAML file or absolute. |
| `sources` | Ordered list of DAT + collection pairs. Empty list is valid YAML; `yarc verify` then fails with `dat_missing_config`. |

Each source row:

| Key | Meaning |
| --- | --- |
| `dat` | User-supplied Logiqx XML or ClrMamePro text DAT. Relative to the YAML file or absolute. |
| `collection` | Directory of locally owned files for that DAT. Relative to the YAML file or absolute. |

Do not bundle DAT dumps or provide download links. Do not add `dat_roots:`, nested `folder` trees, or extra group keys to YAML. Hierarchy in the GUI is derived from `dat` paths.

All source collections and extra CLI scan folders share **one** configured scan cache.

## i18n

- Default locale `en`. User-visible UI, CLI, and logs: English resource strings.
- Switch via YAML `locale` and `locales/en.yaml`, `locales/ja.yaml` (`en` complete; `ja` may be a stub).
- Do not hard-code Japanese in source. Public README and GitHub pages stay English. Optional `README.ja.md` later.

## Archives

- Containers: ZIP and 7z (first class). RAR later. Standalone `.zst` later.
- ZIP: stored, deflate, **Zstandard (APPNOTE method 93)**.
- 7z: include zstd codec when present.
- Pure Rust. Do not require `7z.exe`. Decompress **in memory** into hashers. Do not extract to disk. Do not treat archive *listing* as verification.

## Scanning

Hashes: CRC32, MD5, SHA1 (stream; do not load whole files into RAM when a stream works). Persist at least path, file name, size, mtime, hashes, inner entries for archives, and last-hashed time for GUI “checked”.

**Initial / full scan:** re-hash every loose-file container and ZIP/7z container.

**Quick rescan:** if **file name + path + size + mtime** all match a stored record, reuse hashes. For archives those four fields apply to the **container**; if they match, reuse inner-entry hashes. Any mismatch: re-hash like an initial scan.

`yarc verify` quick-scans each **unique** configured collection into that shared cache, then matches.

## DAT matching

- Match each DAT only against scan entries whose canonical container path is that source’s collection directory or a descendant. Membership never depends on the DAT file name. Sources that share a collection still produce **independent** reports from the same scan rows.
- Never flatten ROM rows from multiple DATs into a global fill index. Hash fills, duplicates, extras, missing-in-archive evidence, and set completeness are scoped to **one** DAT + collection pair.
- User-supplied No-Intro-family and TOSEC-family DATs in Logiqx XML or ClrMamePro text. Detect format from content.
- Read Logiqx header name, description, version, homepage, and URL. `yarc verify` prints header name and version when present.
- Support `game` and `machine`. Use description as display title when present.
- TOSEC-style names with `()` and `[]` are opaque; do not parse tokens.
- A collection file matches a DAT ROM only if **every hash listed on that ROM** agrees (CRC32, MD5, SHA1, whichever the DAT provides). Size, if present, must also agree.
- ROM names: exact, case-sensitive vs `ScanEntry.entry_name`. No case-folding or fuzzy match.
- Hash identity wins over name. After a hash match, compare the name only with that ROM.
- File statuses: `Have`, `WrongName` (including case-only name differences), `WrongDump`, `Duplicate` (later hash hit for a ROM already filled in scan order), `Extra`.
- Each DAT ROM is filled by at most one file. `Present` when any file hash-matches it. `WrongDump` does not fill.
- Unfilled ROM is `MissingInArchive` when another ROM with the same exact DAT `game` title is `Present` from a ZIP/7z inner entry (evidence is that entry’s `container_path`). Otherwise `Missing`.
- Unreadable archives are scan errors and do not produce a DAT ROM status.
- `nodump` is not missing and cannot be filled. `baddump` still matches by listed hashes and optional size, and is identified as a bad dump.
- Per-game set status: group by game display title. Exclude `nodump` from required ROMs; omit games that are only `nodump`. `Complete` / `Incomplete` / `MissingSet` from whether required ROMs are hash-filled. Hash-filled means `Have`, `WrongName`, or `Duplicate`. A filled `baddump` counts as filled.
- DAT-listed `.cue` files are ordinary ROM rows. Cue-sheet contents are not parsed; every track must be listed in the DAT.

**Dump definitions with a different data model** must not be treated as complete Have/Missing reports until a dedicated issue implements them:

| Format | Why the model differs | Status |
| --- | --- | --- |
| MAME ListXML (`mame -listxml`) | Machines, clone/BIOS/merge; completeness per machine | Remaining |
| MAME software lists | Separate XML family | Remaining |
| MAME `-listinfo` | Arcade emulator header / set semantics | Remaining |
| FBNeo / HBMAME (and similar) | Clone, BIOS, samples | Remaining |
| Redump disc sets | Cue + tracks / CHD / GDI | Set status from DAT ROM rows is implemented; cue-sheet, CHD, GDI later |
| TOSEC-ISO | Disc/ISO set as the unit | Remaining |
| CHD / ListXML `<disk>` | CHD hashes, not raw ROM hashes | Remaining |
| No-Intro XSD / parent-clone | ids and 1G1R grouping | Remaining (file-level Logiqx rows may still parse) |
| RomCenter DAT | Not ClrMamePro | Remaining |
| Hardware Target Game Database SMDB | Hash table, not Logiqx/CMP | Remaining |

Headered vs headerless dumps, TorrentZip layout, and cue/gdi pairing are file-interpretation issues, not extra DAT formats. They stay out of that table.

DAT **content** trees (parent/clone, header groups, MAME machines) are **not** the same as the GUI folder tree of DAT **files**. Content grouping is later, separate issues.

## CLI (`yarc`)

Subcommands: initial `scan`, `quick` rescan, `full` rescan (each takes a folder path into the shared cache), and `verify` (configured `sources` only; no extra collection path argument).

Verify with empty `sources` fails with the missing-source message. CLI does **not** print a folder/DAT tree. Hierarchy is GUI-only.

## GUI

Design: Markdown/chat wireframes first, then egui. No Figma. Native Win32 styling is not required.

### Primary window

- Menu bar: Scan, Verify, Settings, Report; Organize visible and **disabled**. No Main / Scan / DAT / Verify tab strip. No ROM/DAT downloader.
- Four panes. One full-height **vertical** splitter between left and right. Each column has its own **horizontal** splitter. Every pane keeps a small minimum size. Splitter positions are **session-only** (not YAML).
- Top-left: configured sources as a **folder + DAT file** tree (below). Bottom-left: external media **placeholder** (later). Top-right: sets for the selected DAT. Bottom-right: members of the selected set when it is not a single loose file.
- Matching runs only when the user asks (menu Verify or equivalent). Selecting a DAT, folder, or set does **not** start Scan or Verify.

### Top-left: Sources tree

- Built from YAML `sources`. Not a ROM-collection folder explorer. Not a DAT-content (clone/group) tree.
- Split each resolved `dat` path on directory separators. Intermediate components are **folder** nodes; the file is a **DAT leaf**. A folder may contain both DAT leaves and child folders.
- Only a **DAT leaf** selects a source and fills the Sets pane. Folder rows expand/collapse only.
- DAT leaf title: `{name} ({found}/{total})` with ASCII parentheses and slash and **no spaces** inside the count (example `No-Intro Example (12/340)`). Name is the DAT header name, or the DAT path if missing. `total` includes every DAT ROM including `nodump`. `found` is `0` until that source is verified, then counts `Present`. Unloadable DATs show name or path with no count.
- Folder rows do **not** show `(found/total)`.
- Do not invent directories that are not a prefix of a configured `dat` path. Empty folders on disk are omitted.
- Unrelated `dat` parents render as multiple roots.
- Expand/collapse is session-only; not YAML.
- Empty `sources`: localized `dat_missing_config` and a way to open Settings. No tree.
- Do **not** walk DAT folders on startup, on paint, or on selection. Disk recursion happens only when the user bulk-adds (or later, an explicit “reload from folder” action).

### Top-right: Sets

- Shown when a DAT leaf is selected. Set names can come from the DAT before Verify; status and counts are placeholders until Verify.
- Columns: Name, Status, Present, Missing, MissingInArchive, nodump. Extra is not a set-row count.
- Left-click a column header to sort that column. Same header cycles **ascending → descending → original DAT order**. A different column starts ascending. Name: case-sensitive lexicographic order. Status: displayed text (including not-verified). Counts: numeric; blank (unverified) counts sort as missing and precede numbers when ascending. Sort is session-only; not YAML. Sorting does not Scan or Verify.
- Selecting a set does not Verify.

### Bottom-right: members

- When the selected set is a ZIP/7z archive or a multi-file set (for example cue + tracks). Hidden or a select-members message for nothing selected or a single loose file.
- Columns: name, size, mtime, CRC32, MD5, SHA1, last-checked (cache hash time). Do not extract archives to disk.

### Status bar

Selected source collection (or `No source selected`), last scan entry / hashed-container / reused-container counts (or `No scan yet`), locale, resolved config path (read-only, selectable).

### Scan (modal)

Modal over the four panes, not a full-page explorer. Modes: initial/full, quick, full rescan. Start hashes unique `sources` collections (optional extra folder writes the same cache). Progress: current container, hashed-container count, reused-container count, entry count. Result line matches CLI scan wording. Error list keeps successful results. Cancel is not required.

### Verify

Explicit user action only. Selected source, or all sources if the user chooses that. Same per-source matching rules as CLI.

### Settings

- Read-only resolved config path. Locale `en` / `ja`. Editable `cache_path` with Browse.
- Ordered `sources` table: DAT path, collection path, Add, Remove, Browse. Manual add does **not** rewrite collection from DAT folders; the left tree still nests those rows by `dat` path.
- **Bulk add** (user-initiated): recurse the chosen DAT folder for non-directory `*.dat` / `*.xml` (extension case-insensitive). Append after existing rows. Sort new files by relative path. Skip DAT paths already in `sources` and count skips. Per-file load errors are listed; keep successful rows. Collection path: `{collections parent}/{relative directory from the DAT folder}/{sanitized header name}` (file stem if no name). Sanitize **each path segment** with Windows-illegal characters `<>:"/\\|?*` and trailing dots/spaces. Do **not** create directories. If the derived collection collides with an existing row or another row in the same batch, skip that DAT, report an error, continue. Do not persist the last DAT-folder / collections-parent pick in YAML.
- Save writes `locale`, `cache_path`, and `sources` only.
- Omitted until later: editing the config-file location, themes, automatic DAT downloads, database-backed settings.

### Report

Last scan summary and last DAT summary, read-only. Copy to clipboard. Export disabled stub. Organize preview disabled placeholder.

## Organize (later, destructive)

Recommended until decided otherwise: dry-run default, confirm in GUI, quarantine instead of delete. Plans must use shared scan-cache rows for a source collection; do not copy scanned files per DAT.

## External archive media (later)

Read-only: CD/DVD/BD, removable HDD/SSD, **LTFS LTO**. Later: health check and copy **from** media into the working collection. **No writing** to those media (no burn, tape write, or LTFS format). Bottom-left pane is a placeholder until then. Health metrics are TBD.

## Out of scope unless a later issue says otherwise

- ROM or DAT downloaders, bundled DAT dumps, download links
- Writing to optical, removable, or LTFS media
- CLI hierarchy display
- Auto-sync of `sources:` from disk on launch
- Checksum-only lists (`.sfv` / `.md5` / `.sha1`), RetroArch `.rdb`, launcher XML (HyperList, LaunchBox, EmulationStation)

## Collaboration

Flow: **update this file → GitHub issue → Codex PR (`codex`, `Fixes #N`) → Cursor review/test report → Cursor merges on accept**. Details: [AGENTS.md](../AGENTS.md).

| Role | Does |
| --- | --- |
| Human | Prioritizes work; may still merge or revert |
| Cursor | Updates this spec; opens issues; does **not** implement product code; reviews, reports, merges on accept |
| Codex | Implements in `C:\Users\shira\yaRomChecker-codex` |

## Open items

- v1 organize scope (rename+quarantine vs folder sort)
- Dry-run / rollback logging details
- i18n crate (`rust-i18n` vs Fluent)
- CI details
- External media health metrics
- Copy-from-media destination / overwrite / dry-run rules
- Header stripping (iNES and similar)
- Sets-header sort indicators (`▲` / `▼`) are optional

## Remaining DAT work (different data models)

Not started except as noted in the matching table. One family per issue when practical. Do not bundle DAT dumps or add downloaders.

Suggested order: Redump follow-up (cue-sheet, CHD, GDI) → No-Intro parent-clone / XSD → MAME ListXML → CHD `<disk>` → MAME software lists → MAME `-listinfo` → FBNeo/HBMAME arcade model → TOSEC-ISO → RomCenter → SMDB.
