# 03 - Properties P5, P6, P7

Common: one binary `tests/combination.rs` (target name `combination`),
`--test-threads=1`, support::cfg, `-H` input, ASCII pool, `0x03`
excluded, no prompt/prefix left dangling at a checkpoint (01 §3),
serial law. Seeds: harness-written `/tmp/emur-pbt-<pid>-<tag>-<seq>`
files (existing pattern law).

## P5 - full-pane differential (the core property)

Setup: seed file (1..4 ASCII lines from a printable+space pool, 1..20
chars/line); `emur <seedfile>` in 80x24.

Ops (each = one FULL keybinding, weighted):

| weight | op (bytes) | model step |
|---|---|---|
| 10 | typing char / space / `Enter` (self-insert, newline; tag 1) | buffer insert (+undo push) |
| 4 | `C-a C-e C-b C-f C-n C-p` motions, arrows, `M-b/M-f`, `M-</M->` | leaf/buffer motion (as P2) |
| 3 | `C-d` `DEL` `C-k` `C-y` `C-o` `C-t` `C-SPC` `C-/` | as P2 (tags 0/2, undo pop) |
| 4 | `C-x 2`, `C-x 3` | split (copy-sibling, active stays) |
| 3 | `C-x o` | next leaf, moved |
| 2 | `C-x 1` | collapse |
| 3 | `C-x b` + `<name>` + Enter (name pool: seed path, `*demo*`, a created name, a NOVEL name -> create path) | switch_buffer |
| 2 | `C-x C-b` + Enter | cycle bi+1 wrap |
| 2 | `C-x b` + Tab + `<k>` + Enter (k in {a-z}: completion 0/1/many branches) | reader+completion model |
| 2 | `C-x b` + C-g | Cancelled |
| 2 | `C-x C-w` + outpath + Enter | Wrote/binding (outpath pattern) |
| 2 | `C-x C-s` | NO prompt (writes the bound filename; unbound -> msg `No file name for this buffer`; success: `Wrote {path}` + modified=false; fail: `Failed to write {path}` - main.rs save) |
| 1 | `C-x C-f` + seedpath (reuse) \| nonexistent (fail msg) + Enter | find_file branches incl. single-leaf split rule |
| 1 | `C-x C-i` + seedpath + Enter | insert_file (+undo entry) |
| 2 | `C-x k` + yes + Enter \| no + Enter (only MODIFIED buffers prompt) | kill_buffer incl. alias reset |
| 2 | `C-l` + digits + Enter (in-range, 0, past-end, non-digit, empty) | goto_line msgs |
| 1 | `C-r` + q + Enter (found / absent) | find_backward |
| 1 | `C-s` + q + Enter \| `C-s` + q + C-g | isearch machine |
| 2 | `C-x` + C-g; ESC + C-g | prefix-abort msg / silent |
| 2 | plain `C-g` (0x07) | `Cmd::Cancel`: msg `Cancelled` (main.rs:832) - the canonical msg-precedence producer on the status row |
| 1 | `C-q` + printable | quote_insert (tag 1) |

Burst: 1..14 ops. Checkpoints: after ops {2, 6, 10} (as available) +
final = max 4 grid comparisons per case; each checkpoint = settle
`SETTLE_CHECK_MS` + `capture()` + FULL-GRID diff (02 §3).
Config: 24 cases (CI: `PBT_CASES=8`).

Invariants (all on the grid):

1. EVERY row: model row == tmux row (after per-line trim_end),
   rows 0..22 = tree frames (boundaries, text, `@` fill), row 23 =
   status/echo line with model label precedence;
2. the model grid is 24 rows x <= 80 cols by construction (the width
   bound rides along for free);
3. liveness/panic/ESC-litter (P1 invariants 1-3) at the final settle;
4. the seed file on disk unchanged (the `C-x C-s` generator rule of 02 §4
   keeps it out of reach: no op writes the seed path);
5. every written file (outpath via `C-x C-w`, and outpath via `C-x C-s`
   on a W-bound buffer) content == the model text AT THE MOMENT OF THE
   LAST write event to that path (the model logs write events);
6. on divergence the failure text prints: op list, checkpoint index,
   first differing row i, `MODEL[rows]` and `TMUX[rows]` blocks
   (6 rows around i), seed path (triage convention).

## P6 - minibuffer machine (mid-prompt included)

Focus: reader-level behaviors a final-state-only property cannot see.

Setup: 3 seeded files in a SHORT seeded DIRECTORY
`/tmp/emur-pbt-<id>-d/` (names `a1.txt`, `a2.txt`, `zzq.txt` - common
prefix `a`) + 2 buffers exist (`*demo*` + seed buffer, via a
fixed `C-x b`/`C-x C-f` prelude, NOT generated). ECHO BUDGET LAW (gate
J3): prompt + input + tab-list <= 80 CELLS (this setup: 11 + ~25 + 2 +
name list <= 60; the model ASSERTS the budget before asserting a
mid-prompt row - wrap is out of the comparison surface, 05 §10).

Ops (enumerated skeletons, parameters generated):

| family | sequence | asserted mid-prompt (capture between Tab and Enter) |
|---|---|---|
| file completion | `C-x C-f` + dir path + `/` + Tab | 0 hits -> bell: echo == prompt+input; 1 hit -> suffix appended; many -> common prefix + `"  " cand cand cand ` list on the echo row |
| file completion 1-hit | `C-x C-f` + dir + `/emurdemo-1` + Tab | suffix `.txt` appears, no list |
| buffer completion | `C-x b` + Tab | candidate list = buffer names (2 -> list echo; caret-span Enter selects) |
| select-from-list | `C-x b` + Tab + `C-b` x j + Enter (j small, span arithmetic) | Enter picks the candidate under the caret (model caret tracking) |
| cancel echo | `C-x C-w` + junk + C-g | echo gains ` [Cancelled]`; final status row = the PREVIOUS msg RETAINED (write-cancel is silent, main.rs:320 takes no msg branch - the model predicts exactly the retained string from the model msg state) |
| cycle-cancel | `C-x C-b` + C-g | model: empty return -> CYCLE (listform branch) - the grid must show the cycled buffer |
| isearch echo | `C-s` + 2 chars (present / absent -> bell) + more chars + DEL (pop) + Enter | final grid (point moved to match END); the isearch header row is only mid-state (final grid carries no header) |
| kill-confirm | modify, `C-x k` + `no`/`yes` + Enter | mid-prompt: `Kill buffer {name}? (yes|no) ` echo; final: Killed/Cancelled + alias geometry |

Config: 16 cases. Every mid-prompt capture = settle + `capture()` +
row-23 diff; every case ends with a committed prompt state (Enter
consumed) so no prompt is open at case end (law).

## P7 - split x prefix matrix (enumerated, deterministic)

Purpose: the user-flagged timing class "C-x prefix command right after a
split / right after C-x o / prefix C-g at split". Enumerated, NOT random:

- prelude patterns (5): `[]`, `[C-x 2]`, `[C-x 3]`, `[C-x 2 C-x 2]`
  (nested), `[C-x 3 C-x o]`;
- command patterns (8): `C-x o`, `C-x 1`, `C-x 2`, `C-x b *demo* Enter`,
  `C-x C-b Enter`, `C-x` + `C-g`, `C-x k yes Enter` (buffer modified
  first by a fixed `x` typing prelude), `C-l 1 Enter`;
- grid compared after EVERY op (matrix size 5x8=40 sessions, bounded).

Config: fixed enumeration (no proptest randomization; runs as a single
`#[test]` that loops the matrix - failure prints the failing
(prelude, command, op index) triple).
