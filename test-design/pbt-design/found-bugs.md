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

(no entries yet)
