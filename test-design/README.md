# End-to-End Test Design (tmux-driven)

Language: English markdown. Tool: GNU `tmux` 3.x (preferred; GNU `screen` is
present but unusable here because our assertions need a scriptable pane
capture that `tmux capture-pane` provides directly).

## Why tmux

The editor under test is a full-screen TUI: it only takes the "curses"
rendering path when stdout is a real tty (`termsize` returns None otherwise),
and its input is single-byte raw-key driven (`termios`). Running it inside a
tmux pane gives exactly what a human user sees:

- stdin/stdout are ttys, raw mode works, terminal size is fixed by the
  harness (`-x 80 -y 24`),
- keystrokes can be injected one dispatch event at a time (`send-keys -l`
  with raw control bytes),
- the visible screen can be read back as plain text
  (`tmux capture-pane -p`) for text assertions.

This complements the unit tests in `src/buffer.rs` (which cover the gap
buffer data model) by covering the UI layer: rendering, key dispatch,
windows, file I/O, and the status/echo line.

## Harness design

One script: `test-design/e2e.sh` (bash). Design points:

- **Isolation.** Each scenario runs in its own tmux session
  (`-s emu-sNN`), created with fixed geometry 80x24. Session teardown at
  the end guarantees no state leaks between tests.
- **Exit observability.** The pane runs
  `sh -c './target/debug/emur; echo EMU_EXITED $?'`, so after a quit
  command (C-x C-c / C-c / EOF) the pane shows `EMU_EXITED 0`.
- **Key injection.** tmux `send-keys -l` sends literal bytes as-is,
  including control bytes, so the harness encodes keys as octal escapes:
  `C-x` is `$'\030'`, `C-s` is `$'\023'`, `C-g` is `$'\a'`, `Meta-b` is
  `$'\033b'`, `DEL` is `$'\177'`. Enter (CR, which `read_line` accepts as
  terminator) is sent via the named key `Enter` so tmux emits a real CR.
- **Timing.** `STEP=0.25` s after each send batch; the editor repaints
  synchronously at the top of its loop, so a fixed small sleep is
  deterministic enough against local execution.
- **Assertions.** `capture-pane -p` text, right-trimmed:
  `expect` (line regex present), `expect_absent` (regex absent),
  `expect_count` (exactly N lines match), and bash-level file content
  checks for file commands.
- **Result.** Every scenario prints `PASS sNN: ...` / `FAIL sNN: ...`;
  the script exits non-zero if any scenario fails.

## Byte table (octal)

| Key    | Byte | Shell literal |
|--------|------|---------------|
| C-a    | 0x01 | `$'\001'` |
| C-b    | 0x02 | `$'\002'` |
| C-d    | 0x04 | `$'\004'` |
| C-g    | 0x07 | `$'\a'` |
| C-j    | 0x0a | `$'\n'` (as literal byte) |
| C-k    | 0x0b | `$'\013'` |
| C-o    | 0x0f | `$'\017'` |
| C-r    | 0x12 | `$'\022'` |
| C-s    | 0x13 | `$'\023'` |
| C-x    | 0x18 | `$'\030'` |
| ESC    | 0x1b | `$'\033'` |
| DEL    | 0x7f | `$'\177'` |

## Scenarios

| ID   | Setup (send-keys sequence)                          | Expected observation |
|------|------------------------------------------------------|----------------------|
| s01 | (none, just boot)                                   | seed line "This is the first line" present; status "Buf *demo* " without star |
| s02 | `hello world`                                       | top content line `hello worldThis is the first line`; status star (modified) |
| s03 | `AAA` C-j `BBB`                                     | `AAA` on its own line; `BBBThis is the first line` on the next |
| s04 | `abc def` Meta-b `X`                                | `abc XdefThis is the first line` (word nav is per point) |
| s05 | C-a C-k                                             | "This is the first line" gone; "of the demo buffer." present |
| s06 | `foo bar` C-x C-w `/tmp/emur-e2e-write.txt` Enter; C-x C-s | file exists and contains `foo bar`; status `Wrote /tmp/emur-e2e-write.txt` |
| s07 | C-x C-f `/tmp/emur-e2e-write.txt` Enter             | status buffer name `Buf /tmp/emur-e2e-write.txt` |
| s08 | C-x C-i `/tmp/emur-e2e-insert.txt` Enter (file holds `-INSERTED-`) | text line contains `-INSERTED-This` |
| s09 | C-x k                                               | status `Killed buffer *demo*`; `@` continuation rows (empty buffer) |
| s10 | C-x 2                                               | seed text line count == 2 (both windows, shared buffer) |
| s11 | C-x 3                                               | one line matches `This is the first line +This is the first line` (40-col side-by-side cells) |
| s12 | C-x 2; C-x o; C-a; `X`; C-x o; `Y`                  | content starts `YXThis is the first line` (per-window point: lower window moved its own point, upper kept 0) |
| s13 | C-x 2; C-x 3; C-x 1                                 | seed text line count == 1 (delete-other-windows) |
| s14 | C-s `zzz` Enter                                     | status `Search string not found: zzz` |
| s15 | C-s C-g (cancel inside read_line)                   | status `Cancelled` |
| s16 | Meta-> ; C-r `buffer` Enter                         | status back to `Buf *demo* ` (success clears msg); no "not found" |
| s17 | C-x C-c                                             | pane shows `EMU_EXITED 0` |

## Combined window-pattern scenarios

These exercise split/erase combinations where global (shared) state
(buffer text) and per-window state (point) interact. The seed line
" This is the first line" is used as a countable landmark; typing
effects are observed through the shared buffer text.

| ID   | Setup                                                          | Expected observation |
|------|-----------------------------------------------------------------|---------------------|
| s20 | C-x 2; C-x 2 (split active, then split active again)          | seed line appears in 3 stacked windows (line count 3) |
| s21 | C-x 2; C-x 3 (right-split the top window)                     | seed line occurs 3 times total (`expect_occ`, two on one pane row) |
| s22 | C-x 2; C-x 3; then type `1` / C-x o `2` / C-x o `3`           | buffer text starts `321This is the first line` (3 windows, each with its own point 0) |
| s23 | C-x 2; C-x o; type `X`; C-x 1; type `Y`                        | `XYThis is the first line` — erase (collapse) preserves the active window's point |
| s24 | C-x 3; C-x o; type `R`; C-x 1; type `S`                        | `RSThis is the first line` (same via a right split) |
| s25 | C-x 2; C-x o `X`; C-x 1; `Y`; C-x 2; C-x o; `Z`               | `ZYXThis is the first line` — erase then re-split inherits points |
| s26 | C-x 2; C-x 3; C-x k; type `K`                                  | status `Killed buffer ...`; buffer is fresh-empty: only `K` on a line, seed text absent (kill works from a 3-window state) |
| s27 | Meta-> ; C-r `buffer` Enter; C-x 1; type `Q`                   | `demo Qbuffer.` — a per-window point found by reverse search survives erase |
| s28 | `foo bar` C-x C-w file; C-x C-f file; C-x 2; C-x o `F`; C-x 1; C-x C-s | status `Wrote /tmp/emur-e2e-write.txt`; file contains `Ffoo bar` |

## Command-line argument scenarios

`start_session <id> [extra-args]` passes the file path as argv[1] to the
editor. When a path argument is given, the editor opens it as the buffer
(name = path, `filename` set) instead of the seeded `*demo*` buffer.

| ID   | Setup                                   | Expected observation |
|------|------------------------------------------|----------------------|
| s30 | argv[1] = `/tmp/emur-e2e-arg.txt`      | status `Buf /tmp/emur-e2e-arg.txt`; file content visible; `*demo*` absent |
| s31 | argv file; type `X`; C-x C-s            | status `Wrote /tmp/emur-e2e-arg.txt` (no prompt needed); file now contains `XFROM ARG FIRST` |

## WSL2 / tty failure-mode scenarios

These target the three failure modes reported on real WSL2 ttys.

| ID   | Setup                                        | Expected observation |
|------|-----------------------------------------------|----------------------|
| s40 | argv[1] = 30-line CRLF (`\r\n`) file          | `CRLF LINE 01`/`23` visible; after 25 x C-n: `CRLF LINE 26` visible and `CRLF LINE 01` scrolled off |
| s41 | argv file; `Up Up Down Right Right Left` (tmux named keys); type `Z` | `LZINE 02 of big file` (arrows moved the cursor); no `ALINE`/`BLINE`/`CLINE`/`DLIN` junk inserted |
| s42 | argv file; C-x C-c; type `echo RESTORED_TTY` + Enter    | `RESTORED_TTY` executes: the controlling tty was restored on exit |

## Kill/paste scenarios (seed buffer)

| ID   | Setup                                | Expected observation |
|------|---------------------------------------|----------------------|
| s50 | C-k; C-y                              | first line gone after C-k; `This is the first line` pasted back by C-y |
| s51 | C-k; C-k; C-y                         | buffer emptied of both rows; one C-y restores both rows (consecutive kills accumulate the newline) |
| s52 | C-k; type `X`; C-y                    | `XThis is the first line`: killed text survives intervening commands, paste at point |

## Buffer / window feature scenarios (this round)

| ID   | Setup                                | Expected observation |
|------|---------------------------------------|----------------------|
| s60 | (boot) + 10 x C-f                    | bottom row shows `Buf *demo* 1/3 (0%)` before typing and `Buf *demo* 1/3 (23%)` after: status is always the bottom row and carries line/total + point-percentage |
| s61 | C-@ ; type `ABCDEFGHIJ`; C-b x3; C-k | line becomes `ABCDEFGThis is the first line` — `C-k` killed only `HIJ` (the region between point and mark), not to end of line |
| s62 | type `ABCDEFGHIJ`; C-b x3; C-@; C-f x2; C-a; type `Z` | `ABCDEFGZHIJThis is the first line` — `C-a` stopped at the region mark (point 7) instead of line start |
| s63 | C-x C-f `/tmp/emur-e2e-open.txt`     | two windows: opened-file buffer below, `*demo*` above; bottom status `Buf /tmp/emur-e2e-open.txt 1/3 (0%)` (filename shown in status) |
| s70 | C-x C-f `/tmp/emur-e2e-sw.txt`; C-x b `*demo*` Enter; Ctrl-x Ctrl-b | bottom window switches back to `*demo*` (status `Buf *demo* 1/3 (0%)`); the minibuffer list still shows `Buffers: *demo* /tmp/emur-e2e-sw.txt` (the switched-away buffer remains) |
| s71 | C-x b `fresh` Enter; type `K`        | active window now a fresh empty buffer: only `K` on its row, seed text absent; status `Buf fresh * 1/1 (100%)` |
| s72 | C-x C-f `/tmp/emur-e2e-cyc.txt`; Ctrl-x Ctrl-b; Enter | minibuffer lists `Buffers: *demo* /tmp/emur-e2e-cyc.txt`; Enter cycles the window to the next buffer: status back to `Buf *demo* 1/3 (0%)` |

## Minibuffer Tab-completion scenarios

| ID   | Setup                                | Expected observation |
|------|---------------------------------------|----------------------|
| s80 | type `TABTEST`; C-x C-w `/tmp/emur-e2e-comp` Tab Enter | Tab completes the unique path to `/tmp/emur-e2e-comp.txt`; status `Wrote ...`; file contains `TABTEST` |
| s81 | C-x b `*demo` Tab Enter              | Tab expands the unique buffer name to `*demo*`; window switches, status `Buf *demo* 1/3 (0%)` |
| s82 | C-x b `aa` Enter; C-x b `ab` Enter; C-x b `a` Tab | ambiguous completion echoes the candidate list `aa ab`; typing `b` Enter then switches: status `Buf ab 1/1 (0%)` |
| s83 | C-x C-f `/tmp/emur-e2e-fcomp` Tab Enter | Tab completes the unique path; split window shows file content; status `Buf /tmp/emur-e2e-fcomp.txt 1/3 (0%)` |

## Running

```
cargo build
bash test-design/e2e.sh
```

All scenarios must print PASS; script exit status is 0 only then.
Scratch files live in /tmp (`/tmp/emur-e2e-*.txt`); the repo tree is not
touched by the harness.

## Notes / constraints found by this harness

- `read_line` uses CR (`Enter`) as the terminator; send Enter as a tmux
  named key, not as a literal `\n`, otherwise the quit byte gets
  swallowed into the query.
- tmux `send-keys -l $'\t'` sends a literal 0x09 Tab byte, so completion
  tests embed `\t` inside the quoted path/name string sent to the prompt.
- Status-line assertions are done against the *cleared* last frame: after
  each dispatch the whole screen is refreshed, so leftover prompts
  (`Find file: ` etc.) never pollute the capture.
- `expect_lastline` compares the *exact* bottom-row string (`=`), used to
  pin the new always-bottom status format `Buf <name>[*] <line>/<total>
  (<pct>%)`. s16's demo seed is 78 chars long so reverse-search point 63
  renders `2/3 (81%)`.
- tmux `C-@` sends a real NUL (0x00) byte, so `C-space` mark tests need
  `read_byte_eof` in `src/terminal.rs` to distinguish NUL from EOF (plain
  `read()` returning `Ok(0)`); the `0x00` key branch is checked before the
  printable/self-insert branches in `src/main.rs`.
- Found by this harness (fixed in `src/terminal.rs`): the raw-mode setup
  kept `IXOFF` (and `IXON`/`XON`) set, so the tty intercepted Ctrl-S (0x13,
  XOFF) as flow control and the editor never received it. Raw mode now
  clears `IXON | IXOFF` in `c_iflag` along with the other input flags
  (`XON` is BSD-only and not defined on Linux).
- s16 moves to buffer end (`Meta->`) before `C-r`, otherwise a reverse
  search from point 0 correctly reports "not found".
- Found by this harness (fixed in `src/main.rs`): `C-x C-w` (write-file)
  did not record `Buffer.filename`, so a subsequent `C-x C-s` (save)
  reported "No file name for this buffer". write-file now sets
  `filename` before writing, matching Emacs semantics.
- Found on WSL2 (fixed in `src/terminal.rs`): the editor left the tty in
  raw mode on exit (SIGINT/EOL/echo broken in the parent shell
  afterwards). `save_raw_mode()` now returns the original `Termios` and
  `restore_mode()` reapplies it after the main loop; s42 asserts the
  shell still reads lines after quit.
- Found on WSL2 (fixed in `src/screen.rs`): control characters from
  buffer content reached the screen. A CRLF (Windows) file carries `\r`
  into every line; the stray CR re-positioned the cursor to column 1
  before the padding spaces, visually wiping whole rows so scrolling
  looked frozen. `draw_cell` now renders any `c < ' '` as a space.
- Found on WSL2 (fixed in `src/main.rs`): `ESC [ A/B/C/D` (arrow keys)
  fell into the Meta handler and the trailing letter was self-inserted.
  Arrow keys now dispatch to previous/next line and backward/forward
  char.
- Found on WSL2 (fixed via `terminal::flush_stdout`): stdout was never
  flushed. Rust line-buffers stdout, the editor emits no `\n` per frame,
  and `refresh()` erases the whole screen first — so a real terminal
  showed a wiped/partially repainted screen (scrolling appeared broken,
  prompts invisible) and the last unflushed frame was lost on exit.
  Every frame and every local-echo step now ends with an explicit flush.
- Reported on Windows Terminal (fixed in `src/screen.rs`): each line lost
  its first character, which rode at the end of the row above. The curses
  crate's `refresh()`/`canvas_text` emit 0-based home codes (`ESC[0;0H`)
  and `ESC[0m` resets around moves, which the ConPTY layer misprocesses.
  Rendering is now hand-rolled ANSI only: 1-based `ESC[<row>;<col>H`
  before every cell, `ESC[2J` + `ESC[1;1H` to clear, and one flush per
  cell so the move and its text always arrive as a single atomic write.
- Reported by the user (fixed): `C-k` followed by `C-y` pasted nothing.
  `C-y` (0x19) had no dispatch branch at all and `kill_to_end_of_line`
  dropped the cut text. kill now returns the killed `String`;
  `src/main.rs` keeps a `killed` accumulator — consecutive `C-k`
  commands concatenate (Emacs-style, newline included), any other
  command ends the accumulation run — and `C-y` re-inserts it at point.
