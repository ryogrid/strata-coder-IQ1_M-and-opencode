# 02 - The `pbt-test` Package

## Layout

```
pbt-test/
  Cargo.toml          # workspace member "pbt-test", edition 2021
                      # (no package lock: the ROOT Cargo.lock carries the
                      #  proptest tree; CI uses --locked on the root lock)
  src/                # NO lib target. Gate finding MA-2: a lib-crate
                      # #[path] include of emur's buffer.rs trips
                      # clippy::len_without_is_empty (lib crates are linted;
                      # bin crates exempt), and a per-target include without
                      # full use trips rustc dead_code - fatal under the
                      # zero-warning gate. Solution implemented in 03 §P0:
                      # the op list covers EVERY pub API item and the model
                      # reads name/filename -> probe-verified clean.
  tests/
    support/          # tmux harness module; used by tmux_fuzz ONLY
                      # (buffer_model omits mod support;: P0 is tmux-free)
    buffer_model.rs   # P0 (in-process differential)
    tmux_fuzz.rs      # P1-P4 (tmux; all properties in ONE binary)
```

## Dependency and workspace policy

- The root `Cargo.toml` gains `[workspace] members = ["pbt-test"]` (root
  package stays `emur`). One shared `target/` and one shared `Cargo.lock`
  (proptest's tree lands in the root lock - expected, reviewed at commit).
- `proptest = "1"` is the only dependency (dev-dependency of pbt-test).
  No other new crates. `tmux` and the `emur` binary are invoked via
  `std::process::Command` - no Rust tmux lib.
- Root-crate behavior is untouched: root `cargo build` / `cargo test` /
  `cargo clippy` resolve to `emur` ONLY via `default-members = ["."]`
  (probed at the design gate). The EXISTING CI steps need no `-p emur`
  flag and no change (05); the tmux PBT binaries can never enter a
  default parallel `cargo test` (tmux contention rule).

## Reusing emur's Buffer (model replica)

Non-test code of `src/buffer.rs` has **zero `use` statements** (verified;
its `#[cfg(test)] mod tests` is std-only too) - self-contained. Each TEST
TARGET compiles it verbatim:

```rust
// in tests/buffer_model.rs AND tests/tmux_fuzz.rs (gate MA-2: per-target
// include, NOT a shared lib):
#[path = "../../src/buffer.rs"]
mod buffer;
```

Consequences (accepted deliberately):

- the P2 model uses the exact production gap-buffer code, so the model
  cannot drift from the editor's buffer semantics by construction;
- `buffer.rs`'s own `#[cfg(test)]` unit tests get compiled into the
  `pbt-test` test targets too and run a second time under
  `cargo test -p pbt-test` (accepted: harmless duplication, doubles as a
  compile check of the include);
- STANDING CONSTRAINT (gate MA-2): every pub API item of `buffer.rs`
  must stay reachable from the pbt-test targets - the P0 op list includes
  ALL pub functions (`find_forward`/`find_backward` included) and the
  model reads `name`/`filename`. A future emur commit that adds public
  items breaks the pbt gates loudly until the op list grows; that is the
  intended, loud contract (no #[allow], no silent dead_code).
- if `buffer.rs` ever grows `use` paths that break the include, P0/P2
  break loudly at compile time (a design constraint on buffer.rs:
  stay self-contained - noted in 01 risks).

## Build gates for pbt-test

Same rule set as the root crate (AGENT.md), applied per package:

- `cd pbt-test && cargo build --all-targets` prints zero `warning:`.
- `cd pbt-test && cargo clippy --all-targets` prints zero `warning:`
  (own code; dependencies are not linted by clippy).
- `RUSTFLAGS="-D warnings" cargo build --locked --all-targets` inside
  `pbt-test`: run at scaffold time. DESIGN-GATE PROBE (proptest 1.12.0,
  rustc 1.99.1, scratch workspace): **PASS, zero warnings** - the
  fallback below stays DORMANT. Pre-registered in case a future dep
  update trips it (deps compile under the same RUSTFLAGS): the CI gate
  then falls back to plain `cargo build --locked --all-targets` +
  zero-warning grep, and the deviation is recorded in `TODO-PBT.md`.
  The pre-register exists so the gate is never silently weakened later.
- No `#[allow]` anywhere in `pbt-test/` sources.
- Formatting of op tables/encodings mirrors `src/keymap.rs` style
  (documented mapping tables beat clever code).

## Binary path contract

Tests resolve the editor binary as:

1. `$EMUR_BIN` if set (CI and scripts set this; a RELATIVE value
   resolves against the test process CWD = the package dir `pbt-test/`,
   so CI passes `../target/debug/emur` - gate B-1),
2. else `<manifest_dir>/../target/debug/emur`
   (`PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/debug/emur")`).

The harness asserts the path exists with the message
`run: cargo build -p emur --bin emur` (so a missing binary is an
immediate, obvious failure, not a tmux ghost).

## Runner

`test-design/pbt-run.sh` (new, small): builds `emur` debug, runs

```
cargo test -p pbt-test --locked --test buffer_model
cargo test -p pbt-test --locked --test tmux_fuzz -- --test-threads=1
```

sequentially, `PBT_CASES` passthrough, and reports `PBT rc=N`.
It is the single entry point used locally and in CI (05) so the serial
rule (04 §5) is structural, not tribal.
