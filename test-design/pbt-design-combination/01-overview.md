# 01 - Overview: why a full-pane model, and the suite law

## 1. The coverage gap (facts from pbt-design 03 + the code inventory)

Existing tmux properties assert, at most:

| Property | What is compared today |
|---|---|
| P1 | liveness, no panic, no ESC litter, status TAIL regex, line width |
| P2 | buffer text via final `C-x C-w`; status line `Wrote <path> N/M (P%)` |
| P3 | boundary EXISTENCE: an 8+ dash run / a `|` char (PBTF-004) |
| P4 | liveness + line width at resize; status check LAX below wrap width |

Nothing models: leaf geometry (`top/height/col/width` from
`window::layout`), boundary COUNTS/POSITIONS, `@` end-of-buffer filler
rows, per-leaf `scroll` windows, `msg`-over-`Buf ...` status precedence,
the minibuffer echo row (the physical status row during prompts),
multi-buffer aliasing (split copies share a buffer index with
INDEPENDENT point/mark/scroll), and every minibuffer command
(`C-x b`/`C-x C-b` selection+completion, `C-x C-f`, `C-x C-w`,
`C-x C-i`, `C-x k` yes/no, `C-l`, `C-s`, `C-r`) including mid-prompt
`C-g` and prefix `C-x` + `C-g`.

These are exactly the historical bug areas (window splits, minibuffer
behavior). P5-P7 make the FULL PANE the assertion surface.

## 2. Methodology

For every generated op sequence:

1. The op sequence is sent to real `emur` in a real tmux 80x24 pty
   (identical `-H` hex path as P1-P4).
2. The model runs the SAME ops on its own state
   (`bufs`/`tree`/`active`/`msg`/`killed`/`undo`/prompt effects),
   then RENDERS a full 80x24 char grid with a pure renderer that mirrors
   the `screen.rs` pipeline (row layout rules, `draw_cell` rules,
   `split_lines`/`point_pos`, scroll prepass, status line, minibuffer
   echo).
3. The grid is compared with `tmux capture-pane -p`
   (both sides `trim_end`-per-line, tmux 3.4 trailing-padding semantics)
   at checkpoints (bounded count) and at the final state.
4. Any grid-row difference is a failure: the report prints the op list,
   the row index, and the model/real strings side by side.

The model is a MIRROR (like P2's `Sim`) of `main.rs` + `window.rs` +
`screen.rs` + `terminal.rs` (read_line_c) semantics. `window.rs` and
`buffer.rs` are compiled verbatim via `#[path]` (pure modules, no I/O);
`screen.rs`/`terminal.rs` are mirrored in the target (they are I/O
bound - see 04 §1 for the include decision and the table-sync rules).

## 3. Op = ONE full keybinding (user requirement)

Every generated op is a COMPLETE command: all bytes the command needs,
including prompt input, in one atomic send. No op may leave the editor
in a dangling state (prefix pending, prompt open) at a checkpoint:

- window ops: `C-x 2`, `C-x 3`, `C-x o`, `C-x 1` (single send);
- prompt commands carry their scripted tail: e.g.
  `C-x b` + `<name>` + `Enter`; `C-x k` + `yes|Enter`;
  `C-x C-b` + `Enter` (cycle); `C-l` + `3` + `Enter`;
  `C-s` + `ab` + `Enter` / `C-g`; `C-x b` + `C-g` (mid-prompt cancel);
  `C-x` + `C-g` (prefix abort -> `Undefined command: C-x \x07` msg,
  which RENDERS on the status row with the control char as a SPACE
  per `draw_cell` - an intentional model/real comparison target);
- plain/motion/typing/undo ops: single bytes as in P2 (`C-/` included).

Consequence: every checkpoint is a quiet, fully-settled frame - the
same discipline that made P2 stable.

## 4. Suite law (inherited, unchanged)

- tmux suites strictly SERIAL (e2e / release_smoke / PBT / combination
  never overlap; no `cargo build` of the tested binary during a run);
- `--test-threads=1`, proptest timeout=0 (no fork), Drop guards +
  once-sweep (PBTF-002/005);
- ALL input via `tmux send-keys -H <hex>` (never `-l`, PBTF-003);
- fixed settle after each op; checkpoints add one capture;
- `0x03` (C-c) EXCLUDED everywhere (liveness law: exit-confirm can
  legally end the session); quit commands excluded;
- failure persistence off, `max_shrink_time` 15 s, `PBT_CASES` env
  override (support::cfg reused);
- zero-warning gates unchanged: root + pbt-test
  `RUSTFLAGS=-D warnings` build/clippy, no `#[allow]`.

## 5. Determinism sources

- pool is ASCII printable + space + `\n` (wide chars are a v1 non-goal,
  05 §2) -> `char_width` = 1 for the pool, so the renderer mirror has
  NO width tables to duplicate (padding = char count = cell count);
- seeds come from fixed test-written files under `/tmp/emur-pbt-*`
  (paths match the existing `Session::start` arg pattern law);
- no randomness inside the model (mirrors are deterministic by
  construction); `fs` reads the model performs (`C-x C-f`/`C-x C-i`)
  read the same files the harness wrote.
