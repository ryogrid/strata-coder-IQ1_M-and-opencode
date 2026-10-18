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
  Input is UTF-8; buffer positions are character indexes.
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

## Editor usage

With no arguments, starts a buffer named `*demo*` seeded with two sample
lines; with an optional file path as the first argument, that file is
loaded into the buffer (buffer name = path, file name set, unmodified —
missing files open empty). The bottom row is
always the status line: `Buf <name> <line>/<total> (<pct>%)` — the trailing
`<line>/<total>` and `<pct>%` give the point position in the buffer (1-based
line, percent of buffer length); a trailing `*` before the position marks
the buffer as modified; when a transient message exists (e.g. `Wrote file`,
`Cancelled`, `Search string not found: q`) it replaces the status line for
one frame. All commands are Emacs-style control keys: `C-x` is Ctrl-x, `M-x`
is Meta/Esc followed by the key. Buffer text is syntax-highlighted by the
language detected from the buffer/file-name extension: keywords cyan, strings
magenta, comments green, numbers blue, markdown headings / HTML tags bold
cyan. Supported extensions: `rs c cpp h js ts go css` (C-family: `//` and
`/* */` comments), `py sh rb yaml toml` (`#` comments, Python `"""..."""`),
`json`, `md`/`markdown`, `html`/`xml` (`<!-- -->`); anything else (e.g. `txt`,
`*demo*`) stays plain.

| Key | Command |
|-----|---------|
| printable char | self-insert |
| `C-g` | cancel current command / echo |
| `C-x C-c`, `C-c` | quit (closing stdin / EOF also exits) |
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
| `C-s` / `C-r` | search forward / backward (query ends with Enter) |

Notes:

- Windows all show the same buffer text; each window keeps its own point
  (and mark) state, synced through the active window; the e2e harness
  exercises per-window points (scenarios s20-s28; `mark` is mirrored but
  not directly exercised yet).
- Raw ttys cannot portably send `C-<` / `C->`, so buffer bounds are bound
  to `M-<` / `M->` (Esc then `<` / `>`).
- `C-s` is only received by the editor because raw mode clears
  `IXON | IXOFF` (see `src/terminal.rs`); otherwise the tty would eat it
  as XOFF flow control.
- Long files scroll: `C-n` / `C-p` (or arrow keys) move the point one
  line at a time, and once the point leaves the visible rows the whole
  window scrolls with it (one row per keystroke past the edge).
- Buffer text may contain control characters (e.g. `\r` from Windows
  CRLF files); the renderer replaces them with spaces so lines and
  scrolling never corrupt the screen. The buffer itself is unchanged.
- Quitting restores the tty settings that were saved on startup, so the
  controlling shell keeps working normally (SIGINT, EOL, echo) after the
  editor exits.
