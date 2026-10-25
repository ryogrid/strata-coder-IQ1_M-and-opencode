# PBT (Property-Based Testing) Design

English design docs for the property-based testing effort on `emur`.
Progress is tracked (in Japanese) in `../../TODO-PBT.md`; bugs found are
recorded in `found-bugs.md` (English).

## Index

| File | Content |
|---|---|
| `01-overview.md` | Goals, scope, methodology, risk register, gate discipline |
| `02-package-layout.md` | The `pbt-test/` cargo package: deps, workspace wiring, build gates |
| `03-properties.md` | The property catalog (P0-P4): generators, invariants, op alphabets, byte encodings |
| `04-tmux-harness.md` | tmux session mechanics, send/settle/capture protocol, timing, session naming |
| `05-ci-integration.md` | GitHub Actions wiring and case-count policy |
| `found-bugs.md` | Bug log (append-only, English; template at the top) |

## Conventions

- All PBT code lives in `pbt-test/` (new, standalone package). It must pass
  the same warning gates as the root crate (`cargo build`/`clippy --all-targets`
  zero warnings, no `#[allow]`, `RUSTFLAGS=-D warnings` where applicable).
- tmux suites never run concurrently with each other, with the e2e suite,
  with release smoke, or with cargo builds of the binary under test
  (same discipline as the e2e suite; see `04-tmux-harness.md` §5).
- Every bug finding gets an entry in `found-bugs.md` with a minimized
  reproduction BEFORE any fix is implemented. Fixes then follow the repo
  workflow (1 finding = 1 commit, pre-commit subagent gate).
- Design docs, found-bugs and all `pbt-design/` content are English
  (deliberate exception to the Japanese convention of `review/`; the user
  requested English here). `TODO-PBT.md` is Japanese, like the other ledgers.
