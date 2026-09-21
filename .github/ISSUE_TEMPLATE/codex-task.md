---
name: Codex implementation task
about: Feature or fix for Codex to implement and open a PR. Cursor will review and test.
labels: []
---

## Goal

<!-- One sentence. English. -->

## Requirements

- Decided rules are already written in `docs/REQUIREMENTS.md` (and `docs/GUI_WIREFRAMES.md` if the UI changed). This issue does not invent spec that is missing from those files.
- See `docs/REQUIREMENTS.md`
- <!-- Extra constraints, file paths, CLI/GUI notes -->

## Acceptance criteria

- [ ] <!-- Testable outcome -->
- [ ] `cargo test --workspace` passes on Windows
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes

## Out of scope

<!-- What Codex must not do in this issue. -->

## Codex

- One PR, label `codex`, body includes `Fixes #` this issue.
- Work in the Codex clone only. Do not push to `main`.
- Do not redistribute ROM or DAT files. Do not add downloaders.
