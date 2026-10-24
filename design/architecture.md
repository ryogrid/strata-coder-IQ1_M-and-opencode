# Architecture

## Data model

### Buffer

A buffer is the editable text for one file. It is backed by a **gap buffer**,
the same conceptual structure Emacs uses: a contiguous array of `char` with a
movable empty gap around the cursor, giving O(1) amortized insert/delete near
the cursor. (In this Rust implementation insertion is O(n) - see "Current
implementation" at the end.)

```
[ chars ... | gap | ... chars ]
            ^point        ^mark (region end)
```

- `point`: cursor position (0-based char index into the visible text).
- `mark`: the other end of the active region (for region ops and search).
- `name`: buffer name (usually the file name).
- `filename`: associated file path, if any.
- `modified`: dirty flag for save.

All indexing is by `char` (Unicode code point), matching Emacs's character
semantics under UTF-8.

### Window

A window is a viewport into a buffer. Windows form a binary split tree:

```
        root
       /    \
   leaf W1  leaf W2
```

Each leaf window stores:

- the buffer it displays,
- its own `point`/`mark` (per-window cursor),
- a viewport rectangle (top row, height, left col, width),
- a scroll offset (first visible line).

Splitting a leaf replaces it with an internal node whose two children are the
original window (resized) and a new sibling window. `Ctrl-x o` cycles through
leaves in tree order. `Ctrl-x 1` collapses the tree to a single leaf.

## Key input

`termios` puts the tty in raw mode so control codes arrive as raw bytes. A
small keymap tree resolves byte sequences:

- self-inserting printable characters,
- control characters (`Ctrl-a`..`Ctrl-z`) mapped to commands,
- prefix keymaps: `Ctrl-x` and `Meta`/`Esc`,
- `Ctrl-q` quotes the next key (forces self-insert).

## Rendering

`curses` draws the screen each frame:

- one status/echo line at the top,
- each window's visible lines within its rectangle,
- a cursor placed at the active window's point.

## Command flow

1. Read a key event (possibly a multi-key sequence).
2. Resolve to an interactive command.
3. Mutate buffers/windows.
4. Redraw.

## Current implementation (as-built)

The sections above describe the target design. Where the shipped code
deviates (measured against `src/` as of 2026-10):

- **Gap buffer insert is O(n)**, not amortized O(1): the gap is a
  `Vec<char>` region and insertions memmove the tail
  (`buffer.rs`; analysis `review/01-buffer.md` B-1). The gap itself is
  real (no per-insert reallocation at the gap).
- **Key dispatch is a flat if-else chain**, not a keymap tree:
  `main.rs` reads a byte (plus prefix byte after `Ctrl-x`, plus `ESC [`
  arrow sequences) and walks `if/else if` comparisons.
- **Rendering is hand-written ANSI escape sequences** printed via
  `print!` (`screen.rs`, `terminal.rs`). `curses` is used ONLY for the
  terminal size query (`curses::canvas::get_size`); with stdout
  redirected the fallback is 24x80.
- **The status/echo line is at the BOTTOM row**, not the top: the
  `Buf <name> <line>/<total> (<pct>%)` status and the minibuffer
  prompts share the last screen row.
