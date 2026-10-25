# Found Bugs (PBT)

Append-only log of defects discovered by property-based testing.
One entry per finding; update `status`/`fix` fields in place when the
fix lands. Every entry MUST contain a minimized, replayable
reproduction (proptest seed + shrunk op list + replay.sh recipe).

## Template (copy for a new entry)

```
## PBTF-NNN - <short title>

- date: YYYY-MM-DD
- property: P0/P1/P2/P3/P4 (binary + test fn)
- severity: crash | data-corruption | display | hang | ux
- status: OPEN | FIXED <commit-hash> | NOT-A-BUG (justified below)

Summary: <1-3 sentences>

Reproduction:
- EMUR_BIN: <path> (commit <hash>)
- proptest seed: <seed printed by proptest>
- shrunk op sequence: <list of ops / hex bytes>
- replay.sh:
  <exact tmux commands, copy-pasteable>

Observed: <what happened (pane tail, exit code, file diff)>
Expected: <invariant from 03-properties.md, cited>

Analysis: <root cause; file:line; model-drift vs real-editor bug>
Fix: <commit hash + summary, or reason for NOT-A-BUG>
Notes: <follow-ups>
```

---

## PBTF-001 - buffer.rs header comment overstates the gap-window '\0' padding

- date: 2026-10-25
- property: P0 (design-gate probe, BEFORE implementation)
- severity: ux (source-comment documentation)
- status: NOT-A-BUG

Summary: `src/buffer.rs:2-5` claims "Invariant: chars inside the gap
window is `'\0'` PADDING, never live text". The "never live text" half is
the real contract; the "`'\0'` padding" half is aspirational: a gate
probe (insert + delete_forward + delete_backward on production code)
found deleted-char residue inside `[gap_start, gap_end)`.

Observed: non-`'\0'` cells inside the gap window after ordinary edits.
Expected (per comment): all-`'\0'` window.
Analysis: the residue is unreachable through `text()`/`char_at`/`to_index`
(logical-position contract); no editor-visible effect. The comment
overstates the invariant.
Fix: NOT-A-BUG. P0 asserts window WELL-FORMEDNESS (`gap_start <= gap_end`,
`len()` identity) plus accessor/naive-model equivalence (03 §P0), not
window byte content. Softening the comment wording is a docs scrub
candidate (same class as the TODO-261024 closure entry's scrub list).
Notes: found by a design-gate probe, not by a suite run.

---

## PBTF-002 - proptest fork mode is incompatible with the tmux Drop-guard harness

- date: 2026-10-25
- property: P1/P2/P3 (suite run, fork-mode config)
- severity: test-infra
- status: NOT-A-BUG (design deviation, adopted: `timeout = 0`)

Summary: `Config.timeout = 10_000` silently enables proptest FORK mode
(every case runs in a `rusty_fork` child). Four independent damages:
(a) a child killed at the per-case timeout never runs the `Session` Drop
guard -> leaked `pbt-*` sessions; (b) fork children INHERIT the parent's
memory, so every child sees the same `SEQ` counter snapshot -> duplicate
session names -> `tmux new-session failed for pbt-<pid>-N: duplicate
session`; (c) children inherit the parent's completed sweep `Once` ->
they never sweep; (d) a child failure reaches the parent only as
"failed in other process" - the assertion message (ops log, pane tail)
is lost. P1/P2/P3 all failed with these signatures simultaneously.

Observed: `p1_raw_byte_fuzz` panic "tmux new-session failed for
pbt-968462-0"; leaked sessions `pbt-968783-0/1/2`; P2/P3 "Test failed:
failed in other process."; "Ran past the end of the replay."
Expected: in-process cases with full failure messages and Drop cleanup.
Fix: `timeout = 0` (fork disabled) in `support::cfg` (landed 17e6637); cases run
in-process under `--test-threads=1`; a hung case is bounded by the CI
job `timeout-minutes` (05). All four signatures disappeared.
Notes: recorded as a design deviation in 03 §Config plumbing + 04 §1;
the per-case timeout protection is traded for guard reliability.

---

## PBTF-003 - tmux swallows a lone ';' send-keys argument (P2 model mismatch)

- date: 2026-10-25
- property: P2 (found by PBT: seed + op sequence minimized by shrinking)
- severity: test-infra (input channel; NOT emur, NOT tmux-as-editor-host)
- status: NOT-A-BUG (harness fix adopted)

Summary: P2 saved-file content mismatched the model replica; shrinking
minimized to seed `"!"` and ops `[Char('é'), Char(';'), Char('M'), Bs,
Cf, Char('~'), Mb]`. Observed file: `"é!~"`, expected `"é;!~"`. The
input channel was `tmux send-keys -l -- <text>`; live probes proved a
standalone `send-keys -l -- ';'` sends NOTHING (the `;` acts as tmux's
command separator), while `;` INSIDE a longer argument (`A;B`) arrives
fine. emur is not involved: every other byte of the burst survived.

Replay: `tmux new-session -d -s s -x 80 -y 24 sh -c '<bin> /tmp/seed1.txt;
sleep 60'`; send text `1`, then `;`, then `X;Y`; capture shows `1X;Y!`
(not `1;X;Y!`).
Analysis: tmux builds the server command line from client argv with
shell-style quoting; a lone `;` argument is not quoted for the SERVER
parser, so the command splits and `-l --` receives zero keys.
Fix: `Session::send_text` now encodes UTF-8 to hex bytes and uses
`send-keys -H` (hex tokens carry no command syntax; landed 17e6637); P1-P4 all send
every byte through `-H`.
Notes: this is the flagship PBT find - model-equivalence fuzz minimized
to a 7-op reproducer of a tmux-harness quoting trap.

---

## PBTF-004 - split-boundary detection: the dash row is not an all-dashes line

- date: 2026-10-25
- property: P3 (suite runs)
- severity: test-infra (assertion design)
- status: NOT-A-BUG (assertion refined)

Summary: the P3 C-x 2 check "a line trimmed == all '-' (>= 40)" failed
on valid layouts. Live captures showed:
(a) in a NESTED split (`C-x 3` active) the dash ROW spans only the
split column: ~39 dashes at -x 80 (threshold 40 was too high);
(b) the row keeps the column separator and the right column's content
on the same line: `----------...----|@` (all-dashes test fails);
(c) with the ACTIVE window in the right column (`C-x o` then `C-x 2`)
the dashes sit AFTER the `|` (`@...|----...`).
Fix: detect any run of 8 consecutive '-' inside any pane line (seed
text and P3 typed chars contain no '-', so the run is unambiguous;
landed 17e6637).
Collapse check (`C-x 1` -> no dash run, no '|') unchanged.
Notes: emur's rendering was correct in all three captures.

---

## PBTF-005 - sweeping the last session races the first new-session ("server exited unexpectedly")

- date: 2026-10-25
- property: P3 (suite run, first case)
- severity: test-infra
- status: NOT-A-BUG (harness fix adopted)

Summary: the startup sweep killing the tmux server's LAST `pbt-*`
session shuts the server down; the immediately following first
`new-session` client connected to the dying server and failed:
`tmux new-session failed for pbt-971437-0: server exited
unexpectedly.` (tmux then auto-restarted, leaving shrink-created
sessions behind).

Fix: `sweep_leftovers` settles 600 ms after killing sessions, and
`Session::start` retries `new-session` once after a settle on failure
(landed 17e6637).
Notes: harness-level; hits only the FIRST session of a test-binary run.

---

## PBTF-006 - P2 model drift: sync_in clamps MARK to buffer len every loop iteration

- date: 2026-10-25
- property: P2 p2_edit_roundtrip (pbt-test binary)
- severity: data-model mismatch in the TEST model (not emur)
- status: NOT-A-BUG (emur behavior by design, M-15; model fixed)

Summary: saved file `"\nhCüOé/é \"JO2\niK/ \"s [ "` (emur) vs model
`"\nhCüOé/é \"JO2\niK/ \"s ["` - a trailing space the model's final
C-k removed but emur's did not. Shrunk repro (18 successes before it):
seed = `hCüOé/é \"JO2\niK/ \"s x[` and ops
`[Bs, Up, Cy, Cspc, Cp, Cspc, Enter, Cn, Mb, Down, Cf, Char(' '), Mb,
Cd, Cf, Ck, Ck]`.

Reproduction:
- EMUR_BIN: target/debug/emur (commit dcb8373 tree)
- proptest seed: not persisted (failure_persistence=None); the shrunk
  op list above is deterministic - replay: start emur on the seed file,
  send hex `7f 1b5b41 19 00 10 00 0d 0e 1b62 1b5b42 06 20 1b62 04 06
  0b 0b` (one send-keys -H call each), then C-x C-w + path + Enter.
- observed: saved text keeps a trailing space; model: space killed.

Observed (via `tmux display-message -p '#{cursor_x}'` + status P% +
temporary stderr trace): after the C-d at point 21, emur's MARK (24,
set by the space insertion) is 24 > len 23; the NEXT loop iteration's
`sync_in` clamps mark to len (23) writing BOTH leaf and buffer
(main.rs:120-132, the M-15 two-way clamp), so the following C-f's
mark-ahead rule (`mark(23) > point(21) && mark(21) <= line_end(21)`)
jumps point to 23 = end; C-k at end cuts 0 chars; the space survives.
Expected (old model): mark 24 stays > line_end, C-f moves 1 char, C-k
kills the space.
Analysis: emur is self-consistent (the mark clamp is documented M-15
behavior for stale leaves). The P2 dispatch mirror omitted the loop-top
sync_in, so its mark semantics diverged after ANY shrink op with a
mark ahead of the new end (C-d/BS/C-k with mark set by a prior
insert). This is exactly the class of bug PBT is for: 7-op deep
interaction between insert-mark, shrink, clamp, and the mark-ahead
C-f rule.
Fix: model mirrors sync_in: after every P2 op,
`point = min(point, len); mark = min(mark, len)` in `Sim::op`
(landed 17e6637). P2 re-run 3x24 cases green.
Notes: emur unit tests cover sync_in's POINT clamp; the MARK clamp
interaction with forward_char's region rule has no direct unit test -
candidate follow-up if emur ever exposes mark in a unit-testable loop.

---

## PBTF-007 - P1 C-g suffix (2 bytes) can be swallowed by a mid-isearch UTF-8 lead byte

- date: 2026-10-25
- property: P1 p1_raw_byte_fuzz (full suite run #2, 9 successes before)
- severity: test-infra (harness flush sequence)
- status: NOT-A-BUG (harness fix adopted)

Summary: ops `[Ctrl(0x12), Char('L'), Meta(0xf1), Char('+'),
Cx(0xe7)]` + the 2-byte C-g suffix left the session on the
`Reverse find: L+` prompt line: the invariant 4 status-line check
failed. The isearch loop's printable-char path calls
`read_char_eof(c2)` (main.rs:432); `0xE7` is a 3-byte UTF-8 lead and
legally consumed the 2 suffix C-g bytes as continuation bytes (invalid
sequence -> inserts ' '), leaving isearch waiting for the NEXT key.
The pane tail showed the prompt, not the status line.

Replay: start emur (no args), send hex `12` (C-r opens isearch),
`4c 1b f1 2b`, then `18 e7`, then `07 07`; capture: last line
`Reverse find: L+` (prompt) - no status line.
Analysis: emur is self-consistent (read_char_eof consumes exactly the
continuation bytes the lead promises - the same contract P0 pins). The
harness suffix must survive the worst-case consumption: a lead byte
0xF0..0xF7 eats up to 3 bytes.
Fix: P1 suffix is now FOUR `07` bytes (3 consumed + 1 that the open
prompt sees; the extra also covers a pending C-x prefix). Landed
17e6637 with 03 §P1 + 04 §3 doc fold-in.
Notes: found by the full-case suite, not by smoke - the class of bug
only deep runs hit.

(no further entries)
