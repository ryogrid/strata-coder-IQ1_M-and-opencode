# 01 - Overview and Methodology

## Goal

`emur` is a terminal editor driven one byte at a time through the tty
(`src/terminal.rs`, `src/keymap.rs` since fix #27). Its e2e suite
(`test-design/e2e.sh`, 95 scenarios) covers known key paths, but nobody
has tested *arbitrary* byte sequences for crashes, buffer corruption,
and display breakage. This project adds property-based testing (PBT)
with `proptest`:

- **in-process** PBT against the gap buffer (`src/buffer.rs` included via
  `#[path]`) - fast, thousands of cases, differential vs a naive model;
- **tmux-driven** PBT: proptest generates random *key sequences*, the
  harness feeds them to the real editor in a real pty through `tmux`,
  and asserts global invariants (no crash, sane screen, exact buffer
  content via a model replica).

## Why a model replica and not a re-spec

The tmux text-roundtrip property (P2) simulates the op sequence with
**emur's own `Buffer` code** (the same `src/buffer.rs` compiled into the
test crate via `#[path]` include) plus a thin mirror of the dispatch-level
semantics (kill-run accumulation, yank, command -> Buffer method mapping).
This deliberately does NOT hunt for semantic bugs inside `buffer.rs`
algorithm itself (P0 targets those); it hunts for integration-level bugs:

- the real pty byte path (`read_char_eof`, UTF-8 continuation, prefix reads),
- prompt/echo state leaking into the buffer,
- status-line computation vs buffer state,
- save roundtrip (`C-x C-w`) fidelity,
- dispatch mirror drift (a command doing something other than its mapping).

P0 (in-process differential vs a naive `Vec<char>` model) is what hunts
the gap-buffer algorithmic bugs, at 1000x the speed, no tmux.

## Properties (summary; details in 03-properties.md)

| ID | Kind | Target | Main invariant |
|---|---|---|---|
| P0 | in-process | gap buffer ops | text/point/mark == naive model after every op; gap window well-formed (structural, 03 §P0-4) |
| P1 | tmux | random raw bytes (no quit) | session alive, no panic text, no escape litter, status line well-formed |
| P2 | tmux | restricted editing alphabet, seeded file | saved file == model text; status line == model-computed line/total/pct |
| P3 | tmux | window-op random walk | split/collapse boundary structure exact; no buffer-content dashes |
| P4 | tmux | window resizes interleaved with ops | alive + sane screen at every size; status well-formed |

## Scope

- Pure testing work. `emur` behavior changes only when a PBT finding
  requires a fix (then: one finding = one commit, pre-commit gate, full
  gate suite, `found-bugs.md` entry updated with the fix hash).
- No new editor features. No hidden dump commands in v1 (P2 observes
  buffer content through the existing `C-x C-w` save; P2 observes point
  through the existing status-line `(P%)` field).
- No changes to `test-design/e2e.sh` (that suite's harness/scenarios are
  frozen; release_smoke.sh depends on its line ranges).

## Timing budget

tmux properties cost ~2.0-3.5 s/case (per-property table in 04 §6).
Defaults: P1 24 cases, P2 24, P3 16, P4 16 (~3.5 min total, serial). P0 is 512 cases in milliseconds. Case counts scale via
`PBT_CASES` (CI uses 8). Shrinking runs the tmux round-trip per shrunk
case; that is accepted (shrinks are few and short).

## Risk register

| Risk | Mitigation |
|---|---|
| flaky timing (render races) | the settle law (04 §6): 0.6 s session start, 0.8 s after P1/P2 bursts, 0.4 s P2 write-phase/P4 resizes, 0.5 s P3 checkpoints (e2e STEP=0.4 provenance), poll-capture helper (3 x 0.3 s) for status-line checks |
| tmux session collisions | unique names `pbt-<pid>-<seq>`, Drop guard kills, startup sweep of `pbt-*` leftovers, serial rules (04 §5) |
| proptest dep tree + `RUSTFLAGS=-D warnings` on rustc 1.99 | verify at scaffold time; if a third-party crate warns under `-D warnings`, keep `-D warnings` for the emur-root gates and gate pbt-test with plain `cargo build`/`clippy` + zero-warning grep (document the deviation in TODO-PBT.md) |
| model mirror drift (P2) | mapping table in 03 §P2 reviewed against `src/main.rs` dispatch arm-by-arm at design gate and code gate; e2e already pins each command individually |
| CPU contention flakes | `--test-threads=1` for the tmux test binary (proptest 1.x has no worker knobs - gate MA-1), fork-mode cases serial under the single libtest thread, no concurrent suites |
| bug triage cost | every failure: reproduce by hand (replay script), root-cause (model drift vs real bug) recorded in found-bugs.md before any fix |

## Gate discipline (repo conventions that apply here)

- Subagent review gates: design docs (this directory) BEFORE implementation,
  code gate before each fix commit, same as the fix #1-#27 workflow.
- `cargo build` / `cargo test` / `cargo clippy --all-targets` warning-free,
  `#[allow]` forbidden (root AND `pbt-test`).
- One finding = one commit; push per commit; CI green after.
- Never edit a running suite's script (bash re-read semantics).
- tmux suites serialized (e2e / release smoke / PBT binaries).
