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
   (4x300 ms)` - poll-until-equal is NOT used for grid diffs (it masks
   lag-vs-wrong-state distinctions), only for the P6 mid-prompt echo
   row (echo appears deterministically once keys are consumed;
   documented exception; the poll target is the echo's LAST chunk, and
   a NEVER-appearing target PANICS - an unobservable mid state is a
   test failure, never a silent pass).
10. **Mid-prompt echo wrap**: the echo writes at the LAST screen row;
    a > w-cell echo WRAPS onto the pane's TRAILING rows as chunks of w
    chars (the tree shifts up). The v1 budget law: echo <= `2 * w`
    cells; `check_mid` compares EVERY chunk row bottom-up (the model
    builds the same chunks - wrapping IS on the comparison surface,
    bounded to 2 rows; 03 §P6 seeds SHORT dirs (`/tmp/e<pid..><tag>`)
    so the buffer-list and file echoes stay inside the budget).
11. **Mid-prompt observable surface**: ONLY the echo row is compared
    mid-prompt (tree rows shift on wrap - excluded by design). Two
    physical facts shape the checkpoints: (a) the prompt echo appears
    AFTER the prefix bytes are consumed, racing the tree repaint -
    hence the echo-chunk poll; (b) a closing `C-g` echo
    (`" [Cancelled]"`) is EPHEMERAL: production's cancel paints the
    msg on the status row via a full-screen repaint that lands at
    once, so the suffixed echo is unobservable. P6 mid cuts therefore
    land BEFORE closing C-gs (stable prompt state), and the
    `[Cancelled]` echo remains MODELED for the final grid (status row).
12. **Status-row truncation**: production draws the status via
    `draw_cell(status, cols)` = cell truncation at the pane width, so
    a long msg (`Failed to open {long path}...`, `Buffers: ...`) cuts
    the ` {line}/{total} ({pct}%)` tail on row 23. The P1
    `parse_status` tail parser is therefore INVALID on truncated rows:
    the combination sanity (`comb_sane`) does NOT run it, and the P5
    status-tail assert is GUARDED by `last_line < PANE_W` (untruncated
    row). Truncation is DOCUMENTED PRODUCTION BEHAVIOR, not a bug (the
    grid diff compares the truncated row verbatim, so it is covered).
13. **tmux driver laws** (hard-won, recorded): `send-keys -H` with NO
    key arguments sends ENTER - `send_hex(&[])` must NEVER be called;
    byte encodings must be derived from the `keymap.rs` tables, not
    memory - sending C-x C-f (`18 06`) where the model ran save
    (`18 13`) opened a Find-file prompt whose reader consumed the NEXT
    op's bytes (the root cause of two mirror-mystery failures). The
    model NEVER sends bytes; the drivers do (`run_prompt` is
    model-only; every caller pairs it with `send_hex`).
14. **Model/production drift**: the renderer/reader mirrors duplicate
   rules that production owns (screen.rs/terminal.rs are not compiled
   in-target). Mitigation: every mirror rule carries the production
   file:line; the design gate verifies rule-for-rule; e2e keeps
   production pinned. Residual risk recorded honestly.
15. **SGR attributes are OUT of the grid comparison**: the full-grid diffs
   compare TEXT (`capture-pane -p`). The two live overlays added later
   (isearch match = yellow bg `43m`, mark..point region = cyan bg `46m`,
   screen.rs `apply_span` - attribute-only, text never touched) therefore
   change NO model grid (P5-P7 re-ran green unchanged). Their visible
   pins are the e2e `-e` captures (s165/s166). Model-side face painting
   = v2 candidate.
