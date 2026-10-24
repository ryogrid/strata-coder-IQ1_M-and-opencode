# Emacs-style Text Editor in Rust

A command-line text editor that emulates the core interaction model of GNU
Emacs: keyboard-driven editing with Emacs key bindings, buffers holding file
contents, and multiple side-by-side windows in a single terminal.

## Platform

- x64 Linux only.
- UTF-8 assumed for all text.
- Terminal driven through `curses` (terminal size query ONLY - drawing
  is hand-written ANSI escapes, see architecture.md "Current
  implementation") and `termios` (raw key input).

## Feature scope

Implemented:

- Basic loading, editing, and saving of files (with buffer reuse for an
  already-open file).
- Window splitting (`split-window-below`, `split-window-right`), window
  selection (`select-window`), and `delete-other-windows`.
- The core Emacs key binding set (movement, editing, files, windows,
  search), plus `M-b`/`M-f` word movement and `M-<`/`M->` buffer bounds
  (raw ttys cannot portably send `C-<`/`C->`, so buffer bounds are
  Meta-bound).
- Incremental search (`C-s`) and backward query search (`C-r`).
- Syntax highlighting: keyword faces (Emacs `font-lock` style,
  `highlight.rs`; see the README "Syntax highlighting" section).
- Minibuffer prompts on the bottom row: file paths, buffer names,
  search queries, go-to-line — with Tab completion for paths and buffer
  names, candidate listing, cursor movement over candidates, and
  Enter-on-candidate selection.

Not implemented (out of scope):

- Undo/redo history, undoable operations. There is NO undo: edits, kills
  and buffer kills are irreversible (the save/kill/exit confirmations
  cover unsaved data only), so destructive operations deserve care.
- A full minibuffer COMMAND framework (the interactive prompt/completion
  above is implemented; there is no command-entry framework).
- Modes, indentation engines.
- Full `Meta`-heavy keymaps beyond the core set above.

## Layout

- `src/` — the editor implementation.
- `design/` — this design documentation and the `TODO.md` task tracker.

See `design/architecture.md`, `design/keybindings.md`, and `design/TODO.md`.
