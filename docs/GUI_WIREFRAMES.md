# GUI wireframes

These low-fidelity wireframes define the first `yaRomChecker` desktop GUI slice described by issue #12. They specify structure and visible behavior, not final styling. The GUI will be a separate `yaRomChecker` binary using the shared core library; this document does not implement that binary.

## Shared application chrome

```text
+------------------------------------------------------------------------------------------------+
| yaRomChecker                                                                                   |
+------------------------------------------------------------------------------------------------+
| [Main] [Scan] [DAT / Verify] [Settings] [Report] [Organize - disabled]                         |
|                                                     [External media - disabled]                |
+------------------------------------------------------------------------------------------------+
| Screen content                                                                                 |
+------------------------------------------------------------------------------------------------+
| Root: C:\ROMs | Last scan: entries 2,013 / hashed 31 / reused 1,209 | Locale: en              |
| Config: C:\...\yaRomChecker.yaml (read-only)                                                   |
+------------------------------------------------------------------------------------------------+
```

- Primary navigation: Main, Scan, DAT / Verify, Settings, and Report; the current screen is selected.
- Organize and External media are visible, disabled navigation placeholders marked as later features.
- The status bar always shows the collection root (or `No collection selected`), last scan entry/hashed-container/reused-container counts (or `No scan yet`), and selected locale.
- The resolved YAML configuration path is always visible, selectable, and read-only.
- Current activity may also appear while work runs. Long values may be truncated if their complete value is available in a tooltip or selectable text.
- No ROM or DAT downloader is offered.

## Main

Main is the collection explorer, not a dashboard-only landing screen.

```text
+-- Main ----------------------------------------------------------------------------------------+
| Collection root: [C:\ROMs____________________________________________________] [Browse...]     |
| [Scan] [Quick] [Full] [Verify]                                                                |
+------------------------------+-----------------------------------------------------------------+
| Folders and containers       | Files and archive entries                                      |
| v C:\ROMs                    | Name     Path              Kind       Size CRC32 MD5 SHA1 Reuse DAT|
|   > Console A                | game.bin C:\ROMs\game.bin loose file 2 MiB ...   ... ...  no    Have|
|   v pack.zip                 | rom.bin  pack.zip/rom.bin  ZIP entry  1 MiB ...   ... ...  yes   Have|
|     > inner                  | disk.bin set.7z/disk.bin   7z entry   4 MiB ...   ... ...  no         |
|   > set.7z                   |                                                                 |
+------------------------------+-----------------------------------------------------------------+
| Selected row                                                                                   |
| Container path: C:\ROMs\pack.zip | Container size: 4 MiB | Container mtime: 2026-09-20 12:00 |
| CRC32: ... | MD5: ... | SHA1: ...                                                            |
+------------------------------------------------------------------------------------------------+
```

- Editable collection-root field and Browse action.
- Explorer tree implemented with `egui_ltreeview`; it shows directories and ZIP/7z containers.
- File table implemented with `egui_extras::TableBuilder`. Columns: name (`entry_name`), path (`entry_path`), kind (`loose file`, `ZIP entry`, or `7z entry`), size, CRC32, MD5, SHA1, cache reused (`yes`/`no`), and DAT file status.
- DAT status after verification is `Have`, `WrongName`, `WrongDump`, `Duplicate`, `Extra`, or empty.
- Selected-row details show container path, container size, container modification time, and the selected entry's CRC32, MD5, and SHA1. A loose file supplies its own container details.
- Scan, Quick, Full, and Verify carry the root to the corresponding screen. Scan selects initial/full mode; Quick and Full select those rescan modes. Work starts only after Start or Verify there.
- Empty state: `Choose a collection root to browse or scan.` Tree, table, and details remain empty.
- Omitted, later: recent-activity dashboard, rename, quarantine, deletion, media health checks, and copying from external media.

## Scan

```text
+-- Scan ----------------------------------------------------------------------------------------+
| Collection root: [C:\ROMs____________________________________________________] [Browse...]     |
| Mode: (o) Initial/full scan  ( ) Quick rescan  ( ) Full rescan                                |
| [Start]                                                                                       |
| Progress: [####################--------------------]                                           |
| Current container: C:\ROMs\pack.zip                                                          |
| Containers hashed: 31 | Containers reused: 1,209 | Entries: 2,013                            |
| Result: 2,013 entries; 31 hashed; 1,209 reused; 0 errors                                     |
| Errors                                                                                        |
| C:\ROMs\broken.zip: unsupported or unreadable archive                                        |
+------------------------------------------------------------------------------------------------+
```

- Collection-root field and Browse action.
- Three mutually exclusive modes: Initial/full scan, Quick rescan, and Full rescan.
  - Initial/full and Full re-hash every loose-file container and ZIP/7z container, streaming inner entries into the hashers.
  - Quick reuses hashes only when container path, file name, size, and modification time match; otherwise it re-hashes the container.
- Start action. Cancel is not required and is omitted; a later implementation may add it as optional.
- Progress shows current container path, hashed-container count, reused-container count, and entry count. Archives are never extracted to disk.
- Completion uses the same count meanings and wording as the CLI scan summary.
- An error list identifies affected paths and messages without discarding successful results.
- Empty state: `Choose a collection root and scan mode.`
- Omitted, later: RAR, standalone `.zst`, header stripping, archive extraction controls, and required cancellation.

## DAT / Verify

```text
+-- DAT / Verify --------------------------------------------------------------------------------+
| Configured sources                                                            [Open Settings] |
| Header name       Version    DAT path                       Collection                         |
| No-Intro Example  2026-09    C:\DATs\No-Intro.dat          C:\ROMs\NES                       |
| TOSEC Example     2026-08    C:\DATs\TOSEC.dat             C:\ROMs\TOSEC                     |
| [Verify]  (quick-scan unique collections, then match each pair)                                  |
| Selected source summary: files 1,240 | present 1,112 | missing 12 | nodump 3              |
| DAT read errors: C:\DATs\broken.dat: line 12: invalid ROM record                              |
|                                                                                                |
| File results   Filters: [Have] [WrongName] [WrongDump] [Duplicate] [Extra]                    |
| Status      Path                    Game              DAT ROM name       baddump               |
| Have        C:\ROMs\game.bin        Example Game      game.bin                                   |
| WrongName   C:\ROMs\GAME2.BIN       Example Game 2    game2.bin                                  |
|                                                                                                |
| DAT ROM results                                                                               |
| State       Game                    Name               baddump                                  |
| Present     Example Game             game.bin                                                   |
| Missing     Example Game 3           game3.bin                                                  |
| nodump      Example Game 4           undumped.bin                                               |
+------------------------------------------------------------------------------------------------+
```

- Read-only ordered source list showing header name, version, DAT path, and paired collection directory. Open Settings edits the list.
- Verify quick-scans each unique collection into the shared cache, then matches each readable DAT only against its paired collection. Two DATs paired with one collection have separate reports backed by the same scan rows.
- Summary is per source and shows collection files, DAT ROMs present, DAT ROMs missing, and DAT ROMs marked `nodump`.
- First table: collection-file status, path, game display title, exact DAT ROM name, and visible `baddump`; filters are Have, WrongName, WrongDump, Duplicate, and Extra.
- Second, separate table: DAT-ROM state (Present, Missing, or `nodump`), game display title, exact name, and visible `baddump`.
- Names compare exactly and case-sensitively; TOSEC-style names remain unchanged. Hash identity takes precedence, and all DAT-provided hashes and optional size must agree.
- With no configured sources, show the localized CLI `dat_missing_config` message in the empty source-list area, offer Open Settings, and disable Verify.
- DAT read errors show their DAT path and diagnostic; readable DATs may still produce results.
- Before verification, both tables show `Run verification to see results.`
- `nodump` is informational, is not Missing, and cannot be filled. `baddump` matches normally and stays visibly flagged.
- Omitted, later: cancellation, DAT downloads/links, DAT editing, MAME/arcade completeness, Redump multi-file sets, CHD, and parent/clone grouping.

## Settings

```text
+-- Settings ------------------------------------------------------------------------------------+
| Configuration file (read-only): C:\...\yaRomChecker.yaml                                      |
| Locale: [en v]    choices: en, ja                                                            |
| Cache path: [cache.sqlite____________________________________________________] [Browse...]     |
| DAT and collection sources                                                                    |
| DAT: [C:\DATs\No-Intro.dat____] [Browse] Collection: [C:\ROMs\NES____] [Browse] [Remove]   |
| DAT: [C:\DATs\TOSEC.dat_______] [Browse] Collection: [C:\ROMs\TOSEC__] [Browse] [Remove]   |
| [Add source]                                                                                  |
| [Save]                                                                                        |
+------------------------------------------------------------------------------------------------+
```

- Read-only resolved configuration-file path.
- Locale selector with `en` and `ja`; English is the default.
- Editable `cache_path` with Browse.
- Ordered `sources` list with a DAT path and collection-directory path in every row, plus Add, Remove, and Browse. Paths may be absolute or YAML-relative.
- Save writes settings to YAML only; scan records and hashes remain in the cache.
- If YAML is missing, Save creates it with the currently displayed settings (including defaults). If it exists, loading preserves its values and Save updates that same file only in response to the explicit Save action; automatic create-if-missing behavior never overwrites existing YAML.
- Missing DAT paths and invalid or unwritable settings appear inline; missing DATs are warnings so other valid settings can still be saved.
- Omitted, later: editing config-file location, themes, automatic DAT downloads, and database-backed settings.

## Report

```text
+-- Report --------------------------------------------------------------------------------------+
| Last scan summary (read-only)                                                                 |
| Entries: 2,013 | Containers hashed: 31 | Containers reused: 1,209 | Errors: 0                 |
| Last DAT summary (read-only)                                                                  |
| Files: 1,240 | Present: 1,112 | Missing: 12 | nodump: 3                                      |
| [Copy summaries]  [Export - later, disabled]                                                  |
| Organize preview: later (disabled placeholder)                                                |
+------------------------------------------------------------------------------------------------+
```

- Read-only last-scan summary: entry, hashed-container, reused-container, and error counts.
- Read-only last-DAT summary: file, present, missing, and `nodump` counts.
- Copy summaries places a plain-English diagnostic summary on the clipboard without ROM contents.
- Export is a disabled later stub. Organize preview is a disabled later placeholder with no rename, quarantine, or delete action.
- Empty state: `No report is available. Run a scan or verification first.`
- Omitted, later: export implementation, report history, automatic sharing, and organize execution.

## Navigation and state notes

- Main actions preselect an operation and navigate; they do not start it immediately.
- Results remain available during the current session. Persistence beyond the scan cache is a later decision.
- Running activity appears in shared status. Navigating away does not imply cancellation.
- Errors attach to affected items where practical; one unreadable item does not erase successful results.
- Keyboard order follows visible layout. Disabled/later controls are skipped.
