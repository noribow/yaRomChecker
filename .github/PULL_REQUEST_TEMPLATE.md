## Summary

<!-- What changed and why. English. -->

Fixes #<!-- issue number -->

## Source

- [ ] Produced with **Codex** — add the `codex` label (required for Codex PRs)
- [ ] **Not** Codex (spec, review follow-up, manual) — do **not** add `codex`

## Test plan

- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] New behavior covered by tests

Codex: wait for the Cursor review report before expecting a merge. Do not push to `main`.
