#!/usr/bin/env bash
# End-to-end test harness for emur, tmux-driven.
# Design: ../test-design/README.md
set -u
BIN="./target/debug/emur"
STEP=0.3
FAILED=0
SESSION=""

start_session() {
  SESSION="emu-e2e-$1"
  local cmd="$BIN ${2:-}"
  tmux new-session -d -s "$SESSION" -x 80 -y 24 "$cmd; echo EMU_EXITED \$?; sleep 30"
  sleep 0.6
}

sendb() { tmux send-keys -t "$SESSION" -l "$@"; sleep "$STEP"; }
enter() { tmux send-keys -t "$SESSION" Enter; sleep "$STEP"; }

cap() { tmux capture-pane -p -t "$SESSION" 2>/dev/null | perl -pe 's/[ \t]+$//'; }

expect() { cap | grep -qE -- "$1"; }
expect_absent() { ! cap | grep -qE -- "$1"; }
expect_count() { [ "$(cap | grep -cE -- "$1")" = "$2" ]; }

report() {
  if [ "$2" = 1 ]; then
    echo "PASS $1"
  else
    echo "FAIL $1"
    FAILED=1
  fi
  tmux kill-session -t "$SESSION" 2>/dev/null
}

t_s01() {
  start_session s01
  local ok=1
  expect 'This is the first line' || ok=0
  expect '^Buf \*demo\*$' || ok=0
  report s01 "$ok"
}

t_s02() {
  start_session s02
  sendb 'hello world'
  local ok=1
  expect 'hello worldThis is the first line' || ok=0
  expect '^Buf \*demo\* \*$' || ok=0
  report s02 "$ok"
}

t_s03() {
  start_session s03
  sendb 'AAA' $'\012' 'BBB'
  local ok=1
  expect '^AAA$' || ok=0
  expect 'BBBThis is the first line' || ok=0
  report s03 "$ok"
}

t_s04() {
  start_session s04
  sendb 'abc def' $'\033b' 'X'
  local ok=1
  expect 'abc XdefThis is the first line' || ok=0
  report s04 "$ok"
}

t_s05() {
  start_session s05
  sendb $'\001' $'\013'
  local ok=1
  expect_absent 'This is the first line' || ok=0
  expect 'of the demo buffer\.' || ok=0
  report s05 "$ok"
}

t_s06() {
  rm -f /tmp/emur-e2e-write.txt
  start_session s06
  sendb 'foo bar' $'\030\027' '/tmp/emur-e2e-write.txt'
  enter
  sendb $'\030\023'
  local ok=1
  grep -q 'foo bar' /tmp/emur-e2e-write.txt || ok=0
  expect 'Wrote /tmp/emur-e2e-write\.txt' || ok=0
  report s06 "$ok"
}

t_s07() {
  start_session s07
  sendb $'\030\006' '/tmp/emur-e2e-write.txt'
  enter
  local ok=1
  expect 'Buf /tmp/emur-e2e-write\.txt' || ok=0
  expect 'foo barThis is the first line' || ok=0
  report s07 "$ok"
}

t_s08() {
  start_session s08
  sendb $'\030\011' '/tmp/emur-e2e-insert.txt'
  enter
  local ok=1
  expect '\-INSERTED-This is the first line' || ok=0
  report s08 "$ok"
}

t_s09() {
  start_session s09
  sendb $'\030k'
  local ok=1
  expect 'Killed buffer' || ok=0
  expect '^@+$' || ok=0
  report s09 "$ok"
}

t_s10() {
  start_session s10
  sendb $'\0302'
  local ok=1
  expect_count 'This is the first line' 2 || ok=0
  report s10 "$ok"
}

t_s11() {
  start_session s11
  sendb $'\0303'
  local ok=1
  expect 'This is the first line +This is the first line' || ok=0
  report s11 "$ok"
}

t_s12() {
  start_session s12
  sendb $'\0302'
  sendb $'\030o'
  sendb $'\001' 'X'
  sendb $'\030o' 'Y'
  local ok=1
  expect 'YXThis is the first line' || ok=0
  report s12 "$ok"
}

t_s13() {
  start_session s13
  sendb $'\0302' $'\0303' $'\0301'
  local ok=1
  expect_count 'This is the first line' 1 || ok=0
  report s13 "$ok"
}

t_s14() {
  start_session s14
  sendb $'\023zzz'
  enter
  local ok=1
  expect 'Search string not found: zzz' || ok=0
  report s14 "$ok"
}

t_s15() {
  start_session s15
  sendb $'\023' $'\a'
  local ok=1
  expect 'Cancelled' || ok=0
  report s15 "$ok"
}

t_s16() {
  start_session s16
  sendb $'\033>'
  sendb $'\022buffer'
  enter
  local ok=1
  expect '^Buf \*demo\*$' || ok=0
  expect_absent 'not found' || ok=0
  report s16 "$ok"
}

t_s17() {
  start_session s17
  sendb $'\030\003'
  local ok=1
  expect 'EMU_EXITED 0' || ok=0
  report s17 "$ok"
}

# total occurrence count of pattern across all pane lines
count_occ() { cap | grep -oE -- "$1" | wc -l; }
expect_occ() { [ "$(count_occ "$1")" = "$2" ]; }

# --- combined window-pattern scenarios ---------------------------------------

t_s20() {  # three stacked windows
  start_session s20
  sendb $'\0302\0302'
  local ok=1
  expect_count 'This is the first line' 3 || ok=0
  report s20 "$ok"
}

t_s21() {  # top split again, right: 3 windows, 3 seed occurrences
  start_session s21
  sendb $'\0302' $'\0303'
  local ok=1
  expect_occ 'This is the first line' 3 || ok=0
  report s21 "$ok"
}

t_s22() {  # 3 windows, one char typed per window while cycling
  start_session s22
  sendb $'\0302\0303'
  sendb '1'
  sendb $'\030o' '2'
  sendb $'\030o' '3'
  local ok=1
  expect '321This is the first line' || ok=0
  report s22 "$ok"
}

t_s23() {  # erase: collapse preserves the active (lower) window's point
  start_session s23
  sendb $'\0302'
  sendb $'\030o' 'X'
  sendb $'\0301' 'Y'
  local ok=1
  expect 'XYThis is the first line' || ok=0
  report s23 "$ok"
}

t_s24() {  # erase after right split keeps the cycled window's point
  start_session s24
  sendb $'\0303'
  sendb $'\030o' 'R'
  sendb $'\0301' 'S'
  local ok=1
  expect 'RSThis is the first line' || ok=0
  report s24 "$ok"
}

t_s25() {  # split -> erase -> re-split: points inherited on re-split
  start_session s25
  sendb $'\0302'
  sendb $'\030o' 'X'
  sendb $'\0301' 'Y'
  sendb $'\0302'
  sendb $'\030o' 'Z'
  local ok=1
  expect 'XYZThis is the first line' || ok=0
  report s25 "$ok"
}

t_s26() {  # kill-buffer in a 3-window state, then typing works
  start_session s26
  sendb $'\0302\0303'
  sendb $'\030k' 'K'
  local ok=1
  expect 'Killed buffer' || ok=0
  expect '^K$' || ok=0
  expect_absent 'This is the first line' || ok=0
  report s26 "$ok"
}

t_s27() {  # reverse search then erase: point of the match survives erase
  start_session s27
  sendb $'\033>'
  sendb $'\022buffer'
  enter
  sendb $'\0301' 'Q'
  local ok=1
  expect 'demo Qbuffer\.' || ok=0
  report s27 "$ok"
}

t_s28() {  # write/load + split + erase + save round-trip
  start_session s28
  sendb 'foo bar' $'\030\027' '/tmp/emur-e2e-write.txt'
  enter
  sendb $'\030\006' '/tmp/emur-e2e-write.txt'
  enter
  sendb $'\0302'
  sendb $'\030o' 'F'
  sendb $'\0301'
  sendb $'\030\023'
  local ok=1
  expect 'Wrote /tmp/emur-e2e-write\.txt' || ok=0
  grep -q 'Ffoo bar' /tmp/emur-e2e-write.txt || ok=0
  report s28 "$ok"
}

rm -f /tmp/emur-e2e-write.txt

t_s30() {  # file given as command-line argument is opened
  printf 'FROM ARG FIRST\nsecond arg line\n' > /tmp/emur-e2e-arg.txt
  start_session s30 /tmp/emur-e2e-arg.txt
  local ok=1
  expect '^Buf /tmp/emur-e2e-arg\.txt$' || ok=0
  expect 'FROM ARG FIRST' || ok=0
  expect_absent '\*demo\*' || ok=0
  report s30 "$ok"
}

t_s31() {  # CLI-opened buffer saves to its file without prompt
  printf 'FROM ARG FIRST\nsecond arg line\n' > /tmp/emur-e2e-arg.txt
  start_session s31 /tmp/emur-e2e-arg.txt
  sendb 'X'
  sendb $'\030\023'
  local ok=1
  expect 'Wrote /tmp/emur-e2e-arg\.txt' || ok=0
  grep -q 'XFROM ARG FIRST' /tmp/emur-e2e-arg.txt || ok=0
  report s31 "$ok"
}

printf '%s' '-INSERTED-' > /tmp/emur-e2e-insert.txt

for t in t_s01 t_s02 t_s03 t_s04 t_s05 t_s06 t_s07 t_s08 t_s09 t_s10 t_s11 t_s12 t_s13 t_s14 t_s15 t_s16 t_s17 t_s20 t_s21 t_s22 t_s23 t_s24 t_s25 t_s26 t_s27 t_s28 t_s30 t_s31; do
  "$t"
done

rm -f /tmp/emur-e2e-insert.txt

if [ "$FAILED" = 0 ]; then
  echo "ALL TESTS PASS"
  exit 0
else
  echo "SOME TESTS FAILED"
  exit 1
fi
