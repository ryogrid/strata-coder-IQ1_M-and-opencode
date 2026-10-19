# emur — an Emacs-style terminal text editor (Rust)

A small full-screen, Emacs-flavored text editor implemented in Rust as a
task-by-task exercise (`design/TODO.md`). It uses a gap buffer, a binary
window-split tree, and pure-ANSI terminal rendering.

Repo layout:

| Path | Contents |
|------|----------|
| `src/` | `main.rs` (key dispatch), `buffer.rs` (gap buffer), `window.rs` (window tree), `screen.rs` (rendering), `terminal.rs` (raw I/O), `highlight.rs` (syntax highlighting) |
| `design/` | design docs + task tracker |
| `test-design/` | end-to-end test design doc + tmux-driven harness |

## Development environment

- Coding agent: OpenCode v1.18.31, model **Qwen3.8-Flash-Next Coder
  IQ1_M (Strata)** (model ID `strata/qwen3.8-flash-next-coder`, served by a
  local OpenAI-compatible endpoint; configured in `opencode.json` under the
  display name "Qwen3.8-Flash-Next Coder (Strata)").
- Rust toolchain: `rustc 1.99.1`, `cargo 1.99.1`, language edition 2021.
  Dependencies: `curses` 0.1.0 (ANSI canvas), `termios` 0.3.3 (POSIX tty).
- OS requirement: x64 Linux only (tested on a WSL2 kernel, `x86_64`).
  `termios`/POSIX tty ioctls are used directly; macOS/Windows are not
  supported.
- Terminal requirement: any ANSI-capable terminal (tmux, xterm, a real tty).
  The editor auto-detects its size when stdout is a tty (`termsize`);
  when stdout is redirected it falls back to 24 rows x 80 columns.
  Input is UTF-8; buffer positions are character indexes. The tty is put
  into raw mode on startup (with `IXON | IXOFF` cleared so Ctrl-S reaches
  the editor instead of being eaten as XOFF flow control) and restored on
  quit.
  The end-to-end harness additionally requires GNU `tmux` (3.4 used).

## Build procedure

```sh
cargo build              # debug build -> target/debug/emur
cargo build --release    # optimized build -> target/release/emur
cargo test               # unit tests (gap-buffer data model)
bash test-design/e2e.sh  # end-to-end tmux harness (requires tmux)
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
  = path, filename set so `C-x C-s` saves to it, buffer unmodified,
  missing files open empty).
- **Insertion and editing** — printable chars self-insert (UTF-8 included);
  `C-j`/`C-m` insert a newline at point; `C-o` inserts a newline after
  point; `C-t` transposes the chars around point; `C-q` inserts the next
  key literally (quote).
- **Movement** — one char, one line, one word (`M-b`/`M-f`), line
  beginning/end, and buffer bounds (`M-<`/`M->`; raw ttys cannot portably
  send `C-<`/`C->`, so the buffer-bound commands are Meta-bound).
- **Delete and kill** — `C-d`/backspace delete around point; `C-k` kills to
  end of line and stops at the region mark when it is ahead on the line;
  consecutive `C-k`s accumulate the killed text (newline included) and any
  other command ends the run; `C-y` pastes the killed text back at point.
- **Regions** — `C-@` (`C-space`, NUL) sets the mark at point; when the
  mark is on the current line, `C-a`/`C-e`/`C-b`/`C-f`/`C-k` stop at the
  region edge.
- **Incremental forward search** — `C-s` reads the query one char at a
  time: each typed char extends the query, moves point to the match end,
  and the search wraps at the buffer end; a char that matches nowhere
  rings the bell and is dropped; `C-s` while searching repeats the current
  query; Enter accepts the current match, `C-g` cancels and restores the
  original point, ESC ends like Enter.
- **Backward search** — `C-r` reads the whole query in the bottom-row
  minibuffer (Enter-terminated); a miss reports
  `Search string not found: <query>` in the status line.
- **Go to line** — `C-l` reads a line number in the minibuffer and moves
  point to the start of that line (1-based, clamped to the last line);
  empty input cancels, zero or non-digits report
  `Invalid line number: <input>`.
- **Minibuffer and Tab completion** — file paths, buffer names and search/
  goto queries are typed in the bottom-row minibuffer (Tab completion works
  in path and buffer-name prompts; see Key bindings).
- **File operations** — `C-x C-f` opens a file in a new split window below
  (buffer name = path, shown by the status line); `C-x C-s` saves the
  buffer to its filename; `C-x C-w` writes to a named file and records it,
  so later `C-x C-s` saves there; `C-x C-i` inserts the contents of a file
  at point without re-buffering; `C-x k` kills the current buffer (a fresh
  empty buffer with the same name).
- **Buffer switching and listing** — `C-x b` switches the active window to
  a buffer named in the minibuffer (Tab completes names; a new name
  creates an empty buffer); `C-x C-b` lists all buffer names in the
  minibuffer — type a name + Enter to select and switch, Enter alone
  cycles to the next buffer.
- **Window splitting** — windows form a binary-split tree: `C-x 2`/`C-x 3`
  split the active window below / right, `C-x o` cycles the active window,
  `C-x 1` deletes the others (fit to one). All windows show the same
  buffer text; each window keeps its own point, mark and scroll position,
  synced through the active window (the e2e harness exercises per-window
  points in scenarios s20-s28).
- **Scrolling** — long files scroll automatically: once the point leaves
  the visible rows (via `C-n`/`C-p` or arrow keys), the whole window
  scrolls one row per keystroke past the edge.
- **Status line** — the bottom row always shows `Buf <name> <line>/<total>
  (<pct>%)`: the point position (1-based line out of total, percent of
  buffer length); a `*` before the position marks the buffer as modified;
  a transient message (`Wrote file`, `Cancelled`, `Search string not
  found: q`, `Invalid line number: x`, …) replaces the status line for one
  frame.
- **Quit and exit confirmation** — `C-x C-c`/`C-c` quit; if any buffer has
  unsaved modifications, the editor first asks `Modified buffers exist;
  exit anyway? (yes|no)` in the minibuffer and only `yes` (or `y`) exits,
  anything else cancels; closing stdin / EOF exits unconditionally. Quitting
  restores the tty settings saved on startup, so the controlling shell keeps
  working normally (SIGINT, EOL, echo) after the editor exits.
- **Syntax highlighting** — automatic, no key: the language is detected
  from the buffer/file-name extension; keywords cyan, strings magenta,
  comments green, numbers blue, markdown headings and HTML tags bold cyan.
  Extensions: `rs c cpp h js ts go css` (`//` and `/* */` comments),
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
| `C-x C-c`, `C-c` | quit; if any buffer has unsaved modifications, first asks `Modified buffers exist; exit anyway? (yes|no)` and only `yes` exits (closing stdin / EOF exits unconditionally) |
| `C-j` / `C-m` | insert newline at point |
| `C-b` / `C-f` | backward / forward one char |
| `C-n` / `C-p` | next / previous line |
| arrow keys | same as `C-p` / `C-n` / `C-f` / `C-b` (ESC [ A/B/C/D) |
| `C-a` / `C-e` | beginning / end of line |
| `M-b` / `M-f` | backward / forward one word |
| `M-<` / `M->` | beginning / end of buffer |
| `C-d` / `DEL` | delete forward / delete backward (backspace) |
| `C-o` | insert newline after point |
| `C-t` | transpose chars around point |
| `C-k` | kill to end of line (stops at the region mark when it is ahead on the line) |
| `C-y` | paste killed text at point (consecutive `C-k`s accumulate, including the newline) |
| `C-@` (`C-space`, NUL) | set the mark at point (start a region); when mark is on the current line, `C-a`/`C-e`/`C-b`/`C-f`/`C-k` stop at the region edge |
| `C-q` | quote: insert next key literally |
| `C-x C-f` | open file in a new split window below (buffer name = path, status shows it; prompt in the bottom row, end with Enter; Tab completes a path from the directory listing) |
| `C-x b` | switch-to-buffer: buffer name typed in the bottom-row minibuffer prompt (Tab completes a buffer name), Enter switches the active window to that buffer (creates an empty one if new) |
| `C-x C-b` | list all buffer names in the bottom-row minibuffer; Tab completes among them; type a name + Enter to select and switch, Enter alone cycles to the next buffer |
| `C-x C-s` | save buffer to its file |
| `C-x C-w` | write buffer to a named file |
| `C-x C-i` | insert contents of a file at point |
| `C-x k` | kill-buffer (start a fresh empty buffer, same name) |
| `C-x 2` / `C-x 3` | split window below / right |
| `C-x o` | select next window (cycles all windows) |
| `C-x 1` | delete other windows |
| `C-s` | incremental forward search: type the query one char at a time, each char moves point to the end of the (wrapping) match; a char that matches nowhere rings the bell and is dropped; `C-s` repeats the query, Enter accepts, Ctrl-g cancels (point restored) |
| `C-r` | backward search: whole query typed in the bottom-row minibuffer, ends with Enter |
| `C-l` | go to line: number typed in the minibuffer, point moves to the start of that line (1-based, clamped to the last line) |
| `Tab` (in a minibuffer prompt) | complete a file path (file prompts) or a buffer name (`C-x b`, `C-x C-b`) |
| Enter (in a minibuffer prompt) | end the prompt/query (empty input cancels) |
