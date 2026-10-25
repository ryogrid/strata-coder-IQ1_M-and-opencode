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
honored everywhere):

| layer | file | pub items | compiled by |
|---|---|---|---|
| core (pure) | `support/core.rs` | `PANE_W`, the FOUR `SETTLE_*` constants, `settle`, `cases`, `cfg`, `parse_status`, `pane_tail`, `last_line` (free fn - the `Session::last_line` assoc fn MOVES here; tmux_fuzz call sites adjust) | tmux_fuzz AND combination |
| sess (tmux) | `support/sess.rs` | `Session` (start/send_hex/send_text/resize/capture/capture_bytes/kill/capture_poll), `sweep_leftovers`, `pane_sane` | tmux_fuzz AND combination |
| narrow | `support/narrow.rs` | `pane_sane_lax` ONLY (P4's lax status skip) | tmux_fuzz ONLY |

`support/mod.rs` = core + sess + narrow + re-exports, so `tmux_fuzz.rs`
keeps its `mod support;` + current import paths (edit-free).
`combination.rs` declares `mod core; #[path = "support/sess.rs"] mod
sess;` (NOT narrow) and USES EVERY item of both layers it compiles:

- core: `cfg` (proptest configs), `cases` (used inside cfg, module-internal
  use counts), `settle` + ALL FOUR `SETTLE_*` constants (start/burst/
  phase/checkpoint uses), `PANE_W` (row width asserts), `parse_status`
  (row-23 sanity pre-diff), `pane_tail` + `last_line` (failure reports);
- sess: `Session::start/send_hex/send_text/capture/capture_bytes/kill`,
  `capture_poll` (the P6 echo-poll exception, 05 §8), `sweep_leftovers`
  (used INSIDE `Session::start` - module-internal), `pane_sane` (final
  P5/P7 sanity), `resize` (P7 SUBMATRIX: selected matrix cells run at
  60x20 with the grid model PARAMETERIZED by (w, h) - the renderer
  takes dims as parameters, no constants baked in; a natural,
  deterministic use of the resize path with no P4 overlap).
Private items (`tmux`, `tmux_ok`, `emur_bin`, statics) need NO use map
(gate m1). Budget claim corrected per gate m9: the gates job TODAY is
~8-12 min (build+clippy+unit+e2e ~5-6 + PBT ~4-5 incl. the ~90 s tmux
step), +3-5 min for P5-P7 -> ~12-17 min against `timeout-minutes: 20`;
the tail risk is shrink re-runs (`max_shrink_time` 15 s).

## 2. Runner (`test-design/pbt-run.sh`)

Append a third serial step:

```
echo "== P5-P7 (combination, tmux, serialized)"
cargo test --locked -p pbt-test --test combination -- --test-threads=1 || FAILED=1
```

The serial law extends verbatim: while `pbt-run.sh` runs, no other tmux
suite and no `cargo build` of `target/debug/emur`.

## 3. CI (`github/workflows/ci.yml`)

Extend the PBT steps in the `gates` job (after the tmux_fuzz step):

```
cargo test --locked -p pbt-test --test combination -- --test-threads=1
```

with `PBT_CASES=8` (same env as tmux_fuzz), `EMUR_BIN=../target/debug/emur`
cwd `pbt-test/` (B-1 pattern). Budget: P5 8 cases x <=4 checkpoints +
P6 8 + P7 40 sessions x <=9 ops x ~0.35 s + captures ~= 3-5 min;
current gates job ~5 min -> stays under `timeout-minutes: 20`
(verified at gate time by a full local `pbt-run.sh` timing report in
the ledger).

## 4. Triage convention

Failure text carries the op list + first-differ-row blocks (03 §P5.6),
the seed path (kept: the test does NOT delete seed/out files on
FAILURE, only on success - triage access), and the session is killed by
the Drop guard. Confirmed real bugs are recorded in
`../pbt-design/found-bugs.md` (PBTF-008+, English) with the shrunk
sequence verbatim; fixes get e2e regression pins like PBTF-001..007.
