# 05 - CI Integration

## Workspace default-members rule

The root `Cargo.toml` gains:

```toml
[workspace]
members = ["pbt-test"]
default-members = ["."]
```

so every EXISTING root command (`cargo build/test/clippy` at repo root)
keeps operating on `emur` ONLY (design-gate probe: `cargo test` at root
ran the 42 emur unit tests, never the pbt binaries). The existing CI
gate steps need no change and no `-p emur` flag, and the tmux PBT
binaries can never sneak into a parallel default `cargo test`.

## New CI steps (single `gates` job, appended after the e2e step)

Sequential steps in the one job are serial by construction, which keeps
the tmux non-concurrency law (04 §5) for free:

```yaml
- name: pbt-test build (warning-free)
  run: RUSTFLAGS="-D warnings" cargo build --locked -p pbt-test --all-targets
- name: pbt-test clippy (warning-free)
  run: cargo clippy --locked -p pbt-test --all-targets -- -D warnings
- name: P0 gap-buffer model (in-process)
  run: cargo test --locked -p pbt-test --test buffer_model
- name: P1-P4 tmux fuzz (serialized)
  env:
    PBT_CASES: "8"
    EMUR_BIN: "../target/debug/emur"
  run: cargo test --locked -p pbt-test --test tmux_fuzz -- --test-threads=1
```

Notes:

- `EMUR_BIN` is relative to the TEST PROCESS CWD, which `cargo test`
  sets to the package dir (`pbt-test/`), NOT the repo root (gate B-1,
  probed: `EMUR_BIN=target/debug/emur` fails the binary-exists assert,
  `../target/debug/emur` passes); the unset fallback in support is
  `CARGO_MANIFEST_DIR`-based and lands on the same file;

- the debug `emur` exists from the CI **build step** (e2e.sh itself never
  invokes cargo), and tmux is preinstalled on ubuntu runners (the e2e
  step proves it every run);
- `PBT_CASES=8` keeps the tmux PBT step near 90 s;
- if the `RUSTFLAGS=-D warnings` build trips on a third-party dependency
  warning (pre-registered in 02), this step switches to plain
  `cargo build --locked -p pbt-test --all-targets` and the deviation is
  recorded in `TODO-PBT.md`; the clippy step (lints own crates only)
  keeps `-D warnings` regardless;
- `Cargo.lock`: a workspace member has NO package-local lock; the ROOT
  lock gains the proptest tree and must be committed (gate probe
  verified); CI uses `--locked` everywhere. The EXISTING gate steps
  (build/clippy/unit at root, no `-p` flags) stay unchanged and are
  pinned to emur by `default-members = ["."]` (02) - the tmux PBT
  binaries can never join the parallel default-thread unit-test step.

## Local entry point

`test-design/pbt-run.sh` (02) is the local mirror of the CI steps.
Full local round: `bash test-design/pbt-run.sh` (defaults, ~4-5 min).
