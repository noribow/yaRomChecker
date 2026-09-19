# Agent instructions (Codex)

This repository is **yaRomChecker**. Source implementation is done by **Codex**. Cursor reviews, tests, and accepts. Do not treat this file as optional.

## Working copies (git worktrees)

Do **not** edit the Cursor tree from Codex, and do not run Codex in the Cursor folder.

| Role | Directory | Git branch (typical) |
| --- | --- | --- |
| Cursor (review, spec, accept) | `C:\Users\shira\yaRomChecker` | `main` |
| Codex (implementation) | `C:\Users\shira\yaRomChecker-codex` | feature branches from this worktree |

The Codex directory is a `git worktree` of the same repository (`codex/workspace` was created as the worktree’s starting branch). Create each implementation branch **inside** `yaRomChecker-codex`. Cursor stays on `main` (or review branches) in `yaRomChecker`.

If `main` moved in Cursor, in the Codex worktree run `git fetch` and rebase/merge `origin/main` before starting work.

## Workflow

1. Read [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md) before writing code.
2. Work on a **feature branch**. Open a **pull request** into `main`. **Do not push directly to `main`.**
3. Apply the GitHub label **`codex`** on every PR you open. Do not use that label for non-Codex work.
4. Keep PRs small. One milestone per PR when practical.
5. Include tests for new behavior. `cargo test` and `cargo clippy` must pass on Windows.
6. Public docs, UI strings, CLI help, and commit/PR text are **English**. Do not hard-code Japanese in source.
7. Do not redistribute DAT files or ROM files. Do not add downloaders.

## Layout (required)

Cargo workspace:

- `crates/core` — library: scan, hash, DAT match, cache of scan results, organize *plans*
- `crates/gui` — binary **`yaRomChecker`**: egui + eframe
- `crates/cli` — binary **`yarc`**: clap, global `--config <path>`

Do not merge GUI and CLI into one binary. Do not add a `--cli` flag on the GUI.

## First PR (do this first)

Scaffold the workspace and a **minimal core + CLI** only. No GUI yet (screens are wired in Markdown before egui).

Must include:

- MIT `LICENSE` already exists; keep it.
- `Cargo.toml` workspace, Rust **stable**, edition **2024**.
- Core: recursive scan of a folder; for each file, stream-hash CRC32, MD5, SHA1 (do not load whole files into RAM).
- ZIP and 7z: **decompress in memory (stream into the hasher), do not extract to disk**. Support ZIP stored, deflate, and **ZIP method 93 (zstd)**. Pure Rust (`zip`, `zstd`, `sevenz-rust` or equivalent). No `7z.exe`.
- Persist scan records for later quick rescan: at least **path, file name, size, mtime, hashes**. Not in the YAML config. Cache format: SQLite is allowed (settings must stay YAML-only).
- CLI `yarc`:
  - `--config` optional; default config path is **`yaRomChecker.yaml` next to the executable**, not the process cwd.
  - Subcommand for **initial scan** (full hash) and subcommands or flags for **quick** vs **full** rescan.
  - Quick rescan: if **file name + path + size + mtime** all match a stored record, reuse hashes. Otherwise re-hash like the initial scan. For archives, the four fields apply to the **container file**; if they match, reuse inner-entry hashes.
- Unit tests with small fixtures (tiny zip/7z, including a zstd-compressed ZIP if feasible).
- `locale: en` in example YAML; i18n resources under `locales/` (`en` complete enough for CLI messages; `ja` may be a stub).

## Later (do not invent a full design in the first PR)

- GUI (egui, `egui_extras` table, `egui_ltreeview`) after wireframes exist in docs.
- Organize (rename / quarantine): dry-run by default; no immediate delete.
- External archive media (CD/DVD/BD, USB HDD/SSD, LTFS LTO): health check and copy **from** media to fill local missing files. **No writing to external media.** Implementation of health checks is TBD; do not ship a fake burn/tape-writer.
- Standalone `.zst`, RAR, Linux packaging, macOS.

## Defaults when REQUIREMENTS.md says “undecided”

- Parallel hashing: `rayon`
- Logging: `tracing`
- Config: `serde_yaml`
- DAT in v1 first slice: user-supplied Logiqx XML if you add matching; otherwise hash + store only is acceptable in the first PR
- Organize, Redump/MAME/CHD, header stripping: later PRs
