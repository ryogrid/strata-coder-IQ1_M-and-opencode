# 05 - Known gaps and accepted v1 non-goals

1. **Style layer (SGR)**: plain `capture-pane -p` has no attributes;
   highlighting (`style_esc`, keyword cyan etc.) is NOT compared. Pinned
   by e2e s90-s94 (`cap_e` ANSI asserts). A future `-e` capture layer
   would extend P5 to styled cells (recorded, not v1).
2. **Wide chars / CJK**: the P5-P7 pools are ASCII + space + `\n` only.
   `char_width` 2-cell rules (14 ranges, the U+1F680 gap, combining
   0-width) stay pinned by screen.rs unit tests + e2e; fullwidth
   truncation/straddling geometry is out of the v1 comparison surface
   (the renderer mirror assumes width 1 - stated, not silently true).
3. **Cursor park coordinates**: unobservable through plain capture
   (no `display -p '#{cursor_x}:#{cursor_y}'` layer in v1; recorded as
   the cheapest P5 v2 extension: compare `tmux display-message -p`
   against the model's `crow/ccol` rules incl. the rows-2 defensive
   clamp).
4. **EOF paths**: piped-stdin EOF behaviors (exit-confirm data loss,
   C-q NUL quirk, `C-x` + EOF = byte 0 `Undefined command: C-x \0`)
   stay in e2e pins - s140 (exit-confirm EOF) and s142 (prompt EOF,
   the C-r cancel path); the C-q NUL quirk and `C-x` + EOF byte-0
   `Undefined command: C-x \0` have NO e2e pin (recorded honestly; the
   tmux session has no EOF source
   mid-sequence. `C-x` + `C-g` (0x07 as prefix tail) IS covered (P5/P7).
5. **isearch mid-loop captures**: the `Search: ` header row is visible
   mid-search on the echo row, but per-char incremental captures are
   timing-sensitive (bell/echo interleaving); P6 asserts the final
   grid after Enter/C-g only, plus the reader-level mid-prompt captures
   that ARE stable (file/buffer completion rows). Mid-isearch grid
   comparison = v2 candidate.
6. **Resize + splits geometry**: P5/P6/P7 run fixed 80x24 (P4 owns
   resize; DEEP `C-x 3` cascades into the S-12 full-overlap cascade at
   the FIXED 80x24 ARE in v1 scope (P5's random splits can chain ~7
   deep; the grid paint order = the production `frames` Vec order
   reproduces the overlap garble deterministically). resize+nested-split
   geometry at W<80 (P4 x splits) is NOT v1; P7's 60x20 cells stay <= 2
   splits (no cascade).
7. **Quit/confirm**: `C-c`/`C-x C-c` excluded (liveness law); the
   exit-confirm minibuffer is pinned by e2e s111 (interactive yes/y)
   and s140 (EOF).
8. **Timing**: fixed settles (no frame-sync). If a checkpoint ever
   flakes, the settle ladder is `SETTLE_CHECK_MS -> capture_poll
   (3x300 ms)` - poll-until-equal is NOT used for grid diffs (it masks
   lag-vs-wrong-state distinctions), only for the P6 mid-prompt echo
   row (echo appears deterministically once keys are consumed;
   documented exception).
10. **Mid-prompt echo wrap**: the echo writes at the LAST screen row; a
    > 80-cell echo WRAPS, scrolls the pane, and row 23 shows the
    overflow fragment. P5/P6 pin an ECHO BUDGET LAW
    (`prompt + input + list <= 80 cells`, ASCII pool, model asserts
    before every mid-prompt comparison); wrapped echoes are out of the
    comparison surface (03 §P6 setup sizes the seeded dir to fit).
11. **Model/production drift**: the renderer/reader mirrors duplicate
   rules that production owns (screen.rs/terminal.rs are not compiled
   in-target). Mitigation: every mirror rule carries the production
   file:line; the design gate verifies rule-for-rule; e2e keeps
   production pinned. Residual risk recorded honestly.
