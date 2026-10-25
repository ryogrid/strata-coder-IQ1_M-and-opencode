# 03 - Property Catalog

Conventions: `ops` is a proptest-generated `Vec<Op>`; every property runs
`start -> burst -> settle -> checkpoints -> kill`. All bursts are sent
**without inter-op sleeps** (the pty queues bytes in order; the editor
consumes them sequentially) - legal only because the P2/P3/P4 alphabets
contain no prompt-opening commands except the final `C-x C-w` phase
(04 §3). Byte encodings for `tmux send-keys -H` (hex) / `-l --` (text).

Char pools (shared):

- ASCII pool: `0x21..0x7e` (letters, digits, punctuation; includes `/`
  never in P2 text ops; includes `-`, allowed in P2 text, forbidden in
  the P3 seed text because `-` rows are boundary markers);
- CJK pool: a fixed small set (e.g. `あいう漢字`), width-2 cells
  (exercises Design Doc C display path);
- newline comes only from the Enter op in P2 (keeps `\n` generation explicit).

## P0 - gap buffer differential (in-process, `tests/buffer_model.rs`)

Model: `pbt_test::buffer::Buffer` (the production `src/buffer.rs` via
`#[path]`) vs a naive reference `RefModel { s: Vec<char>, point, mark }`
implementing the same algorithms naively.

Ops (method-level, direct calls): `insert_char(c)` (pool incl. `' '`
and control chars like `'\u{7}'`), `insert_string(s)`, `delete_forward`,
`delete_backward`, `forward_char`, `backward_char`, `beginning_of_line`,
`end_of_line`, `next_line`, `previous_line`, `beginning_of_buffer`,
`end_of_buffer`, `insert_newline_after`, `transpose_chars`,
`kill_to_end_of_line` (the RETURNED cut string is compared too),
`forward_word`, `backward_word`, `set mark = point` (mirrors `C-SPC`),
`find_forward(pat, from)` / `find_backward(pat, from)` (read-only; pattern
pool = ASCII substrings incl. multi-char and absent patterns).
The op list covers EVERY pub API item of buffer.rs (design gate MA-2):
the model also reads `name` and `filename` (assert == model values after
`Buffer::new`), so no pub item is dead-code-warned in the per-target
include layout (02).

After EVERY op, assert all of:

1. `buf.text() == ref.s` (collect);
2. `buf.point == ref.point`, `buf.mark == ref.mark`;
3. `buf.len() == ref.s.len()`, and `char_at(i) == ref.s[i]` for every
   `i < len` (full sweep - O(n), fine at these sizes);
4. gap-window WELL-FORMEDNESS: `gap_start <= gap_end <= chars.len()` and
   the `len()` identity (`chars.len() - (gap_end - gap_start)` equals the
   naive `s.len()`). NOTE (design gate BL-1): the header comment "chars
   inside the gap window is `'\0'` PADDING" is ASPIRATIONAL - deleted-char
   residue provably lives in the window; the real contract is "never live
   text", i.e. no accessor ever exposes the residue (invariants 1+3 check
   exactly that). Recorded as PBTF-001 (NOT-A-BUG) in found-bugs.md;
5. `kill_to_end_of_line` return value == reference cut;
6. `find_forward`/`find_backward` return values == naive substring search
   (match-END position semantics, `from`-clamping semantics).

Config: 512 cases, `ops` len `1..=200`. No tmux; parallel-safe.
This property is what hunts gap-arithmetic bugs (`move_gap`,
`refill_gap` splice, `to_index` wrap, `delete_*` bounds).

## P1 - raw byte fuzz, crash-free (tmux, `tests/tmux_fuzz.rs`)

Ops: weighted random tokens -

| token | bytes |
|---|---|
| control byte (0x00-0x1f, 0x7f; **0x03 excluded**) | `-H` one byte |
| printable ASCII char | `-l --` text |
| Unicode text (CJK pool + accented, 1..4 chars) | `-l --` |
| C-x prefix + random byte (not 0x03) | `-H 18 xx` |
| Meta prefix + random byte | `-H 1b xx` |
| arrow | `-H 1b 5b 41..44` |
| random valid single UTF-8 scalar (non-ASCII, printable classes only) | `-l --` |

`0x03` is excluded everywhere: it can open the exit-confirm prompt, and
a random answer spelling `yes`/`y` would END the session legitimately,
destroying the liveness invariant (documented, accepted limitation:
the confirm path is pinned by e2e s111 (interactive yes/y) plus
s140/s142 (piped EOF), not fuzzed).

Burst: 1..40 tokens. Suffix: FOUR `-H 07` bytes (C-g). Rationale
(PBTF-007): the flush must survive the worst-case byte consumption of
the state it flushes - a UTF-8 lead byte 0xF0..0xF7 consumed by
`read_char_eof` (e.g. the printable path of the isearch loop) legally
eats up to 3 continuation bytes, so 3 bytes are placeholders that
guarantee the 4th C-g reaches the open prompt; the sequence also
covers a pending C-x prefix (first C-g lands as the C-x second byte
-> "Undefined command" msg; a later one cleans the residue). Settle 0.8 s.

Invariants after settle:

1. liveness: `has-session` ok AND the captured pane does NOT contain
   `EMU_EXITED` (the start command appends `echo EMU_EXITED $?`);
2. no `panicked` substring in the pane;
3. no raw `0x1b` byte in the captured output (escape litter);
4. last non-empty pane line matches `.*/[0-9]+ \([0-9]+%\)$`
   (status row exists and is well-formed; the LABEL is arbitrary msg
   text, not checked);
5. every pane line length <= 80 (pane width).

Config: 24 cases. On failure the shrunk token list is printed
(proptest) and recorded verbatim in `found-bugs.md`.

## P2 - edit roundtrip vs model replica (tmux)

Setup: seed file `/tmp/emur-pbt-<id>-seed.txt` with generated content
(1..4 lines of pool chars, `Write` via `std::fs`); start
`emur <seedfile>`; model = `Buffer::new(seedpath, seedtext)` + a thin
`Ed` mirror (kill-run state).

Ops (each = one dispatch command, byte encodings exact):

| op | bytes | model step (= main.rs dispatch mirror) |
|---|---|---|
| `Char(c)` (ASCII/CJK pool) | `-H <utf-8 hex>` | `self_insert(c)` -> `insert_char(c)` |
| `Enter` | `-H 0d` | `insert_char('\n')` |
| `BS` | `-H 7f` | `delete_backward()` |
| `C-d` | `-H 04` | `delete_forward()` |
| `C-k` | `-H 0b` | `cut = kill_to_end_of_line()`; if last top-level byte was `0x0b` -> `killed.push_str(cut)` else `killed = cut`; last = `0x0b` |
| `C-y` | `-H 19` | `insert_string(&killed)` (then last = `0x19`) |
| `C-a`/`C-e` | `01`/`05` | `beginning_of_line()` / `end_of_line()` |
| `C-b`/`C-f` | `02`/`06` | `backward_char()` / `forward_char()` |
| `C-n`/`C-p` | `0e`/`10` | `next_line()` / `previous_line()` |
| `C-o` | `0f` | `insert_newline_after()` |
| `C-t` | `14` | `transpose_chars()` |
| `C-SPC` | `00` | `mark = point` |
| `M-b`/`M-f` | `1b 62`/`1b 66` | `backward_word()` / `forward_word()` |
| `M-<`/`M->` | `1b 3c`/`1b 3e` | `beginning_of_buffer()` / `end_of_buffer()` |
| arrows U/D/R/L | `1b 5b 41..44` | `previous_line`/`next_line`/`forward_char`/`backward_char` |

After EVERY op the model replays the loop-top `sync_in` clamp
(main.rs:120-132): `point = min(point, len)` AND
`mark = min(mark, len)` (the clamp writes both leaf and buffer;
found-bugs PBTF-006 - skipping it diverges whenever a shrink op
(C-d/BS/C-k) runs while a prior insert put the mark ahead of the new
end: emur's `forward_char` then jumps to the CLAMPED mark, the model
to `point+1`).

Every op records its TOP-LEVEL lead byte as "last top-level byte" (the
loop sets `prev_cmd = b` after `sync_out`): a plain key = its own byte,
meta/arrow ops = `0x1b`, CJK self-insert = the UTF-8 lead byte (>= 0x80),
C-x ops = `0x18`. Only `0x0b` extends a kill-run and only `C-k` produces
it, so the accumulation (`prev_cmd == 0x0b`) is reproduced exactly.

Excluded by design: C-c, C-x C-c (quit), C-g, C-s/C-r/C-l (prompts),
C-q, all window ops, C-x C-f/b/k, C-x C-s. The ONLY prompt command is
the final save phase. Burst: 1..30 ops, no inter-op sleep.

Write phase: `-H 18 17` (C-x C-w), settle 0.4; `-l -- <outpath>`
(`/tmp/emur-pbt-<id>-out.txt`), settle 0.2; `Enter`, settle 0.6.

Invariants:

1. `fs::read_to_string(outpath) == model.text()` (THE core assertion);
2. status last line == `"Wrote <outpath> N/M (P%)"` exactly, where the
   model computes: `M = '\n'-count(text)+1`, `N = '\n'-count(text[..point])+1`,
   `P = point*100/len` (0 when len==0) - mirroring `screen.rs`
   (`st_pct` uses `checked_div().unwrap_or(0)`); the label is `msg`
   (`Wrote <path>`, set by the successful write; e2e-pinned wording);
3. liveness/no-panic/no-escape-litter (as P1);
4. the seed file on disk is UNCHANGED (C-x C-w binds the new path;
   no write touched the seed).

Line-length guard: the id pattern keeps the status line < 80 columns
(documented: pid+counter bounded; outpath ~34 chars).

Config: 24 cases.

## P3 - window-structure walk (tmux)

Seed text: fixed, contains NO `-` and no `|` (e.g.
`alpha beta\ngamma delta\n`), via an existing seed file like P2.

Ops: `SplitBelow 18 32`, `SplitRight 18 33`, `OtherWindow 18 6f`,
`DeleteOtherWindows 18 31`, `Char('a ')` typing (adds 'a'/' ' to the
shared buffer), `Enter 0d`.

Checkpoints (settle 0.5 s each, max 3 per case):

- after a checkpointing `C-x 2`: >= 1 pane line CONTAINING a run of 8+
  consecutive `-` (dash boundary row). The naive `^-{40,}$` line-anchored
  test fails on real layouts: in a nested split the dash row spans only
  its column (~39 chars at -x 80), keeps the `|` separator and the right
  column's text ON the same line, and sits after the `|` when the active
  window is the right column (found-bugs PBTF-004; seed/typed text has
  no `-`, so an 8-dash run is unambiguous). Exact multi-boundary counts
  are a known v1 weakness - existence only;
- after a checkpointing `C-x 3`: >= 1 line containing a `|` column char;
- final phase: always end with `-H 18 31` (C-x 1) + settle, then assert
  NO 8-dash-run line and NO lone-`|` structure line; buffer text
  visible (`alpha` present in the capture).

Plus P1 invariants 1-4. Config: 16 cases, 1..16 ops.

## P4 - resize fuzz (tmux)

Ops: printable typing, `C-n/C-p/C-a/C-e`, `C-x 2`, `C-x o`, `C-x 1`;
resizes `tmux resize-window -x W -y H` with
`W in {10,20,40,80,120,200}`, `H in {4,10,24,50}`, interleaved
(burst, resize, burst, resize, ...; settle 0.4 s after each resize).
The sequence ends with `C-x 1` + `C-g` + settle.

Invariants: P1's 1-4 checked after the FINAL settle only; additionally
every pane line <= current W. (Per-resize liveness check is implicit:
a crash mid-sequence kills the session, and the final `has-session`
fails. `resize-window -y` is kept >= 4: tmux minimum-height edges are
not the editor's contract; 24x80 fallback for non-tty is documented,
the pty path is what is fuzzed.)

Config: 16 cases, 2..5 bursts + 1..4 resizes.

## Config plumbing (all properties)

`ProptestConfig`: `cases = env PBT_CASES (default 24; P0 default 512)`,
`timeout = 0` (NO fork mode - see below), `max_shrink_time = 15_000` ms
(shrinking re-runs the tmux round-trip; iteration counts are otherwise
UNBOUNDED - gate MA-3), `failure_persistence = None` (no
regression-state files in the repo; proptest still prints the failing
seed on stderr - record it in `found-bugs.md`).

Serial law = `--test-threads=1` on the tmux_fuzz binary (04 §5, 05);
proptest 1.x HAS NO `workers`/`max_concurrent_tests` fields (gate MA-1).
DEVIATION (found-bugs PBTF-002): the design's `timeout = 10_000` was
ABANDONED at implementation - a non-zero `timeout` implicitly enables
FORK mode, and fork mode is incompatible with the tmux harness: killed
children skip the Drop guard, children inherit the parent's session-name
counter snapshot (duplicate names -> `new-session` fails), children
inherit the completed sweep `Once` (never sweep), and child failures
reach the parent as "failed in other process" (assertion detail lost).
`timeout = 0` keeps everything in-process: Drop guards run, full
messages, serialization by the single libtest thread. The lost
protection: a hung case has NO per-case kill; the CI job
`timeout-minutes` (05) is the backstop.
