# 05 - CI Integration

## Workspace default-members rule

The root `Cargo.toml` gains:

```toml
[workspace]
members = ["pbt-test"]
default-members = ["."]
```

so every EXISTING root command (`cargo build/test/clippy` at repo root)
keeps operating on `emur` only - the existing CI gate steps need no
change, and the tmux PBT binaries can never sneak into a parallel
default `cargo test`.

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
    EMUR_BIN: "target/debug/emur"
  run: cargo test --locked -p pbt-test --test tmux_fuzz -- --test-threads=1
```

Notes:

- the e2e step already runs `cargo build` (debug `emur` exists) and tmux
  is preinstalled on ubuntu runners;
- `EMUR_BIN` is relative to the job's working dir (repo root) -
  `Command::new` resolves it fine;
- `PBT_CASES=8` keeps the tmux PBT step near 90 s;
- if the `RUSTFLAGS=-D warnings` build trips on a third-party dependency
  warning (pre-registered in 02), this step switches to plain
  `cargo build --locked -p pbt-test --all-targets` and the deviation is
  recorded in `TODO-PBT.md`; the clippy step (lints own crates only)
  keeps `-D warnings` regardless;
- `Cargo.lock` (root, workspace-wide) must be committed with the
  proptest tree; CI uses `--locked`.

## Local entry point

`test-design/pbt-run.sh` (02) is the local mirror of the CI steps.
Full local round: `bash test-design/pbt-run.sh` (defaults, ~4-5 min).
