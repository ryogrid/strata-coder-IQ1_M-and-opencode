#!/usr/bin/env bash
# PBT serial runner - the single entry point (pbt-design/02 + 05).
# Serial law (pbt-design/04 §5): while this runs, NO other tmux suite
# (e2e.sh / release_smoke.sh), NO cargo build of target/debug/emur, and
# NO edits to running scripts may run concurrently.
set -u
cd "$(dirname "$0")/.."
FAILED=0
cargo build --bin emur || { echo "emur build failed"; exit 1; }
echo "== P0 (in-process gap-buffer model)"
cargo test --locked -p pbt-test --test buffer_model || FAILED=1
echo "== P1-P4 (tmux fuzz, one binary, serialized)"
cargo test --locked -p pbt-test --test tmux_fuzz -- --test-threads=1 || FAILED=1
echo "== P5-P7 (full-pane combination, serialized; pbt-design-combination/04)"
cargo test --locked -p pbt-test --test combination -- --test-threads=1 || FAILED=1
echo "PBT FAILED=$FAILED"
exit $FAILED
