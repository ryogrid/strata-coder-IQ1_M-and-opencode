# TODO

Progress tracker. One commit per completed task. Check off as done.

## Notes to remember

- Platform: x64 Linux only; UTF-8 assumed.
- `curses` crate needs system `ncurses`; if absent, fall back to manual ANSI.
- Gap buffer indexes by `char` (code point), not byte.
- Windows are a binary split tree; `C-x o` cycles leaves in tree order.
- Keep `point`/`mark` per-window, not global.
- `C-x` prefix and `M-`/`Esc` prefix need multi-key dispatch.

## Tasks

- [x] 1. Init git repo + write design/ docs + TODO.md, commit
- [x] 2. Cargo scaffold + deps (curses, termios)
- [ ] 3. Terminal I/O layer (raw input via termios, screen via curses)
- [ ] 4. Gap-buffer Buffer type (point/mark, UTF-8 chars)
- [ ] 5. Cursor movement (Ctrl-b/f, Ctrl-n/p, Ctrl-a)
- [ ] 6. Keymap dispatch tree + self-insert + Ctrl-q
- [ ] 7. File load/save (Ctrl-x Ctrl-f/s/w/i, Ctrl-x k)
- [ ] 8. Line/word nav + delete/insert/kill (Ctrl-d, backspace, Ctrl-o, Ctrl-t, Ctrl-k, Meta-b/f)
- [ ] 9. Single-window render + status line + echo
- [ ] 10. Window splitting (Ctrl-x 2/3) + Ctrl-x o
- [ ] 11. Ctrl-x 1 delete-other-windows
- [ ] 12. Search (Ctrl-s/r) + Ctrl-g cancel + quit (Ctrl-x Ctrl-c)
- [ ] 13. Build + smoke test + final commit
