# Emacs Key Bindings (core set)

Legend: `C-x` = Ctrl-x, `M-x` = Meta/Esc-x, `C-\` = Ctrl-backslash.

## Self-insert & misc

| Key | Command |
|-----|---------|
| printable char | `self-insert` |
| `C-q` | `self-insert` (quote next key) |
| `C-g` | `cancel` (abort current command / clear echo) |
| `C-x C-c` | `quit` |
| `C-c` | `exit` (quit editor) |

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
| `C-<` | `beginning-of-buffer` |
| `C->` | `end-of-buffer` |
| `C-d` | `delete-forward` |
| `DEL` (backspace) | `delete-backward` |

## Editing

| Key | Command |
|-----|---------|
| `C-o` | `insert-newline-after` |
| `C-t` | `transpose-chars` |
| `C-k` | `kill-to-end-of-line` |
| `C-d` | `delete-forward` |

## Regions / Mark

| Key | Command |
|-----|---------|
| `C-space` (`C-@`, NUL) | `set-mark` at point (start a region) |

## Files

| Key | Command |
|-----|---------|
| `C-x C-f` | `open-file-window` (open file in a new split window below; Tab completes file paths) |
| `C-x b` | `switch-to-buffer` (name in echo/minibuffer line; Tab completes buffer names; creates empty buffer if new) |
| `C-x C-b` | `buffer-list-select` (lists buffer names in the minibuffer; Tab completes them; type a name, or Enter cycles to the next buffer) |
| `C-x C-s` | `save` (write current buffer) |
| `C-x C-w` | `write-file` (save as) |
| `C-x C-i` | `insert-file` (insert file at point) |
| `C-x k` | `kill-buffer` |

## Windows

| Key | Command |
|-----|---------|
| `C-x 2` | `split-window-below` |
| `C-x 3` | `split-window-right` |
| `C-x o` | `select-window` (cycle) |
| `C-x 1` | `delete-other-windows` |

## Search

| Key | Command |
|-----|---------|
| `C-s` | `search-forward` (read query in echo line) |
| `C-r` | `search-backward` |

## Display

- Syntax highlighting (automatic, no key): buffer language detected from the
  name/file extension — `rs c cpp h js ts go css`, `py sh rb yaml toml`,
  `json`, `md markdown`, `html xml`, else plain. Styles: keywords cyan,
  strings magenta, comments green, numbers blue, markdown headings and
  HTML tags bold cyan. Comments and multi-line strings use the buffer text,
  not just the visible line.
