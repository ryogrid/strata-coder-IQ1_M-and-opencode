#!/usr/bin/env bash
# End-to-end test harness for emur, tmux-driven.
# Design: ../test-design/README.md
set -u
BIN="./target/debug/emur"
STEP=0.4
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

expect() {
  local i
  for i in 1 2 3; do
    cap | grep -qE -- "$1" && return 0
    [ "$i" = 3 ] || sleep 0.3
  done
  return 1
}
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

# Pipe (non-tmux) runner (Design Doc A §4): runs emur with piped stdin under
# timeout. Sets RC (exit code; a timeout kill gives 124), SIZE (stdout bytes;
# the EOF flood regressions emit tens of MB, so exit code alone is not
# enough) and CAP (captured output file for expect_p).
run_pipe() {
  CAP="/tmp/emur-e2e-pipe-$1.out"
  printf '%s' "$2" | timeout 10 "$BIN" > "$CAP" 2>/dev/null
  RC=$?
  SIZE=$(wc -c < "$CAP")
}
expect_p() { grep -qE -- "$1" "$CAP"; }

t_s01() {
  start_session s01
  local ok=1
  expect 'This is the first line' || ok=0
  expect '^Buf \*demo\* 1/3 \(0%\)$' || ok=0
  report s01 "$ok"
}

t_s02() {
  start_session s02
  sendb 'hello world'
  local ok=1
  expect 'hello worldThis is the first line' || ok=0
  expect '^Buf \*demo\* \* 1/3 \(20%\)$' || ok=0
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
  expect 'This is the first line +\|This is the first line' || ok=0
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
  expect_lastline 'Buf *demo* 1/3 (0%)' || ok=0
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
  expect '^Buf \*demo\* 2/3 \(81%\)$' || ok=0
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
  # NOTE (2026-10-24): the old count assertion (expect_occ ... 3) flaked
  # because a single capture can RACE the rendering: mid-sequence it sees
  # the 2-window state (C-x 3 not applied yet), and frames begin with a
  # screen_clear so a partial capture shows the not-yet-drawn bottom window
  # as '@' lines. The new assertions describe the layout structure (left
  # column seeds x2, glued boundary-column row, dash boundary row) and poll
  # 3x so the render race is absorbed; layout facts: boundary column sits
  # at col 40 (width/2) and the boundary row spans the full width (cols).
  start_session s21
  sendb $'\0302' $'\0303'
  local ok=1
  local i ok3=0
  for i in 1 2 3; do
    sleep 0.4
    cap | grep -cE '^This is the first line' | grep -q '^2$' &&
      cap | grep -qE '\|This is the first line' &&
      cap | grep -qE '^-{40,}$' && { ok3=1; break; }
  done
  [ "$ok3" = 1 ] || ok=0
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
  expect '^Buf /tmp/emur-e2e-arg\.txt 1/3 \(0%\)$' || ok=0
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

t_s40() {  # CRLF (Windows) file renders normally and scrolls on C-n
  python3 -c 'import sys; sys.stdout.write("".join("CRLF LINE %02d\r\n" % i for i in range(1,31)))' > /tmp/emur-e2e-crlf.txt
  start_session s40 /tmp/emur-e2e-crlf.txt
  local ok=1
  expect 'CRLF LINE 01' || ok=0
  expect 'CRLF LINE 23' || ok=0
  sendb $'\016\016\016\016\016\016\016\016\016\016\016\016\016\016\016\016\016\016\016\016\016\016\016\016\016'
  expect 'CRLF LINE 26' || ok=0
  expect_absent 'CRLF LINE 01' || ok=0
  report s40 "$ok"
}

t_s41() {  # ANSI arrow keys move the cursor instead of inserting junk
  python3 -c 'import sys; sys.stdout.write("".join("LINE %02d of big file\n" % i for i in range(1,41)))' > /tmp/emur-e2e-big.txt
  start_session s41 /tmp/emur-e2e-big.txt
  tmux send-keys -t "$SESSION" Up Up Down Right Right Left
  sleep "$STEP"
  sendb 'Z'
  local ok=1
  expect 'LZINE 02 of big file' || ok=0
  expect_absent 'ALINE|BLINE|CLINE|DLIN' || ok=0
  report s41 "$ok"
}

t_s42() {  # tty settings restored on exit: shell still reads lines
  start_session s42 /tmp/emur-e2e-big.txt
  sendb $'\030\003'
  sleep 1
  sendb 'echo RESTORED_TTY'
  enter
  local ok=1
  expect 'RESTORED_TTY' || ok=0
  report s42 "$ok"
}

t_s50() {  # C-k kills the line; C-y pastes it back at point
  start_session s50
  sendb $'\013'
  local ok=1
  expect_absent 'This is the first line' || ok=0
  sendb $'\031'
  expect 'This is the first line' || ok=0
  report s50 "$ok"
}

t_s51() {  # consecutive C-k accumulate (line + newline); one C-y restores both rows
  start_session s51
  sendb $'\013' $'\013'
  local ok=1
  expect_absent 'This is the first line' || ok=0
  sendb $'\031'
  expect 'This is the first line' || ok=0
  expect 'of the demo buffer.' || ok=0
  report s51 "$ok"
}

t_s52() {  # killed text survives intervening commands; paste lands after the typed X
  start_session s52
  sendb $'\013' 'X' $'\031'
  local ok=1
  expect 'XThis is the first line' || ok=0
  report s52 "$ok"
}

expect_lastline() { [ "$(cap | tail -1)" = "$1" ]; }

t_s60() {  # status line sits at the bottom and always shows line/total (pct)
  start_session s60
  local ok=1
  expect_lastline 'Buf *demo* 1/3 (0%)' || ok=0
  sendb $'\006\006\006\006\006\006\006\006\006\006'
  expect_lastline 'Buf *demo* 1/3 (23%)' || ok=0
  report s60 "$ok"
}

t_s61() {  # C-@ sets mark; C-k kills only to the region end (mark)
  start_session s61
  tmux send-keys -t "$SESSION" C-@; sleep "$STEP"
  sendb 'ABCDEFGHIJ' $'\002\002\002' $'\013'
  local ok=1
  expect 'ABCDEFGThis is the first line' || ok=0
  expect_absent 'HIJ' || ok=0
  report s61 "$ok"
}

t_s62() {  # C-a stops at region start (mark) inside the line
  start_session s62
  sendb 'ABCDEFGHIJ' $'\002\002\002'
  tmux send-keys -t "$SESSION" C-@; sleep "$STEP"
  sendb $'\006\006' $'\001' 'Z'
  local ok=1
  expect 'ABCDEFGZHIJThis is the first line' || ok=0
  report s62 "$ok"
}

t_s63() {  # C-x C-f opens a file in a new split window; status shows its name
  printf 'OPENED FIRST LINE\nOPENED SECOND\n' > /tmp/emur-e2e-open.txt
  start_session s63
  sendb $'\030\006' '/tmp/emur-e2e-open.txt'
  enter
  local ok=1
  expect_lastline 'Buf /tmp/emur-e2e-open.txt 1/3 (0%)' || ok=0
  expect 'OPENED FIRST LINE' || ok=0
  expect 'This is the first line' || ok=0
  report s63 "$ok"
}

t_s70() {  # C-x b switches the active window back to the *demo* buffer
  printf 'SWITCHED CONTENT\n' > /tmp/emur-e2e-sw.txt
  start_session s70
  sendb $'\030\006' '/tmp/emur-e2e-sw.txt'
  enter
  sendb $'\030b' '*demo*'
  enter
  local ok=1
  expect_lastline 'Buf *demo* 1/3 (0%)' || ok=0
  sendb $'\030\002'
  expect 'Buffers: \*demo\* /tmp/emur-e2e-sw\.txt' || ok=0
  report s70 "$ok"
}

t_s71() {  # C-x b to a new name creates an empty buffer (seed text gone)
  start_session s71
  sendb $'\030b' 'fresh'
  enter
  sendb 'K'
  local ok=1
  expect '^K$' || ok=0
  expect_absent 'This is the first line' || ok=0
  expect_lastline 'Buf fresh * 1/1 (100%)' || ok=0
  report s71 "$ok"
}

t_s72() {  # Ctrl-x Ctrl-b lists buffers in the minibuffer; Enter cycles to the next one
  printf 'CYCLE CONTENT\n' > /tmp/emur-e2e-cyc.txt
  start_session s72
  sendb $'\030\006' '/tmp/emur-e2e-cyc.txt'
  enter
  sendb $'\030\002'
  local ok=1
  expect 'Buffers: \*demo\* /tmp/emur-e2e-cyc\.txt' || ok=0
  enter
  expect_lastline 'Buf *demo* 1/3 (0%)' || ok=0
  report s72 "$ok"
}

t_s80() {  # Tab in the Write-file minibuffer completes a unique path prefix
  printf 'OLD CONTENT\n' > /tmp/emur-e2e-comp.txt
  start_session s80
  sendb 'TABTEST' $'\030\027'
  sendb $'/tmp/emur-e2e-comp\t'
  enter
  local ok=1
  expect 'Wrote /tmp/emur-e2e-comp\.txt' || ok=0
  grep -q 'TABTEST' /tmp/emur-e2e-comp.txt || ok=0
  report s80 "$ok"
}

t_s81() {  # Tab completes a unique buffer name (C-x b)
  start_session s81
  sendb $'\030b' $'*demo\t'
  enter
  local ok=1
  expect_lastline 'Buf *demo* 1/3 (0%)' || ok=0
  report s81 "$ok"
}

t_s82() {  # ambiguous buffer completion lists candidates; disambiguating key finishes
  start_session s82
  sendb $'\030b' 'aa'
  enter
  sendb $'\030b' 'ab'
  enter
  sendb $'\030b' $'a\t'
  local ok=1
  expect 'aa ab' || ok=0
  sendb 'b'
  enter
  expect_lastline 'Buf ab 1/1 (0%)' || ok=0
  report s82 "$ok"
}

t_s83() {  # Tab in the Find-file minibuffer completes a unique path (split window)
  printf 'FCOMP FIRST LINE\nFCOMP SECOND\n' > /tmp/emur-e2e-fcomp.txt
  start_session s83
  sendb $'\030\006' $'/tmp/emur-e2e-fcomp\t'
  enter
  local ok=1
  expect_lastline 'Buf /tmp/emur-e2e-fcomp.txt 1/3 (0%)' || ok=0
  expect 'FCOMP FIRST LINE' || ok=0
  report s83 "$ok"
}

cap_e() { tmux capture-pane -pe -t "$SESSION" 2>/dev/null; }
expect_e() { cap_e | grep -qE -- $'\033'"$1"; }
expect_absent_e() { ! cap_e | grep -qE -- $'\033'"$1"; }

t_s90() {  # rust: keywords cyan, numbers blue, // comments green
  printf 'fn main() {\n    let x = 42;\n    // note\n}\n' > /tmp/emur-e2e-h.rs
  start_session s90 /tmp/emur-e2e-h.rs
  local ok=1
  expect 'fn main' || ok=0
  expect_e '\[36m' || ok=0
  expect_e '\[34m' || ok=0
  expect_e '\[32m' || ok=0
  report s90 "$ok"
}

t_s91() {  # python: # comment green, string magenta, def keyword cyan
  printf 'def greet():\n    msg = "hi"  # note\n' > /tmp/emur-e2e-h.py
  start_session s91 /tmp/emur-e2e-h.py
  local ok=1
  expect 'def greet' || ok=0
  expect_e '\[36m' || ok=0
  expect_e '\[35m' || ok=0
  expect_e '\[32m' || ok=0
  report s91 "$ok"
}

t_s92() {  # json: strings magenta, numbers blue, true cyan
  printf '{"n": 42, "ok": true}\n' > /tmp/emur-e2e-h.json
  start_session s92 /tmp/emur-e2e-h.json
  local ok=1
  expect '"n": 42' || ok=0
  expect_e '\[35m' || ok=0
  expect_e '\[34m' || ok=0
  expect_e '\[36m' || ok=0
  report s92 "$ok"
}

t_s93() {  # markdown: heading bold-cyan, `code` magenta
  printf '# Heading\ntext `code` here\n' > /tmp/emur-e2e-h.md
  start_session s93 /tmp/emur-e2e-h.md
  local ok=1
  expect '# Heading' || ok=0
  expect_e '\[1m' || ok=0
  expect_e '\[36m' || ok=0
  expect_e '\[35m' || ok=0
  report s93 "$ok"
}

t_s94() {  # plain text (.txt): no color codes beyond per-line resets
  printf 'plain text 42\n' > /tmp/emur-e2e-h.txt
  start_session s94 /tmp/emur-e2e-h.txt
  local ok=1
  expect 'plain text' || ok=0
  expect_absent_e '\[3[2456]m' || ok=0
  report s94 "$ok"
}

t_s100() {  # C-x C-f opens .rs (split) + highlighting; Tab-completed C-x b back to *demo*; colors persist in the file window
  printf 'fn main() {\n    // note\n}\n' > /tmp/emur-e2e-h2.rs
  start_session s100
  sendb $'\030\006' '/tmp/emur-e2e-h2.rs'
  enter
  local ok=1
  expect_lastline 'Buf /tmp/emur-e2e-h2.rs 1/4 (0%)' || ok=0
  expect_e '\[36m' || ok=0
  expect_e '\[32m' || ok=0
  sendb $'\030b' $'*demo\t'
  enter
  expect_lastline 'Buf *demo* 1/3 (0%)' || ok=0
  expect 'This is the first line' || ok=0
  sendb $'\030b' $'/tmp/emur-e2e-h2\t'
  enter
  expect_lastline 'Buf /tmp/emur-e2e-h2.rs 1/4 (0%)' || ok=0
  expect_e '\[36m' || ok=0
  expect_e '\[32m' || ok=0
  report s100 "$ok"
}

t_s101() {  # two .py files opened (two windows) + buffer-list + Tab-completed buffer switch keeps python coloring
  mkdir -p /tmp/emur-cx
  printf 'def a():\n    # A note\n' > /tmp/emur-cx/aa.py
  printf 'def b():\n    # B note\n' > /tmp/emur-cx/bb.py
  start_session s101
  sendb $'\030\006' '/tmp/emur-cx/aa.py'
  enter
  sendb $'\030\006' '/tmp/emur-cx/bb.py'
  enter
  local ok=1
  expect_lastline 'Buf /tmp/emur-cx/bb.py 1/3 (0%)' || ok=0
  sendb $'\030\002'
  expect 'Buffers: \*demo\* /tmp/emur-cx/aa\.py /tmp/emur-cx/bb\.py' || ok=0
  enter
  sendb $'\030b' $'/tmp/emur-cx/a\t'
  enter
  expect_lastline 'Buf /tmp/emur-cx/aa.py 1/3 (0%)' || ok=0
  expect_e '\[36m' || ok=0
  expect_e '\[32m' || ok=0
  report s101 "$ok"
}

t_s102() {  # mark/region + split window + C-k region kill + paste (combined, plain buffer stays colorless)
  start_session s102
  sendb 'ABCDEFGHIJ'
  tmux send-keys -t "$SESSION" C-@; sleep "$STEP"
  sendb $'\002\002\002' $'\030\062' $'\030\061' $'\013' 'X' $'\031'
  local ok=1
  expect 'ABCDEFGXHIJThis is the first line' || ok=0
  expect_absent_e '3[2456]m' || ok=0
  report s102 "$ok"
}

t_s103() {  # C-x C-w writes a .py file, then Tab-completed C-x C-f reopens it in a split window; python keyword colored
  mkdir -p /tmp/emur-cx
  start_session s103
  sendb 'def f():'
  sendb $'\030\027' $'/tmp/emur-cx/xx.py'
  enter
  local ok=1
  grep -q 'def f()' /tmp/emur-cx/xx.py || ok=0
  sendb $'\030\006' $'/tmp/emur-cx/x\t'
  enter
  expect_lastline 'Buf /tmp/emur-cx/xx.py 1/3 (0%)' || ok=0
  expect 'def f\(\):This is the first line' || ok=0
  expect_e '\[36m' || ok=0
  report s103 "$ok"
}

t_s104() {  # json opened via argv + C-x C-i Tab-completed insert of another json; strings/numbers stay colored
  printf '{"a": 1}\n' > /tmp/emur-cx/ins.json
  printf '{"n": 42, "ok": true}\n' > /tmp/emur-e2e-h4.json
  start_session s104 /tmp/emur-e2e-h4.json
  local ok=1
  sendb $'\030\011' $'/tmp/emur-cx/ins\t'
  enter
  expect '{"a": 1}' || ok=0
  expect '{"n": 42, "ok": true}' || ok=0
  expect_e '\[34m' || ok=0
  expect_e '\[35m' || ok=0
  report s104 "$ok"
}

t_s105() {  # C-x C-f .md (split) + split-below + delete-other-windows keeps md heading bold and inline code magenta
  printf '# Title\ntext `code` here\n' > /tmp/emur-e2e-h5.md
  start_session s105
  sendb $'\030\006' '/tmp/emur-e2e-h5.md'
  enter
  sendb $'\030\062' $'\030\061'
  local ok=1
  expect '# Title' || ok=0
  expect_e '\[1m' || ok=0
  expect_e '\[35m' || ok=0
  expect_lastline 'Buf /tmp/emur-e2e-h5.md 1/3 (0%)' || ok=0
  report s105 "$ok"
}
t_s110() {  # C-s incremental search: chars extend the query; a non-matching char is dropped; C-s repeats; Enter accepts
  start_session s110
  sendb $'\023' 'first'
  enter
  local ok=1
  expect_lastline 'Buf *demo* 1/3 (39%)' || ok=0
  sendb $'\023\023'
  enter
  expect_lastline 'Buf *demo* 1/3 (39%)' || ok=0
  sendb $'\023' 'firs' 'z' 't'
  enter
  expect_lastline 'Buf *demo* 1/3 (39%)' || ok=0
  report s110 "$ok"
}
t_s111() {  # C-x C-c with modified content asks for confirmation; "no" cancels, "yes" exits the editor
  start_session s111
  sendb 'X'
  sendb $'\030\003'
  local ok=1
  expect_lastline 'Modified buffers exist; exit anyway? (yes|no)' || ok=0
  sendb 'no'
  enter
  expect_lastline 'Cancelled 1/3 (2%)' || ok=0
  expect 'XThis is the first line' || ok=0
  sendb $'\030\003'
  sleep 0.4
  sendb 'yes'
  enter
  sleep 0.6
  expect 'EMU_EXITED 0' || ok=0
  report s111 "$ok"
}
t_s112() {  # C-l go-to-line: minibuffer number moves point to the start of that line (clamped)
  start_session s112
  sendb $'\014' '2'
  enter
  local ok=1
  expect_lastline 'Buf *demo* 2/3 (53%)' || ok=0
  sendb $'\014' '1'
  enter
  expect_lastline 'Buf *demo* 1/3 (0%)' || ok=0
  sendb $'\014' '99'
  enter
  expect_lastline 'Buf *demo* 3/3 (100%)' || ok=0
  sendb $'\014' 'x'
  enter
  expect_lastline 'Invalid line number: x 3/3 (100%)' || ok=0
  report s112 "$ok"
}

t_s120() {  # right split + C-x C-f loads file into the ACTIVE window (no further split) + C-x o cycling; typing lands in each window's own buffer
  mkdir -p /tmp/emur-cx
  printf 'AAA line\nBBB line\n' > /tmp/emur-cx/s120.py
  start_session s120
  sendb $'\030\063'
  sendb $'\030\157'
  sendb $'\030\006' '/tmp/emur-cx/s120.py'
  enter
  sendb 'F'
  local ok=1
  expect 'FAAA line' || ok=0
  sendb $'\030\157'
  sendb 'D'
  sendb $'\030\157'
  sendb 'G'
  expect 'DThis is the first line +\|FGAAA line' || ok=0
  expect_lastline 'Buf /tmp/emur-cx/s120.py * 1/3 (10%)' || ok=0
  report s120 "$ok"
}

t_s121() {  # C-x b inside a split window: other windows keep their own buffers
  start_session s121
  sendb $'\030\063'
  sendb $'\030\157'
  sendb $'\030b' 'bufB'
  enter
  local ok=1
  expect_lastline 'Buf bufB 1/1 (0%)' || ok=0
  expect_occ 'This is the first line' 1 || ok=0
  sendb $'\030\157'
  expect_lastline 'Buf *demo* 1/3 (0%)' || ok=0
  sendb 'L'
  expect_lastline 'Buf *demo* * 1/3 (2%)' || ok=0
  sendb $'\030\157'
  sendb 'M'
  expect_lastline 'Buf bufB * 1/1 (100%)' || ok=0
  report s121 "$ok"
}

t_s122() {  # user-reported sequence: C-x 3 + switch + C-x C-f + cycle + C-x 1 collapse keeps the active window's buffer
  mkdir -p /tmp/emur-cx
  printf 'AAA line\nBBB line\n' > /tmp/emur-cx/s122.py
  start_session s122
  sendb $'\030\063'
  sendb $'\030\157'
  sendb $'\030\006' '/tmp/emur-cx/s122.py'
  enter
  sendb $'\030\157'
  sendb $'\030\061'
  sendb 'K'
  local ok=1
  expect 'KThis is the first line' || ok=0
  expect_absent 'AAA line' || ok=0
  expect_lastline 'Buf *demo* * 1/3 (2%)' || ok=0
  report s122 "$ok"
}

t_s123() {  # stacked split + open file in lower window + scroll + save from that window
  mkdir -p /tmp/emur-cx
  printf 'AAA line\nBBB line\n' > /tmp/emur-cx/s123.py
  start_session s123
  sendb $'\030\062'
  local ok=1
  expect '^-+$' || ok=0
  sendb $'\030\157'
  sendb $'\030\006' '/tmp/emur-cx/s123.py'
  enter
  sendb 'S'
  expect 'SAAA line' || ok=0
  sendb $'\030\023'
  expect 'Wrote /tmp/emur-cx/s123\.py' || ok=0
  grep -q 'SAAA line' /tmp/emur-cx/s123.py || ok=0
  report s123 "$ok"
}

t_s124() {  # user's exact sequence: file-arg launch + C-x 3 + C-x o + Tab completion listing cancelled by C-g + literal C-x C-f retry + C-n scroll + C-x o cycling
  mkdir -p /tmp/emur-cx
  seq 1 30 | awk '{printf "BIG1 line %02d end\n", $1}' > /tmp/emur-cx/s124-a.txt
  seq 1 30 | awk '{printf "BIG2 line %02d end\n", $1}' > /tmp/emur-cx/s124-b.txt
  start_session s124 /tmp/emur-cx/s124-a.txt
  local ok=1
  expect '^Buf /tmp/emur-cx/s124-a\.txt 1/31 \(0%\)$' || ok=0
  sendb $'\030\063'
  expect 'BIG1 line 01 end +\|BIG1 line 01 end' || ok=0
  sendb $'\030\157'
  sendb $'\030\006'
  tmux send-keys -t "$SESSION" Tab; sleep "$STEP"
  sendb $'\007'
  expect_lastline 'Buf /tmp/emur-cx/s124-a.txt 1/31 (0%)' || ok=0
  sendb $'\030\006' '/tmp/emur-cx/s124-b.txt'
  enter
  expect 'BIG1 line 01 end +\|BIG2 line 01 end' || ok=0
  expect_lastline 'Buf /tmp/emur-cx/s124-b.txt 1/31 (0%)' || ok=0
  for n in 1 2 3 4 5 6 7 8 9 10 11 12; do sendb $'\016'; done
  expect_lastline 'Buf /tmp/emur-cx/s124-b.txt 13/31 (40%)' || ok=0
  expect 'BIG2 line 13 end' || ok=0
  sendb $'\030\157'
  expect_lastline 'Buf /tmp/emur-cx/s124-a.txt 1/31 (0%)' || ok=0
  sendb 'X'
  expect 'XBIG1 line 01 end +\|BIG2 line 01 end' || ok=0
  expect_lastline 'Buf /tmp/emur-cx/s124-a.txt * 1/31 (0%)' || ok=0
  sendb $'\030\157'
  sendb 'Y'
  expect 'YBIG2 line 13 end' || ok=0
  expect_lastline 'Buf /tmp/emur-cx/s124-b.txt * 13/31 (40%)' || ok=0
  sendb $'\030\157'
  expect_lastline 'Buf /tmp/emur-cx/s124-a.txt * 1/31 (0%)' || ok=0
  expect 'XBIG1 line 01 end' || ok=0
  expect 'YBIG2 line 13 end' || ok=0
  report s124 "$ok"
}

printf '%s' '-INSERTED-' > /tmp/emur-e2e-insert.txt

t_s130() {  # C-x C-b: Tab lists all names; arrow-lefts land the caret on "aa"; Enter selects it
  start_session s130
  sendb $'\030b' 'aa'
  enter
  sendb $'\030b' 'ab'
  enter
  sendb $'\030\002'
  tmux send-keys -t "$SESSION" Tab
  sleep "$STEP"
  local ok=1
  expect 'Buffers: \*demo\* aa ab Enter=next, name:   \*demo\* aa ab' || ok=0
  tmux send-keys -t "$SESSION" Left Left Left Left
  sleep "$STEP"
  enter
  expect_lastline 'Buf aa 1/1 (0%)' || ok=0
  report s130 "$ok"
}

t_s131() {  # C-x b: Tab lists aa ab; C-b moves the caret onto the last candidate; Enter selects it
  start_session s131
  sendb $'\030b' 'aa'
  enter
  sendb $'\030b' 'ab'
  enter
  sendb $'\030b' $'a\t'
  local ok=1
  expect 'Switch to buffer: a  aa ab' || ok=0
  sendb $'\002'
  enter
  expect_lastline 'Buf ab 1/1 (0%)' || ok=0
  report s131 "$ok"
}

t_s132() {  # Enter with the caret past the list accepts the typed input (creates buffer "a")
  start_session s132
  sendb $'\030b' 'aa'
  enter
  sendb $'\030b' 'ab'
  enter
  sendb $'\030b' $'a\t'
  local ok=1
  expect 'Switch to buffer: a  aa ab' || ok=0
  enter
  expect_lastline 'Buf a 1/1 (0%)' || ok=0
  report s132 "$ok"
}

t_s133() {  # Find file: Tab lists two files; arrow-lefts land on the first candidate; Enter opens it
  mkdir -p /tmp/emur-cx
  printf 'S133 A\n' > /tmp/emur-cx/s133-a.txt
  printf 'S133 B\n' > /tmp/emur-cx/s133-b.txt
  start_session s133
  sendb $'\030\006' '/tmp/emur-cx/s133-'
  tmux send-keys -t "$SESSION" Tab
  sleep "$STEP"
  local ok=1
  expect '/tmp/emur-cx/s133-a\.txt /tmp/emur-cx/s133-b\.txt' || ok=0
  local i
  for i in $(seq 1 25); do tmux send-keys -t "$SESSION" Left; done
  sleep "$STEP"
  enter
  expect 'S133 A' || ok=0
  expect_lastline 'Buf /tmp/emur-cx/s133-a.txt 1/2 (0%)' || ok=0
  report s133 "$ok"
}

t_s140() {  # exit-confirm EOF (piped stdin): clean exit, no NUL flood (T-1/M-2)
  run_pipe s140 $'x\003'
  local ok=1
  [ "$RC" = 0 ] || ok=0
  [ "$SIZE" -lt 1048576 ] || ok=0
  expect_p 'exit anyway' || ok=0
  report s140 "$ok"
}

t_s141() {  # C-l with a 15-digit number: clamps to last line, editor alive (M-1)
  start_session s141
  sendb $'\014' '999999999999999'
  enter
  local ok=1
  expect_lastline 'Buf *demo* 3/3 (100%)' || ok=0
  sendb 'Z'
  expect_lastline 'Buf *demo* * 3/3 (100%)' || ok=0
  report s141 "$ok"
}

t_s142() {  # C-r query EOF before Enter: cancelled, one status frame, clean exit
  run_pipe s142 $'\022abc'
  local ok=1
  [ "$RC" = 0 ] || ok=0
  [ "$SIZE" -lt 1048576 ] || ok=0
  expect_p 'Cancelled' || ok=0
  report s142 "$ok"
}

for t in t_s01 t_s02 t_s03 t_s04 t_s05 t_s06 t_s07 t_s08 t_s09 t_s10 t_s11 t_s12 t_s13 t_s14 t_s15 t_s16 t_s17 t_s20 t_s21 t_s22 t_s23 t_s24 t_s25 t_s26 t_s27 t_s28 t_s30 t_s31 t_s40 t_s41 t_s42 t_s50 t_s51 t_s52 t_s60 t_s61 t_s62 t_s63 t_s70 t_s71 t_s72 t_s80 t_s81 t_s82 t_s83 t_s90 t_s91 t_s92 t_s93 t_s94 t_s100 t_s101 t_s102 t_s103 t_s104 t_s105 t_s110 t_s111 t_s112 t_s120 t_s121 t_s122 t_s123 t_s124 t_s130 t_s131 t_s132 t_s133 t_s140 t_s141 t_s142; do
  "$t"
done

rm -rf /tmp/emur-cx; rm -f /tmp/emur-e2e-insert.txt /tmp/emur-e2e-crlf.txt /tmp/emur-e2e-big.txt /tmp/emur-e2e-open.txt /tmp/emur-e2e-comp.txt /tmp/emur-e2e-fcomp.txt /tmp/emur-e2e-h.rs /tmp/emur-e2e-h.py /tmp/emur-e2e-h.json /tmp/emur-e2e-h.md /tmp/emur-e2e-h.txt /tmp/emur-e2e-h2.rs /tmp/emur-e2e-h4.json /tmp/emur-e2e-h5.md /tmp/emur-e2e-arg.txt /tmp/emur-e2e-pipe-s140.out /tmp/emur-e2e-pipe-s142.out

if [ "$FAILED" = 0 ]; then
  echo "ALL TESTS PASS"
  exit 0
else
  echo "SOME TESTS FAILED"
  exit 1
fi
