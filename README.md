# emur — an Emacs-style terminal text editor (Rust)

A small full-screen, Emacs-flavored text editor implemented in Rust as a
task-by-task exercise (`design/TODO.md`). It uses a gap buffer, a binary
window-split tree, and pure-ANSI terminal rendering.

Repo layout:

| Path | Contents |
|------|----------|
| `src/` | `main.rs` (key dispatch), `buffer.rs` (gap buffer), `window.rs` (window tree), `screen.rs` (rendering), `terminal.rs` (raw I/O), `highlight.rs` (syntax highlighting) |
| `design/` | design docs + task tracker |
| `test-design/` | end-to-end test design doc + tmux-driven harness (`e2e.sh`) + `release_smoke.sh`; `pbt-design/` + `pbt-design-combination/` property-test design; `pbt-run.sh` serial PBT entry point |
| `pbt-test/` | property-test crate: `buffer_model` (P0, in-process gap-buffer diff), `tmux_fuzz` (P1-P4 proptest, tmux), `combination` (P5 proptest + P6/P7 enumerated matrices: a full-pane grid model from `#[path]` includes of `src/buffer.rs`/`src/window.rs`, diffed row-by-row against `tmux capture-pane`) |
| `docs/` | dev-environment snapshot, design deep-dives (CJK width, keymap dispatch table) |
| `review/` | review finding docs (Japanese, task ledgers) |

## Development environment

Stable build facts only. The volatile development snapshot (host
machine, hardware, local model serving, coding-agent versions) lives in
[docs/dev-env.md](docs/dev-env.md); model IDs are managed solely in
`opencode.json` and are not duplicated here.

Toolchain:

- Rust: `rustc 1.99.1`, `cargo 1.99.1`, language edition 2021.
  Direct dependencies: `termsize` 0.1.9 (terminal SIZE query only —
  rendering is hand-written ANSI escapes), `termios` 0.3.3 (POSIX tty),
  `libc` 0.2 (signal handlers: SIGTERM/SIGHUP/SIGINT/SIGQUIT terminal
  restore).

Runtime requirements:

- OS: x64 Linux only (tested on the WSL2 kernel, `x86_64`).
  `termios`/POSIX tty ioctls are used directly; macOS/Windows are not
  supported (the Windows 11 host develops and runs the editor via WSL2).
- Terminal: any ANSI-capable terminal (tmux, xterm, a real tty).
  The editor auto-detects its size when stdout is a tty
  (`termsize::get`; the direct dependency is `termsize` — the former
  `curses` wrapper was removed as redundant, R-5);
  when stdout is redirected it falls back to 24 rows x 80 columns.
  Input is UTF-8; buffer positions are character indexes. The tty is put
  into raw mode on startup (with `IXON | IXOFF` cleared so Ctrl-S reaches
  the editor instead of being eaten as XOFF flow control) and restored on
  quit.
- The end-to-end harness additionally requires GNU `tmux` (3.4 used).

## Build procedure

```sh
cargo build              # debug build -> target/debug/emur
cargo build --release    # optimized build -> target/release/emur
cargo test               # root unit tests (gap buffer, keymap, window, undo stack)
bash test-design/e2e.sh  # end-to-end tmux harness (requires tmux)
bash test-design/pbt-run.sh  # property tests: P0 in-process + P1-P7 tmux (serial law - no other tmux suite/cargo build while running)
```

Run the editor from a real terminal:

```sh
./target/debug/emur             # opens the seeded *demo* buffer
./target/debug/emur notes.txt   # opens notes.txt: buffer name = path,
                                # filename set, so C-x C-s saves to it
```

## Editor features

- **Buffers** — named buffers backed by a gap buffer; all positions are
  UTF-8 character indexes; each buffer keeps a point, a mark, a modified
  flag and a filename. With no arguments a buffer named `*demo*` is seeded
  with two sample lines; a file-path argument loads that file (buffer name
  = path, filename set so `C-x C-s` saves to it, buffer unmodified). If a
  file cannot be read (missing, unreadable), `C-x C-f`/`C-x C-i`/startup
  report `Failed to open <path>: <reason>` in the status area: `C-x C-f`
  keeps the current window, `C-x C-i` inserts nothing, and startup opens an
  empty buffer with NO filename binding (so `C-x C-s` cannot clobber the
  unreadable file).
- **Insertion and editing** — printable chars self-insert (UTF-8 included);
  `C-j`/`C-m` insert a newline at point; `C-o` inserts a newline after
  point; `C-t` transposes the chars around point (a no-op at the buffer
  start, where there is no char before point); `C-q` inserts the next
  key literally (quote).
- **Movement** — one char, one line, one word (`M-b`/`M-f`), line
  beginning/end, and buffer bounds (`M-<`/`M->`; raw ttys cannot portably
  send `C-<`/`C->`, so the buffer-bound commands are Meta-bound).
- **Page scrolling** — `C-v`/`M-v` move point one screenful DOWN/UP
  (terminal rows - 2 lines); the column is preserved, the window scroll
  follows the point, and buffer bounds clamp (no msg, no undo entry).
- **Delete and kill** — `C-d`/backspace delete around point; `C-k` kills to
  end of line and stops at the region mark when it is ahead on the line;
  consecutive `C-k`s accumulate the killed text (newline included) and any
  other command ends the run; `C-w` (kill-region) cuts the region between
  mark and point (point AND mark land at the region start; an empty region
  does nothing) and consecutive `C-w`s accumulate like `C-k` runs; `C-y`
  pastes the killed text back at point.
- **Undo** — `C-/` (`0x1f`; xterm-family terminals send `Ctrl+/` and
  `Ctrl+_` as this same byte) restores the last change: text, point and
  mark all return, and the buffer is marked modified (an undo is a change
  relative to the file). Consecutive typing (self-insert, `C-j`/`C-m`,
  `C-q`) is ONE undo group, consecutive `C-k` kills are ONE group, and
  any other command breaks the run, so one `C-/` steps back one run.
  The history is bounded (last 200 groups); the stack is editor-global
  (each entry belongs to the buffer that recorded it, so after a buffer
  switch `C-/` can step back into the other buffer's state); `C-x k` drops the entries of
  the killed buffer; with nothing to undo the echo reads
  `No change to undo`.
- **Regions** — `C-@` (`C-space`, NUL) sets the mark at point; when the
  mark is on the current line, `C-a`/`C-e`/`C-b`/`C-f`/`C-k` stop at the
  region edge. For `C-k` the edge is a mark AHEAD of point; a mark
  BEHIND point is not a kill boundary — `C-k` kills to end of line (and
  merges lines when point sits on the newline). EDITING CLEARS the
  region: insertion (typing, `C-j`/`C-m`/`C-o`, `C-y` paste) and
  backspace move the mark to point; re-arm the region with `C-@`.
  `C-d` and `C-k` keep the region.
- **Incremental search** — `C-s`/`C-r` share ONE incremental loop
  (forward/backward), the Emacs isearch model: every typed char extends
  the query and moves point to the next match IN THE CURRENT DIRECTION
  (wrapping once at the buffer edge), and the screen repaints after every
  step - the match scrolls into view as you type, no Enter needed.
  Typing `C-s`/`C-r` INSIDE the search SWITCHES the direction and
  re-searches the current query (`Search: `/`Reverse find: ` echo shows
  it). A forward hit lands point at the match END, a backward hit at the
  match START; `DEL` pops one query char and re-searches from the far
  end; a char that matches nowhere rings the bell and is dropped; Enter
  accepts the current match, `C-g` cancels and restores the original
  point, ESC ends like Enter; arrow keys end the search like Enter and
  then move point as plain arrows do (the whole escape sequence is
  consumed - nothing leaks into the buffer).
- **Go to line** — `C-l` reads a line number in the minibuffer and moves
  point to the start of that line (1-based, clamped to the last line);
  empty input cancels, zero or non-digits report
  `Invalid line number: <input>`.
- **Minibuffer and Tab completion** — file paths, buffer names and search/
  goto queries are typed in the bottom-row minibuffer (Tab completion works
  in path and buffer-name prompts; see Key bindings; the `C-s`/`C-r`
  search echo and the `C-l` goto prompt are plain bottom-row input with
  NO completion). File-prompt listings are
  sorted alphabetically, directories show a trailing `/`, and `~` expands
  to `$HOME` (`C-x C-f` / `C-x C-w` / `C-x C-i`; opening a DIRECTORY
  reports a failure - there is no directory listing). Tab rings the
  terminal bell when NO completion matches. When Tab lists
  several candidates they are echoed after the input: the arrow keys (or
  `C-b`/`C-f`, plus `C-a`/`C-e`) move the cursor over the echoed line, and
  Enter on one of the listed candidates selects it; Enter elsewhere
  accepts the typed input.
- **Fullwidth (CJK) rendering** — display width is measured in terminal
  CELLS: fullwidth chars (kana, CJK ideographs, Hangul, fullwidth
  forms/symbols) render 2 cells wide, and window truncation, cursor
  placement and minibuffer candidate spans are all cell-based (buffer
  positions stay character indexes). The width table is an approximated
  wcwidth (see `docs/design/cjk-display-width.md`): emoji outside
  U+1F300-1F64F count as 1, some holes in the wide ranges count as 2, and
  combining marks count as 0 only in U+0300-036F (other combining ranges
  count as 1);
  when a fullwidth char straddles a window's right edge, the caret lands
  on its 2nd cell and the terminal snaps it back.
- **File operations** — `C-x C-f` opens a file into the active window's
  buffer (with a lone window it first splits below and shows the file there;
  buffer name = path, shown by the status line; opening a file whose buffer
  already exists REUSES that buffer — no duplicate buffer is created and the
  buffer's edits and modified flag are kept, no disk re-read; matching is by
  buffer name, so a buffer saved with `C-x C-w` keeps its own name (e.g.
  `*demo*`) and is NOT matched); `C-x C-s` saves the
  buffer to its filename; `C-x C-w` writes to a named file and records it
  ONLY on success (a failed write leaves the save target untouched), so
  later `C-x C-s` saves there; `C-x C-i` inserts the contents of a file
  at point without re-buffering; `C-x k` kills the current buffer (asks
  `Kill buffer <name>? (yes|no)` first when it has unsaved modifications;
  a fresh empty buffer with the same name).
- **Buffer switching and listing** — `C-x b` switches the active window to
  a buffer named in the minibuffer (Tab completes names; a new name
  creates an empty buffer); `C-x C-b` lists all buffer names in the
  minibuffer — type a name + Enter to select and switch, Enter alone
  cycles to the next buffer.
- **Window splitting** — windows form a binary-split tree: `C-x 2`/`C-x 3`
  split the active window below / right (a full row of `-` / a full column
  of `|` is drawn between the new cells), `C-x o` cycles the active window,
  `C-x 1` deletes the others (fit to one). Right after a split all windows
  show the SAME buffer; `C-x b` and `C-x C-f` switch the buffer of the
  ACTIVE window only, so from then on windows can show different buffers
  (killing a buffer shown by several windows empties it in all of them).
  Each window keeps its own point, mark and scroll position, synced
  through the active window (the e2e harness exercises per-window
  points in scenarios s20-s28, and per-window buffers combined with
  `C-x C-f` + scrolling in s120-s124).
- **Scrolling** — long files scroll automatically: once the point leaves
  the visible rows (via `C-n`/`C-p` or arrow keys), the whole window
  scrolls one row per keystroke past the edge.
- **Status line** — the bottom row always shows `Buf <name> <line>/<total>
  (<pct>%)`: the point position (1-based line out of total, percent of
  buffer length); a `*` before the position marks the buffer as modified;
  a message (`Wrote <path>`, `Cancelled`, `Undo`, `No change to undo`,
  `No file name for this buffer`, `Invalid line number: x`, …) replaces
  the status line and STAYS until a later command sets its own message
  (commands that set none — motions, typing, kills — leave it standing).
- **Quit and exit confirmation** — `C-x C-c`/`C-c` quit; if any buffer has
  unsaved modifications, the editor first asks `Modified buffers exist;
  exit anyway? (yes|no)` in the minibuffer and only `yes` (or `y`) exits,
  anything else cancels. EOF (stdin closed) during key reading exits
  unconditionally WITHOUT asking — unsaved changes are lost (a confirmation
  is impossible once stdin is closed); EOF during a minibuffer prompt
  cancels that command — with one exception: EOF at the exit-confirmation
  prompt exits (the confirmation cannot be answered once stdin is closed).
  Quitting restores the tty settings saved on
  startup — also on panics (the restore guard runs during unwinding) and
  on SIGTERM/SIGHUP/SIGINT/SIGQUIT (SIGKILL stays uncatchable) — so the
  controlling shell keeps working normally (SIGINT, EOL, echo) after the
  editor exits.
- **Syntax highlighting** — automatic, no key: the language is detected
  from the buffer/file-name extension; keywords cyan, strings magenta,
  comments green, numbers blue, markdown headings and HTML tags bold cyan.
  Extensions: `rs c h cpp cc hpp js ts go css` (`//` and `/* */` comments),
  `py sh rb yaml toml` (`#` comments, Python `"""..."""`), `json`,
  `md`/`markdown`, `html`/`xml` (`<!-- -->`); anything else (e.g. `txt`,
  `*demo*`) stays plain. Comments and multi-line strings are matched over
  the buffer text, not just the visible line.
- **Rendering robustness** — control characters in buffer content (e.g.
  `\r` from Windows CRLF files) are rendered as spaces so rows and
  scrolling never corrupt the screen (the buffer itself is unchanged); the
  terminal cursor is never parked on the status row.

## Key bindings

| Key | Command |
|-----|---------|
| printable char | self-insert |
| `C-g` | cancel current command / echo |
| `C-x C-c`, `C-c` | quit; if any buffer has unsaved modifications, first asks `Modified buffers exist; exit anyway? (yes|no)` and only `yes` exits (EOF during key reading exits unconditionally — unsaved changes are lost; EOF in a minibuffer prompt cancels that command) |
| `C-j` / `C-m` | insert newline at point |
| `C-b` / `C-f` | backward / forward one char |
| `C-n` / `C-p` | next / previous line |
| `C-v` / `M-v` | scroll one page down / up: point moves a screenful = terminal rows - 2 lines (column preserved, buffer bounds clamp, the window scroll follows the point) |
| arrow keys | same as `C-p` / `C-n` / `C-f` / `C-b` (ESC [ A/B/C/D) |
| `C-a` / `C-e` | beginning / end of line |
| `M-b` / `M-f` | backward / forward one word |
| `M-<` / `M->` | beginning / end of buffer |
| `C-d` / `DEL` | delete forward / delete backward (backspace) |
| `C-o` | insert newline after point |
| `C-t` | transpose chars around point |
| `C-k` | kill to end of line (stops at the region mark when it is ahead on the line) |
| `C-w` | kill-region: cut the region between mark and point (point AND mark land at the region start; an empty region does nothing); consecutive `C-w`s accumulate the killed text |
| `C-y` | paste killed text at point (consecutive `C-k`s accumulate, including the newline) |
| `C-/` | undo: restore the last change (text + point + mark, buffer marked modified); consecutive typing = one group, consecutive `C-k` = one group, any other command breaks the run; bounded to 200 groups; `C-x k` clears the buffer's entries; echo `Undo` / `No change to undo` (arrives as byte `0x1f`, the same code `C-_` sends) |
| `C-@` (`C-space`, NUL) | set the mark at point (start a region); when mark is on the current line, `C-a`/`C-e`/`C-b`/`C-f`/`C-k` stop at the region edge |
| `C-q` | quote: insert next key literally |
| `C-x C-f` | open file in a new split window below (buffer name = path, status shows it; an existing buffer for that file is reused, not reloaded; prompt in the bottom row, end with Enter; Tab completes a path from the directory listing) |
| `C-x b` | switch-to-buffer: buffer name typed in the bottom-row minibuffer prompt (Tab completes a buffer name), Enter switches the active window to that buffer (creates an empty one if new) |
| `C-x C-b` | list all buffer names in the bottom-row minibuffer; Tab completes among them; type a name + Enter to select and switch, Enter alone cycles to the next buffer |
| `C-x C-s` | save buffer to its file |
| `C-x C-w` | write buffer to a named file (records it as the filename only if the write succeeds) |
| `C-x C-i` | insert contents of a file at point |
| `C-x k` | kill-buffer (asks `yes/no` first when the buffer is modified; starts a fresh empty buffer, same name) |
| `C-x 2` / `C-x 3` | split window below / right |
| `C-x o` | select next window (cycles all windows) |
| `C-x 1` | delete other windows |
| `C-s` | incremental search (forward): each typed char moves point to the next (wrapping) match and repaints the screen, so the match scrolls into view without Enter; typing `C-s` repeats the query, `C-r` switches the search to backward and re-searches; `DEL` pops one query char and re-searches; a char that matches nowhere rings the bell and is dropped; Enter accepts, Ctrl-g cancels (point restored), arrow keys accept and move point |
| `C-r` | incremental search (backward) - the SAME loop as `C-s` (prompt echo `Reverse find: `): searches backward from point, wrapping at the top, point lands at the match START; typing `C-r`/`C-s` inside the search switches direction |
| `C-l` | go to line: number typed in the minibuffer, point moves to the start of that line (1-based, clamped to the last line) |
| `Tab` (in a minibuffer prompt) | complete a file path (file prompts) or a buffer name (`C-x b`, `C-x C-b`); several matches echo the common prefix and list the candidates after the input |
| arrows / `C-b` / `C-f` / `C-a` / `C-e` (in a minibuffer prompt) | move the cursor over the echoed line (the typed input and the listed candidates) |
| Enter (in a minibuffer prompt) | end the prompt/query (empty input cancels); when Tab has listed several candidates, Enter while the cursor is on one of them selects it |
