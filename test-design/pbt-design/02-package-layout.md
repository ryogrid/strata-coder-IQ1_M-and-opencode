# 02 - The `pbt-test` Package

## Layout

```
pbt-test/
  Cargo.toml          # standalone package "pbt-test", edition 2021
  Cargo.lock          # committed; CI uses --locked
  src/
    lib.rs            # the ONLY lib item: #[path] include of emur's buffer
  tests/
    support/          # shared harness module (mod.rs), used by both test bins
    buffer_model.rs   # P0 (in-process differential; no tmux)
    tmux_fuzz.rs      # P1-P4 (tmux; all properties in ONE binary)
```

## Dependency and workspace policy

- The root `Cargo.toml` gains `[workspace] members = ["pbt-test"]` (root
  package stays `emur`). One shared `target/` and one shared `Cargo.lock`
  (proptest's tree lands in the root lock - expected, reviewed at commit).
- `proptest = "1"` is the only dependency (dev-dependency of pbt-test).
  No other new crates. `tmux` and the `emur` binary are invoked via
  `std::process::Command` - no Rust tmux lib.
- Root-crate behavior is untouched: `cargo build` / `cargo test -p emur`
  keep their meaning. Root gates in CI keep running `-p emur` explicitly
  (05). The root unit-test step must NOT pull the tmux PBT binaries into
  parallel default-thread runs (tmux contention rule).

## Reusing emur's Buffer (model replica)

`src/buffer.rs` has **zero `use`/`mod` statements** (verified) - it is
self-contained over `std`. The test crate compiles it verbatim:

```rust
// pbt-test/src/lib.rs
#[path = "../../src/buffer.rs"]
pub mod buffer;
```

Consequences (accepted deliberately):

- the P2 model uses the exact production gap-buffer code, so the model
  cannot drift from the editor's buffer semantics by construction;
- `buffer.rs`'s own `#[cfg(test)]` unit tests get compiled into
  `pbt-test` too and run a second time under `cargo test -p pbt-test`
  (accepted: harmless duplication, doubles as a compile check of the
  include);
- if `buffer.rs` ever grows `use` paths that break the include, P0/P2
  break loudly at compile time (a design constraint on buffer.rs:
  stay self-contained - noted in 01 risks).

## Build gates for pbt-test

Same rule set as the root crate (AGENT.md), applied per package:

- `cd pbt-test && cargo build --all-targets` prints zero `warning:`.
- `cd pbt-test && cargo clippy --all-targets` prints zero `warning:`
  (own code; dependencies are not linted by clippy).
- `RUSTFLAGS="-D warnings" cargo build --locked --all-targets` inside
  `pbt-test`: run at scaffold time. **If a third-party dependency emits
  an `unsafe_op_in_unsafe_fn`-style warning under rustc 1.99** (deps are
  compiled with the same RUSTFLAGS), the CI gate for pbt-test falls back
  to plain `cargo build --locked --all-targets` + zero-warning grep, and
  the deviation is recorded in `TODO-PBT.md`. This pre-register exists so
  the gate is never silently weakened later.
- No `#[allow]` anywhere in `pbt-test/` sources.
- Formatting of op tables/encodings mirrors `src/keymap.rs` style
  (documented mapping tables beat clever code).

## Binary path contract

Tests resolve the editor binary as:

1. `$EMUR_BIN` if set (CI and scripts set this),
2. else `<manifest_dir>/../target/debug/emur`
   (`concat!(env!("CARGO_MANIFEST_DIR"), "/../target/debug/emur")`).

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
