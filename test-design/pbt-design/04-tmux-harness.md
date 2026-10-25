# 04 - tmux Harness Protocol

Shared module `tests/support/mod.rs` (compiled into each test binary via
`mod support;` with `#[allow(dead_code)]`-free design: every helper is
used by at least one property IN that binary; helpers unused by one binary
would warn, so split shared code as needed - gate: zero warnings).

## 1. Session lifecycle

- Name: `pbt-<pid>-<atomic-counter>` (parallel test threads within one
  binary cannot collide; the serial rule below limits the count anyway).
- Start (mirrors the e2e harness exactly, plus an exit marker):

  ```
  tmux new-session -d -s NAME -x 80 -y 24 \
    "sh -c '$EMUR_BIN <argv...>; echo EMU_EXITED $?; sleep 30'"
  ```

  The 30 s tail keeps the session (and the pane text incl. `EMU_EXITED`)
  readable after an exit - that is how crashes are distinguished from
  alive sessions: an alive session has NO `EMU_EXITED` line in the pane.
  `<argv...>` is always a fixed-pattern `/tmp/emur-pbt-<id>...` path
  (no shell metacharacters - the single-quote wrapper stays safe).
- Settle after start: 0.6 s (e2e-proven).
- `SessionGuard { name }`: `Drop` runs
  `tmux kill-session -t NAME` (ignores errors). EVERY property creates
  the guard immediately after `new-session` success; panics unwind
  through the guard (proptest catches panics per case, the test thread
  does not abort, Drop runs).
- Startup sweep (once per test process, `std::sync::Once`): list
  sessions (`tmux list-sessions -F '#{session_name}'`), kill every name
  starting with `pbt-` (leftovers from killed runners; this also absorbs
  the sessions leaked by proptest fork-mode children killed on
  `timeout` - their Drop guards never run, 03 Config plumbing).
- The `sh -c '...'` wrapper stays shell-safe only while argv contains no
  `'` characters: true by construction (`/tmp/emur-pbt-<pid>-<id>...`
  fixed pattern, pid <= 7 digits, counter small; outpath ~34 chars,
  worst status line ~56 cols < 80).

## 2. Sending keys

- `send_hex(name, &[u8])` -> `tmux send-keys -t NAME -H <hex>...`
  (one process call per call site; hex from bytes, exact).
- `send_text(name, &str)` -> `tmux send-keys -t NAME -l -- <text>`
  (`--` makes `-l` literal safe for args starting with `-`).
- Ops -> calls: one call per op (03 table). Bursts have NO inter-op
  sleeps (03 header explains the legality per property).
- Settle after a burst: `sleep 0.8 s`; after the P2 write phase: 0.6 s.

## 3. Prompts and state

P2/P3/P4 alphabets contain no prompt-opening commands except the P2
final `C-x C-w` phase, so mid-burst byte consumption by a prompt cannot
happen there. P1 DELIBERATELY allows prompts mid-burst (C-l, C-x b,
C-x k, isearch, ...) - the following burst bytes become prompt input;
that is the fuzz semantics (prompt input handling under random text is
part of what is tested), and the C-g suffix terminates every prompt
path (`read_line_c` Ctrl-g => Cancelled; isearch Ctrl-g => cancel;
confirm-exit prompt gets `""` => not "yes" => stays alive).

## 4. Capturing and asserting

- `capture(name) -> String`: `tmux capture-pane -p -t NAME` (raw bytes
  kept: the escape-litter check inspects the byte string), each line
  `trim_end()` applied for text assertions (matches the e2e `cap`).
- `poll_capture(name, |s| pred)`: 3 attempts, 0.3 s apart (last-resort
  for the status-line regex on slow runners; bursts+settle make it rare).
- Pane width check: tmux 3.4 `capture-pane -p` TRIMS trailing padding
  (gate probe: max line 63/80 on an 80-col pane), so
  `line.chars().count() <= pane width` is a valid upper bound.

## 5. Serial discipline (repo law, extended to PBT)

While ANY tmux suite runs (e2e, release smoke, PBT tmux binary), NOTHING
of the following may run concurrently:

- another tmux-using suite (session-name prefixes are disjoint
  (`emu-e2e-*` vs `pbt-*`) but CPU-load flakes are shared - e2e s133
  history),
- a cargo build of `target/debug/emur` (the binary under test),
- edits to the running script.

Mechanics: `pbt-run.sh` is the single entry point; the `tmux_fuzz`
binary runs with `--test-threads=1` (proptest 1.x has NO `workers` /
`max_concurrent_tests` fields - gate MA-1; fork-mode cases are
per-case serial under the single libtest thread). `buffer_model` (P0)
is tmux-free and may run in parallel (it never touches the binary).
CI runs all suite steps sequentially in one job.

## 6. Timing and budgets

| property | per case (approx) | cases (default / CI) |
|---|---|---|
| P1 | ~2.0 s (0.6 start + ~0.4 burst + 0.8 settle + ~0.2 checks) | 24 / 8 |
| P2 | ~3.2 s (+ write phase ~1.4 s) | 24 / 8 |
| P3 | ~3.0 s (checkpoints) | 16 / 8 |
| P4 | ~3.5 s (resizes) | 16 / 8 |

Full tmux round ~3.5 min at defaults; ~90 s at CI counts. Shrinks re-run
the round-trip (accepted: short minimized sequences shrink fast).

## 7. Failure reporting

On any invariant failure, the harness prints, in this order, into the
assertion message (proptest adds the shrunk `ops` + seed on top):

1. the op list in hex/text form (replayable),
2. `EMU_EXITED` lines if present (exit code!),
3. the full last pane line,
4. the pane tail (last 6 lines, `|`-joined).

`found-bugs.md` entries copy the proptest printed seed AND the minimized
op list, plus a standalone `replay.sh` recipe
(start/ burst/ capture lines) for manual triage.
