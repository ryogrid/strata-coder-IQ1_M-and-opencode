# Emacs Key Bindings (core set)

Legend: `C-x` = Ctrl-x, `M-x` = Meta/Esc-x.

## Self-insert & misc

| Key | Command |
|-----|---------|
| printable char | `self-insert` |
| `C-q` | `self-insert` (quote next key) |
| `C-g` | `cancel` (abort current command / clear echo) |
| `C-x C-c` | `quit` (if any buffer is modified, first asks `Modified buffers exist; exit anyway? (yes|no)` in the minibuffer; only `yes`/`y` exits, anything else cancels; EOF handling as per `C-c` below) |
| `C-c` | `exit` (same confirmation; EOF during key reading exits unconditionally - unsaved changes are lost, no confirmation is possible once stdin is closed; EOF in a minibuffer prompt cancels that command, except at the exit-confirmation prompt where EOF exits) |

## Movement

| Key | Command |
|-----|---------|
| `C-b` | `backward-char` (stops at mark when mark is on the current line) |
| `C-f` | `forward-char` (stops at mark when mark is on the current line) |
| `C-n` | `next-line` |
| `C-p` | `previous-line` |
| `C-a` | `beginning-of-line` |
| `C-e` | `end-of-line` |
| `M-b` | `backward-word` |
| `M-f` | `forward-word` |
| `M-<` | `beginning-of-buffer` (raw ttys cannot portably send `C-<`) |
| `M->` | `end-of-buffer` (raw ttys cannot portably send `C->`) |

## Editing

| Key | Command |
|-----|---------|
| `C-o` | `insert-newline-after` |
| `C-t` | `transpose-chars` (no-op at the buffer start: no char before point) |
| `C-k` | `kill-to-end-of-line` |
| `C-/` (`0x1f`, the `C-_` code) | `undo`: pop the change-run stack (text+point+mark restored, buffer marked modified; consecutive typing / consecutive `C-k` each merge to ONE group, any non-change command breaks the run; 200-entry cap; `C-x k` drops the buffer's entries) |
| `C-d` | `delete-forward` |
| `DEL` (backspace) | `delete-backward` |

## Regions / Mark

| Key | Command |
|-----|---------|
| `C-space` (`C-@`, NUL) | `set-mark` at point (start a region) |

## Files

| Key | Command |
|-----|---------|
| `C-x C-f` | `open-file` (open a file in the active window — when only one window exists it first splits and shows the file below; a buffer for an already-open file is reused (edits kept, no re-read); Tab completes file paths) |
| `C-x b` | `switch-to-buffer` (name in echo/minibuffer line; Tab completes buffer names; creates empty buffer if new) |
| `C-x C-b` | `buffer-list-select` (lists buffer names in the minibuffer; Tab completes them; type a name, or Enter cycles to the next buffer) |
| `C-x C-s` | `save` (write current buffer) |
| `C-x C-w` | `write-file` (save as; binds the filename only on success) |
| `C-x C-i` | `insert-file` (insert file at point) |
| `C-x k` | `kill-buffer` (asks `Kill buffer <name>? (yes|no)` when modified) |

## Windows

| Key | Command |
|-----|---------|
| `C-x 2` | `split-window-below` |
| `C-x 3` | `split-window-right` |
| `C-x o` | `select-window` (cycle) |
| `C-x 1` | `delete-other-windows` |

Split cells are separated on screen by one boundary column of `|` (`C-x 3`)
or one boundary row of `-` (`C-x 2`): each split consumes one column/row,
given up when the two sides would be too cramped (< 3 cols/rows to spare,
then the pair splits at >= 2 without a boundary).

## Search

| Key | Command |
|-----|---------|
| `C-s` | `search-forward` incremental: each typed char extends the query and moves point to the match end (search wraps at the buffer end); a char that matches nowhere rings the bell and is dropped; `C-s` while searching repeats the query; Enter accepts the current match, Ctrl-g cancels (point restored), ESC ends like Enter |
| `C-r` | `search-backward` (whole query read in the minibuffer, ends with Enter) |

## Lines

| Key | Command |
|-----|---------|
| `C-l` | `goto-line`: minibuffer reads a line number, point moves to the start of that line (1-based, clamped to the last line); empty cancels, zero or non-digits report `Invalid line number: <input>` |

## Minibuffer

Tab completion (the rows below) applies to the path and buffer-name
prompts (`C-x C-f` / `C-x C-w` / `C-x b`; `C-x k` only asks a yes/no
confirmation and has no completion). The `C-s` / `C-r` / `C-l` prompts
have NO completion: `C-s` reads the query one char at a time so Tab
extends the query as a literal char; in the `C-r` / `C-l` input lines Tab
is ignored by the inert completion handler (no candidate line is ever
listed in any of the three). File candidates are listed in alphabetical
order with directories ending in `/`; `~` expands to `$HOME` in the file
prompts; Tab with no match rings the bell.

| Key | Command |
|-----|---------|
| `Tab` | `minibuffer-complete`: a unique match echoes the suffix; several matches echo the common prefix and list the candidates after the input (`"  cand1 cand2 ... "`) |
| arrows / `C-b` / `C-f` | move the cursor one char left/right over the echoed line (the typed input and the listed candidates) |
| `C-a` / `C-e` | echo line start / end |
| `DEL` | delete the char left of the cursor (editing the input clears a stale candidate list echo) |
| `Enter` | accept; while the cursor is on a listed candidate that candidate is selected (the switch/open proceeds with it), elsewhere the typed input is accepted (empty input cancels) |
| `C-g` | cancel |

## Display

- Syntax highlighting (automatic, no key): buffer language detected from the
  name/file extension — `rs c cpp h js ts go css`, `py sh rb yaml toml`,
  `json`, `md markdown`, `html xml`, else plain. Styles: keywords cyan,
  strings magenta, comments green, numbers blue, markdown headings and
  HTML tags bold cyan. Comments and multi-line strings use the buffer text,
  not just the visible line.
