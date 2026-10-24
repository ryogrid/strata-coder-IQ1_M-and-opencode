#!/usr/bin/env bash
# Release-build smoke (R-2): the debug-only e2e lets release-only bugs
# (integer-overflow wrapping, optimized-code behavior drift) pass silently.
# This runs a representative scenario subset against target/release/emur:
#   s02  editing + status
#   s103 file write via C-x C-w + Tab-completed reopen + highlighting
#   s110 incremental search (C-s)
#   s112 go-to-line incl. the 99 clamp
#   s141 15-digit go-to-line (the debug-overflow / release-wrap case)
#   s140 EOF exit-confirm (pipe runner)
# Prereq: cargo build --release has run (see header below; it rebuilds if
# the binary is older than the sources). Usage: bash test-design/release_smoke.sh
# NOTE: shares the emu-e2e-* tmux session prefix with e2e.sh - do NOT run
# it concurrently with e2e.sh (pane captures and session kills collide).
set -u
cd "$(dirname "$0")/.."

# Rebuild release when the binary is missing or older than any src file.
if [ ! -x target/release/emur ] || [ -n "$(find src Cargo.toml -newer target/release/emur 2>/dev/null)" ]; then
  echo "release_smoke: cargo build --release"
  cargo build --release || exit 1
fi

# Borrow the tmux harness (helpers + the six scenario definitions) from
# e2e.sh: lines 1-53 are the header (incl. run_pipe/expect_p), 543-545 the
# ANSI-capture helpers; then extract the needed scenarios.
TMUX_HARNESS="$(mktemp)"
{
  sed -n '1,53p' test-design/e2e.sh
  sed -n '543,545p' test-design/e2e.sh
  echo 'expect_lastline() { [ "$(cap | tail -1)" = "$1" ]; }'
  BIN=./target/release/emur
  for s in s02 s103 s110 s112 s140 s141; do
    sed -n "/^t_$s() {/,/^}/p" test-design/e2e.sh
  done
  echo 'for t in t_s02 t_s103 t_s110 t_s112 t_s140 t_s141; do "$t"; done'
  echo 'echo "FAILED=$FAILED"'
  echo 'exit $FAILED'
} > "$TMUX_HARNESS"
bash "$TMUX_HARNESS"
rc=$?
rm -f "$TMUX_HARNESS"
echo "release_smoke: $([ $rc = 0 ] && echo PASS || echo FAIL)"
exit $rc
