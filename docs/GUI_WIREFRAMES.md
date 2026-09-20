# GUI wireframes

These low-fidelity wireframes define the first `yaRomChecker` desktop GUI slice. They describe structure and visible controls, not final spacing, colors, or typography. The GUI is a separate binary from `yarc` and will share the core library when implemented.

## Shared application chrome

Every screen uses the same frame:

```text
+------------------------------------------------------------------------------+
| yaRomChecker                                                                 |
+------------------------------------------------------------------------------+
| [Main] [Scan] [DAT] [Settings] [Report] [Organize - disabled]                |
|                                      [External media - disabled]             |
+------------------------------------------------------------------------------+
| Screen content                                                               |
|                                                                              |
+------------------------------------------------------------------------------+
| Ready | Config: C:\...\yaRomChecker.yaml (read-only)                         |
+------------------------------------------------------------------------------+
```

Elements:

- Window title: `yaRomChecker`.
- Primary navigation: Main, Scan, DAT, Settings, and Report. The current screen is visibly selected.
- Future navigation: Organize and External media are visible but disabled and marked as later features.
- Status bar: current activity or result summary; progress is shown while work is running.
- Config path: the resolved YAML path is always visible and read-only. Settings can edit values stored in that file, but this path is not editable in the GUI.
- Long paths and status text may be truncated visually, with the complete value available as a tooltip or selectable text.
- Destructive actions are absent from this slice. No ROM or DAT downloader is offered.

## Main

```text
+-- Main ----------------------------------------------------------------------+
| Collection                                                                  |
| Folder: [C:\ROMs____________________________________________] [Browse...]    |
|                                                                             |
| Quick actions                         Last result                            |
| [Initial scan] [Quick rescan]         Files: 1,240   Errors: 0              |
| [Full rescan] [Verify with DATs]      Have: 1,100    Missing: 12            |
|                                                                             |
| Recent activity                                                             |
| 2026-09-20  Quick rescan  C:\ROMs                         Completed          |
| 2026-09-19  Verify       C:\ROMs                         Completed          |
|                                                                             |
| Organize preview: later (disabled)     External media: later (disabled)     |
+------------------------------------------------------------------------------+
```

Elements:

- Collection folder field and Browse action.
- Initial scan, Quick rescan, Full rescan, and Verify with DATs actions. Each opens the corresponding screen with the selected folder carried forward.
- Last-result summary with scan totals and, when verification exists, DAT status totals.
- Recent activity list with timestamp, operation, collection path, and outcome. Empty state: `No scans have been run yet.`
- Organize preview and External media are explicit disabled/later entries; neither can be opened in this slice.
- Omitted/later: file rename, quarantine, deletion, media health checks, and copying from external media.

## Scan

```text
+-- Scan ----------------------------------------------------------------------+
| Collection folder: [C:\ROMs________________________________] [Browse...]     |
| Mode: (o) Initial/full scan  ( ) Quick rescan  ( ) Full rescan              |
|                                                                             |
| [Start scan] [Cancel - enabled only while running]                          |
| Progress: [####################--------------------] 50%                     |
| Current: C:\ROMs\Example.zip                                                |
|                                                                             |
| Folders                         Files / archive entries                      |
| v C:\ROMs                      | Name       Type  Size   CRC32  MD5  SHA1    |
|   > Console A                  | Game.zip   ZIP   4 MiB  ...    ...  ...     |
|   > Console B                  |  `- rom   Entry 2 MiB  ...    ...  ...     |
|                                                                             |
| Summary: 1,240 files; 86 archives; 2,013 entries; 0 errors                  |
| [Open Report]                                                               |
+------------------------------------------------------------------------------+
```

Elements:

- Collection folder field and Browse action.
- Mutually exclusive Initial/full scan, Quick rescan, and Full rescan modes.
  - Initial/full and Full rescan stream and re-hash every loose file and ZIP/7z entry.
  - Quick rescan reuses cached hashes only when container path, file name, size, and modification time all match; otherwise it re-hashes.
- Start scan action and a Cancel action that is disabled when idle.
- Progress indicator, percentage or indeterminate state, and current path. Archives are decompressed to the hashing stream and are never extracted to disk.
- Explorer-style folder tree and results table. The table lists name, loose/archive-entry type, size, CRC32, MD5, SHA1, and per-row error when applicable.
- Completed summary: file, archive, inner-entry, reused-cache, re-hashed, and error counts.
- Open Report action after results exist.
- Empty state: `Choose a collection folder and scan mode.`
- Omitted/later: RAR, standalone `.zst`, header stripping, and archive extraction controls.

## DAT / Verify

The navigation label is `DAT`; the screen heading makes the operation explicit as `DAT / Verify`.

```text
+-- DAT / Verify --------------------------------------------------------------+
| Collection folder: [C:\ROMs________________________________] [Browse...]     |
| User-supplied DATs (from configuration)                     [Open Settings] |
| [x] No-Intro Example.dat   Logiqx XML   No-Intro Example   2026-09           |
| [x] TOSEC Example.dat      CMP text     TOSEC Example      2026-08           |
|                                                                             |
| [Verify] [Cancel - enabled only while running]                              |
| Progress: [################################--------] Matching...             |
|                                                                             |
| Filters: [All v] [Search_________________________________]                   |
| File / entry       DAT title       DAT ROM       Status       Flags         |
| game.bin           Example Game    game.bin      Have                       |
| GAME2.BIN          Example Game 2  game2.bin     WrongName                  |
| --                  Example Game 3  game3.bin     Missing                    |
| bad.bin            Example Game 4  bad.bin       Have         baddump       |
|                                                                             |
| Have 1 | WrongName 1 | WrongDump 0 | Duplicate 0 | Extra 0 | Missing 1     |
| [Open Report]                                                               |
+------------------------------------------------------------------------------+
```

Elements:

- Collection folder field and Browse action.
- Read-only list of configured, user-supplied DAT paths with enabled selection, detected format, header name/description, and version when present. Open Settings changes the configured list.
- Verify and context-sensitive Cancel actions with progress/activity display. Verification performs a quick scan first.
- Result table connecting collection file or archive entry, DAT display title, exact DAT ROM name, status, and flags.
- Filters for All, Have, WrongName, WrongDump, Duplicate, Extra, Missing, nodump, and baddump; text search filters visible rows.
- Summary counts for Have, WrongName, WrongDump, Duplicate, Extra, Missing, nodump, and baddump.
- `nodump` is informational, is not counted as Missing, and cannot be filled. `baddump` is visibly flagged while matching normally.
- Name comparison is exact and case-sensitive. TOSEC-style names are displayed unchanged. Hash identity takes precedence over name identity, and all hashes and optional size supplied by a DAT ROM must agree.
- Empty states: `No DAT files are configured.` with Open Settings, and `Run verification to see results.`
- Omitted/later: DAT downloads or links, editing DAT contents, MAME/arcade set completeness, Redump multi-file sets, CHD, parent/clone grouping, and other data models listed under Remaining DAT work in the requirements.

## Settings

```text
+-- Settings ------------------------------------------------------------------+
| Configuration file (read-only)                                              |
| C:\Program Files\yaRomChecker\yaRomChecker.yaml                              |
|                                                                             |
| General                                                                     |
| Locale: [en v]                                                              |
| Cache path: [cache.sqlite___________________________________] [Browse...]    |
| Log path:   [logs___________________________________________] [Browse...]    |
|                                                                             |
| DAT files                                                                   |
| C:\DATs\No-Intro.dat                                      [Remove]          |
| C:\DATs\TOSEC.dat                                         [Remove]          |
| [Add DAT files...]                                                          |
|                                                                             |
| [Save] [Reload]                                                             |
| Validation: Settings are valid.                                             |
+------------------------------------------------------------------------------+
```

Elements:

- Resolved configuration-file path, read-only.
- Locale selector populated from available resources; English (`en`) is the default.
- Editable cache path and log path, with Browse actions. Relative paths are resolved from the YAML file's directory.
- Ordered list of user-supplied DAT file paths with Add and Remove actions. Paths may be absolute or relative to the YAML file.
- Save writes settings to YAML only. Scan records and hashes remain in the cache, never in YAML.
- Reload discards unsaved field edits after confirmation and reloads the file.
- Inline validation for malformed/unsupported values, unwritable destinations, and missing DAT paths. Missing DATs are warnings so other valid settings can still be saved.
- Unsaved-changes indicator; leaving the screen with edits prompts Save, Discard, or Cancel.
- Omitted/later: changing the configuration-file location, theme controls, automatic DAT downloads, and settings stored in a database.

## Report

```text
+-- Report --------------------------------------------------------------------+
| Report: [Latest verification v]   DAT: [All DATs v]                         |
| Filters: [All statuses v] [Search_______________________]                    |
|                                                                             |
| Summary                                                                     |
| Have 1,100 | WrongName 8 | WrongDump 2 | Duplicate 4 | Extra 25             |
| Present 1,112 | Missing 12 | nodump 3 | baddump 1                           |
|                                                                             |
| Collection path       DAT title       Expected name      Status      Detail |
| C:\ROMs\game.bin      Example Game    game.bin           Have               |
| C:\ROMs\GAME2.BIN     Example Game 2  game2.bin          WrongName  Case    |
| --                    Example Game 3  game3.bin          Missing             |
|                                                                             |
| Selected row details                                                        |
| Size: ...  CRC32: ...  MD5: ...  SHA1: ...  DAT source: ...                 |
| [Copy summary]                                                              |
+------------------------------------------------------------------------------+
```

Elements:

- Report selector for the latest available scan or verification result, plus a DAT selector for combined or individual DAT results.
- Status filter and text search.
- Summary totals for scan errors and all applicable verification states: Have, WrongName, WrongDump, Duplicate, Extra, Present, Missing, nodump, and baddump.
- Explorer-style results table with collection path, DAT title, expected exact name, status, flags, and a concise mismatch/error detail.
- Selected-row details show actual and expected size and available CRC32, MD5, and SHA1 values, plus DAT source/header metadata.
- Copy summary copies a plain-English textual summary suitable for diagnostics without copying ROM contents.
- Empty state: `No report is available. Run a scan or verification first.` with navigation to Scan or DAT.
- Omitted/later: HTML/PDF export, report-history retention policy, automatic upload/sharing, organization actions, and any ROM or DAT file content in a report.

## Navigation and state notes

- Main actions preselect a mode and navigate to Scan or DAT; work starts only after the user confirms with Start scan or Verify.
- Scan and verification results remain available when navigating between screens during the current session. Persistence beyond the scan cache is a later decision.
- A running operation is reflected in the shared status bar. Navigating away does not imply cancellation.
- Errors are attached to the affected row where possible and summarized in the status bar and Report; one unreadable file should not erase already completed results.
- Keyboard order follows the visible top-to-bottom, left-to-right layout. Disabled/later controls are skipped.
