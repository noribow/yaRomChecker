# Agent instructions (Codex)

This repository is **yaRomChecker**. Source implementation is done by **Codex**. Cursor reviews, tests, and accepts. Do not treat this file as optional.

## Working copies (separate clones)

Do **not** edit the Cursor tree from Codex, and do not run Codex in the Cursor folder. **Do not use git worktrees** for this split. Worktrees share one `.git`, so Codex cannot create branches when its sandbox cannot write into the Cursor repo.

| Role | Directory | Git |
| --- | --- | --- |
| Cursor (review, spec, accept) | `C:\Users\shira\yaRomChecker` | independent clone, usually `main` |
| Codex (implementation) | `C:\Users\shira\yaRomChecker-codex` | independent clone; own `.git`; feature branches here |

Sync only through `origin` (`git fetch` / `git pull` / PR). Launch Codex with `-C C:\Users\shira\yaRomChecker-codex` and do **not** pass `--worktree` or `--add-dir` pointing at the Cursor clone.

## Delivery flow (required)

Product code is **not** written in the Cursor clone. Cursor may draft issues, review PRs, run tests, and change **process/spec docs** only (this file, `docs/REQUIREMENTS.md`, GitHub templates). Implementation, tests in crates, and Codex-labeled PRs come from Codex.

```text
Issue (Cursor/human) → Codex implements + PR → Cursor tests + reviews → report to human → human decides merge
```

### 1. Issue first

- Open a GitHub issue **before** implementation. Use `.github/ISSUE_TEMPLATE/codex-task.md`.
- One primary issue per change. Acceptance criteria must be testable.
- Point at [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md). Do not start coding from chat alone.
- Cursor must **not** implement the issue in `C:\Users\shira\yaRomChecker` (no feature commits, no “quick fix” in core/cli/gui).

### 2. Codex implements and opens a PR

- Read [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md) before writing code.
- Work only in `C:\Users\shira\yaRomChecker-codex`. Feature branch. **Do not push to `main`.**
- Open a pull request into `main`. Label **`codex`**. Body **must** include `Fixes #N` (one primary issue).
- Keep PRs small. Include tests for new behavior.
- `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` must pass on Windows. Reach crates.io (`CARGO_NET_OFFLINE` unset). YAML config uses `serde_yml`.
- Public docs, UI strings, CLI help, and commit/PR text are **English**. Do not hard-code Japanese in source.
- Do not redistribute DAT files or ROM files. Do not add downloaders.

### 3. Cursor reviews and tests (automatic, no implement)

As soon as a Codex PR exists (label `codex`, or a PR this session just opened for Codex), Cursor in `C:\Users\shira\yaRomChecker` **must start a thorough test-and-review pass without waiting for the human to ask**. Do not stop at “PR opened.”

1. `git fetch origin` and check out the PR branch **read-only for product code** (do not add implementation commits).
2. Run at least: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`. Add targeted tests only if the issue’s acceptance criteria are untested **and** then file a follow-up issue for Codex instead of patching the PR yourself.
3. Review against the issue, REQUIREMENTS, and this file. Probe edge cases, regressions, and policy (no ROM/DAT redistribution). Read the matching/parser code; do not rely on green tests alone.
4. Post findings on the GitHub PR when they must reach Codex. **Do not push to the Codex branch.**
5. **Present a review report to the human in chat in the same turn** (required). Do not merge unless the human asked.

Review report (use these headings):

- **Verdict:** accept / request changes / blocked
- **Issue / PR:** links
- **Commands:** exact commands and pass/fail
- **Acceptance criteria:** each issue criterion, met or not
- **Findings:** blocking vs non-blocking
- **Follow-up:** new issues only if needed; Codex iterates on the same PR when changes are requested

### 4. Merge

- Human (or Cursor **only when the human asked to merge**) merges after an accept verdict.
- Spec-only PRs from Cursor: no `codex` label. Still use an issue when the change is more than a typo.

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
- Config: `serde_yml`
- DAT in v1 first slice: user-supplied Logiqx XML if you add matching; otherwise hash + store only is acceptable in the first PR
- Organize, Redump/MAME/CHD, header stripping: later PRs
