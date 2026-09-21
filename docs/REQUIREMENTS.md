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
- Ordered `sources:` entries pair a user-supplied Logiqx XML or ClrMamePro text DAT with its collection directory. Both paths may be relative to the YAML file or absolute. An empty list makes `yarc verify` fail with the missing-source configuration message. Do not bundle copyrighted DAT dumps or provide download links.
- All source collections and explicit CLI scans share the one configured scan cache. Scan caches (hashes, sizes, mtimes) may use SQLite or another derived store. Paths for cache/logs belong in YAML.

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

- Verification quick-scans each unique configured collection directory into the shared cache, then matches each DAT only against scan entries whose canonical container path is that collection directory or a descendant. Collection membership never depends on the DAT file name. Sources sharing a collection produce independent reports from the same scan rows.
- Never flatten ROM rows from multiple DATs into a global fill index. Hash fills, duplicates, extras, missing-in-archive evidence, and set completeness are scoped to one DAT and collection pair.
- User-supplied No-Intro-family and TOSEC-family DATs in Logiqx XML or ClrMamePro text format. Detect the format from the content. Do not bundle DAT dumps, provide download links, or add downloaders.
- Read Logiqx header name, description, version, homepage, and URL. `yarc verify` prints the header name and version when present.
- Support both `game` and `machine` records. Use their description as the display title when present.
- Treat TOSEC-style names containing `()` and `[]` as opaque names; do not parse their tokens.
- A collection file matches a DAT ROM only if **every hash listed on that ROM** agrees with the scan (CRC32, MD5, and SHA1, whichever the DAT provides). Size, if present in the DAT, must also agree. One matching algorithm is not enough when the DAT lists several.
- ROM names use an exact, case-sensitive comparison against `ScanEntry.entry_name`; no case folding or fuzzy matching is performed.
- Hash identity takes precedence over name identity. After a file hash-matches a DAT ROM, its name is compared only with that ROM's name.
- File statuses are `Have` (hash and name match), `WrongName` (hash matches but the name does not, including case-only differences), `WrongDump` (name matches but hashes or size do not), `Duplicate` (a later hash hit for a DAT ROM already filled by the first hit in scan order), and `Extra` (neither identity matches).
- Each DAT ROM is filled by at most one file. It is `Present` when any file hash-matches it. `WrongDump` does not fill a DAT ROM.
- An unfilled ROM is `MissingInArchive` when another ROM with the same exact DAT `game` title is `Present` because it was filled from a ZIP or 7z inner entry. The filling entry's `container_path` is the archive evidence; archive file names are never used to infer games. An unfilled ROM is `Missing` otherwise, including when its sibling was filled by a loose file or when the entire archive is absent.
- Unreadable archives remain scan errors and do not produce a DAT ROM status.
- A ROM marked `nodump` is not missing and cannot be filled. A ROM marked `baddump` still matches normally by all listed hashes and optional size, and reports visibly identify it as a bad dump.
- Group DAT ROM rows by their game display title and report an additional per-game set status. Exclude `nodump` rows from the required ROMs and omit games containing only `nodump` rows from the set list. A set is `Complete` when every required ROM is hash-filled, `Incomplete` when at least one but not every required ROM is hash-filled, and `MissingSet` when none is hash-filled. `Have`, `WrongName`, and `Duplicate` indicate a hash-filled ROM; `Extra`, `WrongDump`, `Missing`, and `MissingInArchive` do not. A hash-filled `baddump` ROM counts as filled while retaining its bad-dump mark.
- DAT-listed `.cue` files are ordinary ROM rows matched by their own name, hashes, and optional size. Cue-sheet contents are not parsed to discover tracks; every track must be listed explicitly in the DAT.

**Dump definitions with a different data model**

The current matcher is **one collection file ↔ one DAT ROM** (hashes + exact name). The following circulated formats are dump databases, but they must **not** be treated as that model. Do not pretend a green Have/Missing report is complete for them until a dedicated issue implements the extra structure.

| Format | Why the model differs | Status |
| --- | --- | --- |
| MAME ListXML (`mame -listxml`) | Machines, `cloneof` / `romof` / `merge`, BIOS sets; completeness is per machine, not per loose file | Remaining |
| MAME software lists (`mame -getsoftlist`) | Separate XML family for software; not generic Logiqx ROM rows | Remaining |
| MAME `-listinfo` (`emulator (` …) | Same brace grammar family as ClrMamePro, but arcade emulator header and set semantics | Remaining |
| FBNeo / HBMAME (and similar arcade DATs) | Often look like Logiqx/CMP, but clone, BIOS, and samples are part of the set | Remaining (arcade model; generic ROM rows are not enough) |
| Redump disc sets | One game is cue + multiple tracks/files, not one ROM file | Per-game set status from DAT ROM rows implemented; cue-sheet parsing, CHD, and GDI remain later |
| TOSEC-ISO | Disc/ISO sets; TOSEC names are opaque, but the unit of matching is the set | Remaining |
| CHD / ListXML `<disk>` | Compressed disc images with CHD hashes, not CRC/MD5/SHA1 of a raw ROM | Remaining |
| No-Intro XSD / parent-clone DATs | Extra ids (`id`, `cloneofid`) and 1G1R parent/clone grouping | Remaining (file-level Logiqx rows may still parse) |
| RomCenter DAT | Older manager format; not the same as ClrMamePro | Remaining |
| Hardware Target Game Database SMDB | Hash table schema, not a Logiqx/CMP datafile | Remaining |

Headered vs headerless dumps, TorrentZip layout, and cue/gdi pairing are **file interpretation** issues, not extra DAT file formats. They stay out of this table.

## Organize (later, destructive)

Recommended until decided otherwise: dry-run default, confirm in GUI, quarantine instead of delete.
Organize planning must use the shared scan-cache rows for a source collection; it must not create per-DAT copies of scanned files.

## External archive media (feature, implementation TBD)

Read-only sources: CD/DVD/BD, removable HDD/SSD, **LTFS LTO**.

In scope later: media health; copy **from** media into the working collection to fill missing files.

**Out of scope for now:** writing to those media (no burning, no tape write, no LTFS format).

## GUI

- Design: Markdown/chat wireframes first, then egui. No Figma.
- See [GUI wireframes](GUI_WIREFRAMES.md) for the primary four-pane window, scan popup, Settings, and Report (issue #19; supersedes the issue #12 Main / Scan / DAT / Verify full-page map).
- Native Win32 styling is not required. The GUI is a separate `yaRomChecker` binary; do not combine it with `yarc`.

**Primary window**

- Top: a menu bar (Scan, Verify, Settings, Report; Organize visible and disabled). No Main / Scan / DAT / Verify tab strip.
- Four panes: top-left configured sources as a tree; bottom-left external media (later placeholder); top-right sets for the selected DAT; bottom-right members of the selected set when it is not a single loose file. A draggable full-height vertical splitter divides the left and right columns. Each column has its own draggable horizontal splitter, and every pane retains a small minimum size. Splitter positions persist for the current session only.
- Set-list columns: name, set status, and a count for each status on that set. Inner-file columns: name, size, mtime, hashes, last-checked date.
- Matching (verify) runs only when the user asks (menu Verify, or equivalent). Selecting a DAT or set does not start a match.
- The DAT tree lists YAML `sources` (each node is a DAT plus its collection directory), not a collection-folder tree. With none configured, show the localized CLI `dat_missing_config` message and a path to Settings.
- Each source title is `{name} ({found}/{total})`, using the DAT header name when present and the DAT path otherwise. `total` includes every DAT ROM, including `nodump`; `found` is zero until that source has been verified, then counts ROMs with `Present` status. If the DAT cannot be loaded and no total is known, show only its name or path.
- A persistent status bar shows the selected source's collection directory (or `No source selected`), the last scan summary (entry, hashed-container, and reused-container counts), the selected locale, and the resolved configuration path. The configuration path is read-only.

**Scan**

- Scan is a modal popup over the four-pane window, not a full-page screen that replaces the explorer.
- Start hashes unique `sources` collection directories into the shared cache. The popup has three modes (initial/full scan, quick rescan, and full rescan) and Start. An extra folder field is optional and writes into the same cache.
- Progress shows the current container path, hashed-container count, reused-container count, and entry count.
- Completion shows a result line with the same count meanings as the CLI scan summary and an error list with affected paths and messages.
- Cancel is not required. If added, it is optional rather than an acceptance requirement.

**Verify**

- Verify runs only from an explicit user action (menu Verify or equivalent). It does not run on DAT or set selection. It matches the selected source, or all sources if the user chooses that, using existing per-source rules.

**DAT display**

- DAT / Verify is not a separate full page. Selecting a source lists that DAT's sets; status and counts appear after the user runs Verify. Matching is per source. The bottom-right pane lists member files when the selected set is not a single loose file.
- The Sets table headers sort Name, displayed Status, Present, Missing, MissingInArchive, and nodump. Repeated clicks on one header cycle ascending, descending, and original DAT order; clicking another header starts ascending. Name sorting is case-sensitive, count sorting is numeric, and blank unverified counts precede numbers in ascending order. Sorting is session-only and does not start Scan or Verify.
- The top-left source title uses ASCII parentheses and slash with no spaces inside the count: for example, `No-Intro Example (12/340)`.
- DAT downloads, download links, and DAT editing are omitted. MAME/arcade completeness, Redump multi-file sets, CHD, and parent/clone grouping remain later work.

**Settings**

- Settings exposes locale choices `en` and `ja`, editable `cache_path`, and an ordered `sources` list whose rows contain DAT and collection paths with Add, Remove, and Browse controls. It also shows the resolved configuration path as read-only.
- Settings can bulk-add the non-recursive `*.dat` and `*.xml` contents of a chosen folder. Each collection path is the chosen collections parent joined with the DAT header name, or the DAT file stem when the name is missing. Only Windows-illegal folder characters (`<>:"/\\|?*`) and trailing dots/spaces are sanitized; directories are not created. Existing DAT paths are skipped, and unreadable DATs are reported without discarding successful additions.
- Save writes settings to YAML only; cache records and hashes never become YAML settings.
- If the YAML file is missing, Save may create it with the displayed/default values. If it exists, automatic default creation must not overwrite it; changes to that file occur only after the user explicitly chooses Save.
- Theme selection, automatic DAT downloads, and editing the configuration-file location are omitted until later.

**Report**

- Report shows the last scan summary and last DAT summary as read-only values.
- It provides copy-to-clipboard for the summaries and may show export as a disabled later stub.
- Organize preview appears only as a disabled later placeholder. Report history, export implementation, and organize execution are omitted until later.

## Collaboration

Flow is **issue → Codex PR → Cursor review/test report → Cursor merges on accept**. Details: [AGENTS.md](../AGENTS.md).

| Role | Does |
| --- | --- |
| Human | Prioritizes work; may still merge or revert |
| Cursor | Opens/refines GitHub issues; does **not** implement product code; as soon as a Codex PR exists, fetches it, runs tests, reviews thoroughly, reports in chat, comments on the PR for Codex, and **merges on an accept verdict** |
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

1. Redump follow-up work (cue-sheet parsing, CHD, and GDI; DAT ROM-row set status is implemented)
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
