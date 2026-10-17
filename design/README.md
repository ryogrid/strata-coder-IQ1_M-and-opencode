# Emacs-style Text Editor in Rust

A command-line text editor that emulates the core interaction model of GNU
Emacs: keyboard-driven editing with Emacs key bindings, buffers holding file
contents, and multiple side-by-side windows in a single terminal.

## Platform

- x64 Linux only.
- UTF-8 assumed for all text.
- Terminal driven through `curses` (screen) and `termios` (raw key input).

## Feature scope

Implemented:

- Basic loading, editing, and saving of files.
- Window splitting (`split-window-below`, `split-window-right`), window
  selection (`select-window`), and `delete-other-windows`.
- The core Emacs key binding set (movement, editing, files, windows, search).

Not implemented (out of scope):

- Undo/redo history, undoable operations.
- Full minibuffer command framework, `Meta`-heavy keymaps beyond the core set.
- Modes, syntax highlighting, indentation engines.

## Layout

- `src/` — the editor implementation.
- `design/` — this design documentation and the `TODO.md` task tracker.

See `design/architecture.md`, `design/keybindings.md`, and `design/TODO.md`.
