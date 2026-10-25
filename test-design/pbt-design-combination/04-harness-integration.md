# 04 - Harness integration (package, support reuse, runner, CI)

## 1. Test target

`pbt-test/tests/combination.rs` (new test target in the EXISTING
`pbt-test` package; no lib target - the MA-2 layout rule stands):

- `#[path]` includes: `../../src/buffer.rs`, `../../src/window.rs`
  (both pure). `screen.rs`/`terminal.rs` are NOT included (I/O-bound:
  `terminal::emit`/`term_size` and the stdin readers; including them
  would drag unused pub I/O items into dead_code territory and the
  stdout-capture problem - the renderer/reader MIRRORS live in this
  target, 02 §3/§4; include decision recorded here for the gate);
- model modules live IN the target file(s): `raster` (grid + renderer),
  `reader` (minibuffer machine), `machine` (Ed mirror). All pub items
  are used by the P5-P7 tests (zero-warning gate, MA-2 per-target rule).

`tests/support/` is restructured into THREE layers (design gate J5
decision, pre-settled here - no artificial uses, MA-2 per-target rule
honored everywhere). As BUILT: the old `narrow.rs` became `sanity.rs`
(the two pane_sane variants plus their shared core live together):

| layer | file | pub items | compiled by |
|---|---|---|---|
| core (pure) | `support/core.rs` | `PANE_W`, the FOUR `SETTLE_*` constants, `settle`, `cases`, `cfg`, `parse_status`, `pane_tail`, `last_line` (free fn - the `Session::last_line` assoc fn is GONE; tmux_fuzz imports `support::last_line`) | tmux_fuzz AND combination |
| sess (tmux) | `support/sess.rs` | `Session` (start/start_size/send_hex/send_text/resize/capture/capture_bytes/kill/capture_poll), `sweep_leftovers`, `tmux`, `tmux_ok`, `emur_bin` | tmux_fuzz AND combination |
| sanity | `support/sanity.rs` | `pane_sane`, `pane_sane_lax`, `pane_sane_core` (P1/P4 sanity probes) | tmux_fuzz ONLY |

`support/mod.rs` = core + sess + sanity + re-exports, so `tmux_fuzz.rs`
keeps its `mod support;` + import paths (only the `last_line` call sites
adjusted). `combination.rs` declares `mod core; #[path = "support/sess.rs"]
mod sess;` (NOT sanity) and USES EVERY item of both layers it compiles:

- core: `cfg` (proptest configs), `cases` (used inside cfg, module-internal
  use counts), `settle` + ALL FOUR `SETTLE_*` constants (start/burst/
  phase/checkpoint uses), `PANE_W` (row width asserts), `parse_status`
  (status-tail check, guarded by the truncation law, 05 §12),
  `pane_tail` + `last_line` (failure reports);
- sess: `Session::start/start_size/send_hex/send_text/capture/capture_bytes/kill`,
  `capture_poll` (4 tries x 300 ms; the P6 echo-poll, 05 §8/§11),
  `sweep_leftovers` (used INSIDE `Session::start` - module-internal),
  `resize` (P7b: 60x20 with the grid model PARAMETERIZED by (w, h) -
  the renderer takes dims as parameters, no constants baked in),
  `tmux`/`tmux_ok`/`emur_bin` (module-internal for `Session`).

`pane_sane` is deliberately NOT compiled by combination: its status-TAIL
parser (P1 law) is INVALID on truncated status rows (long messages cut
at the pane width, 05 §12), and the full-grid diff every checkpoint is
strictly stronger. `combination.rs` runs its own local `comb_sane`
(liveness + no escape litter + width bound).
Private items (`tmux`, `tmux_ok`, `emur_bin`, statics) need NO use map
(gate m1). Budget claim corrected per gate m9: the gates job TODAY is
~8-12 min (build+clippy+unit+e2e ~5-6 + PBT ~4-5 incl. the ~90 s tmux
step), +3-5 min for P5-P7 -> ~12-17 min against `timeout-minutes: 20`;
the tail risk is shrink re-runs (`max_shrink_time` 15 s).

## 2. Runner (`test-design/pbt-run.sh`)

THIRD serial step (as landed):

```
echo "== P5-P7 (full-pane combination, serialized; pbt-design-combination/04)"
cargo test --locked -p pbt-test --test combination -- --test-threads=1 || FAILED=1
```

The serial law extends verbatim: while `pbt-run.sh` runs, no other tmux
suite and no `cargo build` of `target/debug/emur`.

## 3. CI (`github/workflows/ci.yml`)

PBT steps in the `gates` job, after the tmux_fuzz step (as landed):

```
cargo test --locked -p pbt-test --test combination -- --test-threads=1
```

with `PBT_CASES=8` (same env as tmux_fuzz), `EMUR_BIN=../target/debug/emur`
cwd `pbt-test/` (B-1 pattern). Measured locally (full 25-target suite,
`--test-threads=1`): P5 25 cases 59.68 s, P6 14 cases 28.71 s, P7 5x8
matrix 97.83 s, P7b 5.92 s - whole combination target 183.99 s; CI's
`PBT_CASES=8` shrinks P5/P6 (~90 s total). Measured full local
`pbt-run.sh`: P0 5.5 s + P1-P4 164 s + P5-P7 197 s = 6m39s wall incl.
the emur build; gates job estimate stays ~12-17 min against
`timeout-minutes: 20` (timing report in the TODO-PBT ledger).

## 4. Triage convention

Failure text carries the op list + first-differ-row blocks (03 §P5.6),
the seed path (kept: the test does NOT delete seed/out files on
FAILURE, only on success - triage access), and the session is killed by
the Drop guard. Confirmed real bugs are recorded in
`../pbt-design/found-bugs.md` (PBTF-008+, English) with the shrunk
sequence verbatim; fixes get e2e regression pins like PBTF-001..007.
