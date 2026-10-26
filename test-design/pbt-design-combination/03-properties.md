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
| 1 | `C-r` + q + Enter (found / absent) | the SAME unified isearch mirror, `backward=true` (the old read_line / "Search string not found" op path is GONE - C-r consumes the tail bytes directly) |
| 1 | `C-s` + q + Enter \| `C-s` + q + C-g | isearch machine (`backward=false`) |
| 2 | `C-v` | page down: next_line x (rows-2), column kept, clamp; SILENT, no entry |
| 2 | `M-v` | page up: previous_line x (rows-2) |
| 2 | `C-w` | kill-region (region only when `C-@` set a mark; empty region = SILENT no-op); killed accumulation on C-w runs (tag 2 group) |
| 2 | `C-x` + C-g; ESC + C-g | prefix-abort msg / silent |
| 2 | plain `C-g` (0x07) | `Cmd::Cancel`: msg `Cancelled` (main.rs:922) - the canonical msg-precedence producer on the status row |
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

Setup: seeded files in a SHORT seeded DIRECTORY
`/tmp/e<pid%100000*100+id><tag>/` (`seed.txt`, `a1.txt`, `a2.txt`,
`zzq.txt`, `sub/` - the `a` pair shares prefix `a`; short paths keep the
file/buffer echoes inside the wrap budget, gate J3 as BUILT). The
buffer-list cases get a SECOND buffer `nb` via a fixed `C-x b` + `nb` +
Enter prelude (NOT generated; startup is the single seed buffer, 02 §1).
ECHO BUDGET LAW: the echo string <= `2 * w` cells; over w it WRAPS as
w-char chunks on the trailing pane rows and EVERY chunk row is compared
(02 §4, 05 §10 - the wrapped echo is ON the comparison surface, bounded
to 2 rows; the model asserts the budget before every mid-prompt
comparison).

Ops (enumerated skeletons, parameters generated):

| family | sequence | asserted mid-prompt (capture between Tab and Enter) |
|---|---|---|
| file completion | `C-x C-f` + dir path + `/` + Tab | 0 hits -> bell: echo == prompt+input; 1 hit -> suffix appended; many -> common prefix + `"  " cand cand cand ` list on the echo row |
| file completion 1-hit | `C-x C-f` + dir + `/emurdemo-1` + Tab | suffix `.txt` appears, no list |
| buffer completion | `C-x b` + Tab | candidate list = buffer names (2 -> list echo; caret-span Enter selects) |
| select-from-list | `C-x b` + Tab + `C-b` x j + Enter (j small, span arithmetic) | Enter picks the candidate under the caret (model caret tracking) |
| cancel echo | `C-x C-w` + junk + C-g + Enter | mid cut is BEFORE the C-g (the prompt + typed junk echo - the ` [Cancelled]` suffix is EPHEMERAL, 05 §11); final status row = the PREVIOUS msg RETAINED (write-cancel is silent, main.rs:320 takes no msg branch - the model predicts exactly the retained string from the model msg state) |
| cycle-cancel | `C-x C-b` + C-g + Enter | model: empty return -> CYCLE (listform branch) - the grid shows the cycled buffer; the scripted Enter is a SEPARATE main-loop command (run_tail law): the newline lands at the CYCLED leaf's point (observed divergence, 02 §4) |
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
