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

(no further entries)
