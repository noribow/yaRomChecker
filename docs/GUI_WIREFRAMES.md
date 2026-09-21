# GUI wireframes

These low-fidelity wireframes define the `yaRomChecker` desktop GUI. They specify structure and visible behavior, not final styling. The GUI will be a separate `yaRomChecker` binary using the shared core library; this document does not implement that binary.

This revision (issue #19) replaces the issue #12 / PR #15 map: the primary window is a **menu bar plus four panes**. Scan is a **modal popup** over that window. There is no full-page Scan explorer and no separate full-page DAT / Verify screen.

## Screen map

| Surface | Role |
| --- | --- |
| Primary window | Menu bar + four panes (DAT tree, external media, sets, inner files) |
| Scan popup | Modal over the primary window while a scan runs (and to choose scan mode / Start) |
| Settings | Secondary screen from the menu (same purpose as #12; not a new landing page) |
| Report | Secondary screen from the menu (same purpose as #12; not a new landing page) |
| Organize | Disabled menu item only (later) |
| External media | Bottom-left pane placeholder only (later; no write to media) |

Do not add extra screens (no dashboard-only home, no Scan full page, no DAT / Verify full page).

## Primary window

```text
+------------------------------------------------------------------------------------------------+
| File   Scan   Verify   Settings   Report   Organize (disabled)                                 |
+----------------------------------+-------------------------------------------------------------+
| Sources (DAT + collection)       | Sets                                                        |
| v DATs                            | Name              Status      Present Missing …             |
|     No-Intro Example (12/340)     | Example Game      Complete          2       0               |
|   v Nintendo                     | Example Game 2    Incomplete        1       1               |
|       NES (10/100)               |                                                             |
|     ...                          |                                                             |
| (empty: dat_missing_config)      | Select a DAT in the tree to list its sets.                  |
|<------ draggable horizontal ---->|<---------------- draggable horizontal -------------------->|
| External media (later)           | Inner files                                                 |
| Placeholder. Health check and    | Name     Size  mtime  CRC32 MD5 SHA1 Checked                 |
| copy-from-media: later.          | rom.bin                                                     |
| No write to CD/DVD/BD, USB, or   | disk.bin                                                    |
| LTFS LTO.                        | Select a multi-file or archive set to list members.         |
+----------------------------------+-------------------------------------------------------------+
| Collection: C:\ROMs\NES | Last scan: entries 2,013 / hashed 31 / reused 1,209 | Locale: en    |
| Config: C:\...\yaRomChecker.yaml (read-only)                                                   |
+------------------------------------------------------------------------------------------------+
```

The center `|` is one full-height draggable vertical splitter across the four-pane region. The two horizontal splitters are independently draggable and terminate at that vertical splitter. All four panes keep a small minimum size; splitter positions last for the current session only.

### Menu bar

- **File**: later file-oriented actions as needed; do not add a ROM/DAT downloader.
- **Scan**: opens the scan popup (mode + Start). Progress stays in that popup over the four panes.
- **Verify**: runs DAT matching for sources the user chooses (at least the selected source, or all configured sources). It does not run because a DAT or set was selected.
- **Settings**: opens the Settings screen.
- **Report**: opens the Report screen.
- **Organize**: visible and **disabled**; later feature (rename / quarantine / delete are out of scope here).
- No tab strip of Main / Scan / DAT / Verify. Those full pages are retired.

### Top-left: configured DATs (tree)

- Tree of YAML `sources` nested under YAML **`dat_roots`**. This is not a ROM collection explorer and not a DAT-content (parent/clone / header group) tree.
- Top-level folders are the configured DAT roots (directory name). A directory may contain both DAT files and subfolders of DAT files. Do not show empty directories that are not a prefix of a configured `dat` path under its root. Do not use drive letters as roots unless the user put that path in `dat_roots`.
- Sources not under any root (including when `dat_roots` is empty) sit under `Outside DAT roots`, still without `C:` as a root.
- DAT **leaves** use `{name} ({found}/{total})`, for example `No-Intro Example (12/340)`. The name is the DAT header name, or the DAT path when the header has no name. Found is `0` until Verify has run for that source, then counts `Present` DAT ROMs; total includes all DAT ROMs, including `nodump`. A DAT that failed to load shows only its name or path because no real total is available. Folder rows have no count.
- Selecting a DAT **leaf** fills the top-right set list from that DAT (names). Folder rows only expand or collapse. Selection does **not** start a scan or a match. DAT folders are not re-scanned on launch; Settings **Check for new DATs** or bulk-add walks disk when the user asks.
- Empty state: localized CLI `dat_missing_config` in this pane, with a way to open Settings. No DAT downloaders or bundled DAT files.

### Bottom-left: external media (later placeholder)

- Placeholder only. Later: health check and copy **from** media into the working collection.
- Out of scope now: burning, tape write, LTFS format, or any write to external media.
- Disabled / empty copy explaining later work is enough; do not design a working media browser in this revision.

### Top-right: sets

- Shown when a DAT is selected in the top-left tree.
- Lists that DAT's **sets** (per-game set rows from DAT matching rules in REQUIREMENTS).
- Columns:
  - **Name** — DAT game display title.
  - **Status** — `Complete`, `Incomplete`, or `MissingSet` after a user-run Verify. Before Verify, status and counts are empty or a not-verified placeholder; do not invent a match.
  - **Counts for each status** on that set: at least Present, Missing, MissingInArchive, and nodump (ROM-side). Extra is not a set-row count.
- A left-click on a column header sorts its rows. Repeated clicks on the same header cycle ascending, descending, and the exact original DAT row order; clicking another header starts ascending. Name uses case-sensitive lexicographic order, Status uses its displayed text (including `Not verified`), and counts use numeric order. Blank unverified counts are missing values and precede every number when ascending. This sort state lasts for the current session and does not start Scan or Verify.
- Empty state: `Select a DAT in the tree to list its sets.`
- A left-click on a column header sorts its rows. Repeated clicks on the same header cycle ascending, descending, and the exact original DAT row order; clicking another header starts ascending. Name uses case-sensitive lexicographic order, Status uses its displayed text (including `Not verified`), and counts use numeric order. Blank unverified counts are missing values and precede every number when ascending. This sort state lasts for the current session and does not start Scan or Verify.
- With a DAT selected but Verify not yet run: keep the pane with set names if they can be listed from the DAT alone, or an empty table plus `Run Verify to see statuses.` Do not navigate to another screen.

### Bottom-right: set members

- Shown when the selected top-right set is **not a single loose file** (ZIP/7z archive, or a multi-file set such as cue plus tracks).
- Hidden or showing `Select a multi-file or archive set to list members.` when nothing is selected or the set is one loose file.
- Columns: **name**, **size**, **mtime**, **hashes** (CRC32, MD5, SHA1), **checked** (last time this entry was hashed into the shared cache). Persist that timestamp on scan if the cache does not already have it.
- Do not extract archives to disk.

### Status bar (unchanged role)

- Selected source collection (or `No source selected`), last scan entry / hashed-container / reused-container counts (or `No scan yet`), selected locale.
- Resolved YAML configuration path: always visible, selectable, read-only.
- Current activity may also appear while work runs. Long values may be truncated if the complete value is in a tooltip or selectable text.
- No ROM or DAT downloader.

## Scan popup (modal)

Scan does **not** replace the four panes. The explorer stays visible behind a modal.

```text
+------------------------------------------------------------------------------------------------+
| File   Scan   Verify   Settings   Report   Organize (disabled)                                 |
+----------------------------------+-------------------------------------------------------------+
| Configured DATs                  | Sets                                                        |
| ... (dimmed, still laid out)     | ...                                                         |
+----------------------------------+-------------------------------------------------------------+
| External media (later)           | Inner files                                                 |
| ...                              | ...                                                         |
+----------------------------------+-------------------------------------------------------------+
|                    +---------------- Scan -----------------------+                             |
|                    | Sources: unique collections from YAML       |                             |
|                    | Mode: (o) Initial/full  ( ) Quick  ( ) Full |                             |
|                    | [Start]                                     |                             |
|                    | Progress: [##########----------]            |                             |
|                    | Current container: C:\ROMs\pack.zip         |                             |
|                    | Hashed: 31 | Reused: 1,209 | Entries: 2,013 |                             |
|                    | Result: ... same wording as CLI summary     |                             |
|                    | Errors:                                     |                             |
|                    | C:\ROMs\broken.zip: unreadable archive      |                             |
|                    +---------------------------------------------+                             |
+------------------------------------------------------------------------------------------------+
```

- Default Start hashes every unique `sources` collection into the shared cache (same as `yarc verify`'s scan step). An extra folder field with Browse is optional and writes into that same cache.
- Three mutually exclusive modes: Initial/full scan, Quick rescan, and Full rescan.
  - Initial/full and Full re-hash every loose-file container and ZIP/7z container, streaming inner entries into the hashers.
  - Quick reuses hashes only when container path, file name, size, and modification time match; otherwise it re-hashes the container.
- Start action. Cancel is not required and is omitted; a later implementation may add it as optional.
- While scanning, progress (current container, hashed-container count, reused-container count, entry count) stays in this popup. Archives are never extracted to disk.
- Completion uses the same count meanings and wording as the CLI scan summary.
- An error list identifies affected paths and messages without discarding successful results.
- Empty state before Start: `Choose a scan mode. Start hashes configured source collections.`
- Closing the popup after completion returns to the four-pane window; it does not open a Scan full page.
- Omitted, later: RAR, standalone `.zst`, header stripping, archive extraction controls, and required cancellation.

## Settings (from menu)

Unchanged in purpose from issue #12. Opened from the menu, not from a primary nav tab. The list is YAML `sources` (DAT + collection), not a bare `dats:` path list.

```text
+-- Settings ------------------------------------------------------------------------------------+
| Configuration file (read-only): C:\...\yaRomChecker.yaml                                      |
| Locale: [en v]    choices: en, ja                                                            |
| Cache path: [cache.sqlite____________________________________________________] [Browse...]     |
| DAT roots (Sources tree)                                                                      |
| Root: [C:\DATs________________] [Browse] [Remove]                                             |
| [Add DAT root]  [Check for new DATs]                                                          |
| DAT and collection sources                                                                    |
| Bulk DAT folder: [C:\DATs________________] [Browse] Collections parent: [C:\ROMs___] [Browse]|
| [Add DAT folder]                                                                              |
| DAT: [C:\DATs\No-Intro.dat____] [Browse] Collection: [C:\ROMs\NES____] [Browse] [Remove]   |
| DAT: [C:\DATs\TOSEC.dat_______] [Browse] Collection: [C:\ROMs\TOSEC__] [Browse] [Remove]   |
| [Add source]                                                                                  |
| [Save]                                                                                        |
+------------------------------------------------------------------------------------------------+
```

- Read-only resolved configuration-file path.
- Locale selector with `en` and `ja`; English is the default.
- Editable `cache_path` with Browse.
- Ordered `dat_roots` directory list with Add, Remove, and Browse. Roots are YAML; they do not scan disk by themselves. **Check for new DATs** recurses every configured root using the same skip/error/collection rules as bulk add; relative paths are from each root; collections parent is the bulk-add collections-parent field on this screen.
- Ordered `sources` list with a DAT path and collection-directory path in every row, plus Add, Remove, and Browse. Paths may be absolute or YAML-relative. The primary-window tree places these rows under `dat_roots`.
- Bulk add has folder pickers for a DAT folder and collections parent. It **recurses** for `.dat`/`.xml` files, appends one source per readable DAT, and sets collection to `{collections parent}/{relative DAT directory}/{sanitized header name}` (file stem fallback). Each path segment uses the Windows-illegal character sanitizer. Duplicate DAT paths are skipped. Derived collection collisions and unreadable DATs are reported without discarding other additions. It does not create collection directories. The last folder picks are not saved in YAML.
- Save writes settings to YAML only; scan records and hashes remain in the cache.
- If YAML is missing, Save creates it with the currently displayed settings (including defaults). If it exists, loading preserves its values and Save updates that same file only in response to the explicit Save action; automatic create-if-missing behavior never overwrites existing YAML.
- Missing DAT paths and invalid or unwritable settings appear inline; missing DATs are warnings so other valid settings can still be saved.
- Omitted, later: editing config-file location, themes, automatic DAT downloads, and database-backed settings.

## Report (from menu)

Unchanged in purpose from issue #12. Opened from the menu, not from a primary nav tab.

```text
+-- Report --------------------------------------------------------------------------------------+
| Last scan summary (read-only)                                                                 |
| Entries: 2,013 | Containers hashed: 31 | Containers reused: 1,209 | Errors: 0                 |
| Last DAT summary (read-only)                                                                 |
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

- The four-pane window is always the designed main screen. Scan work is modal over it.
- Settings and Report are secondary; returning from them shows the same four panes.
- Results remain available during the current session. Persistence beyond the scan cache is a later decision.
- Navigating to Settings or Report does not imply scan cancellation (Cancel is not required).
- Errors attach to affected items where practical; one unreadable item does not erase successful results.
- Keyboard order follows visible layout. Disabled/later controls are skipped.
- DAT matching rules (Have, WrongName, set Complete/Incomplete, and so on) stay in REQUIREMENTS; this wireframe does not invent table columns for them.

## Retired from issue #12

- Full-page **Main** collection-folder explorer (folder tree + file table as the home screen).
- Full-page **Scan** as primary navigation.
- Full-page **DAT / Verify** with two result tables as a separate screen.

Those behaviors are either folded into the four panes, the scan popup, Settings, or Report, or left as TBD columns / later work.
