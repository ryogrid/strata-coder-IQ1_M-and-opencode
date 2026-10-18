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
- Status-line assertions are done against the *cleared* last frame: after
  each dispatch the whole screen is refreshed, so leftover prompts
  (`Find file: ` etc.) never pollute the capture.
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
