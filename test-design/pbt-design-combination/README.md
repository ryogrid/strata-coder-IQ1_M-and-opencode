# pbt-design-combination - combination (full-pane) property testing

Index for the combination-test design: closing the coverage gap between
the existing P0-P4 (buffer-level model + structural existence checks) and
the bug-prone areas of **window splits, the minibuffer, and the status
line** by modeling the WHOLE terminal display.

The premise (user-stated, adopted): the model exists to compare
**what the model renders for a key sequence** against **what tmux renders
for the same key sequence**. P0-P4 compare only buffer text and status
TAIL shapes; P5-P7 compare the FULL 80x24 pane grid.

| Doc | Contents |
|---|---|
| [01-overview.md](01-overview.md) | scope, methodology, suite law (serial, settle, op-as-full-keybinding) |
| [02-model.md](02-model.md) | the full-pane model: state machine, renderer mirror, minibuffer machine, msg table, sync rules |
| [03-properties.md](03-properties.md) | P5 (full-pane differential), P6 (minibuffer machine), P7 (split x prefix matrix) |
| [04-harness-integration.md](04-harness-integration.md) | new test target, support-module reuse (MA-2), runner/CI wiring, budgets |
| [05-known-gaps.md](05-known-gaps.md) | non-goals and accepted v1 limitations (style layer, wide chars, cursor, EOF paths) |

Bugs found land in the existing
[`../pbt-design/found-bugs.md`](../pbt-design/found-bugs.md) registry
(PBTF-008 onward, English). Test code lives in `pbt-test/` as a new test
target (`tests/combination.rs`). Progress/ledger = root `TODO-PBT.md`
(Phase G/H).

Relationship to `../pbt-design/`: same package, same harness, same laws
(serial tmux, `--test-threads=1`, `-H` hex input, timeout=0 no-fork);
this directory ONLY adds the full-pane model + combination properties.
Existing P0-P4 are unchanged.
