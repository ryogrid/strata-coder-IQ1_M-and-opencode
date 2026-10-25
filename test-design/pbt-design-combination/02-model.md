# 02 - The full-pane model

## 1. State (mirrors `Ed` in main.rs)

```
Model {
  bufs:    Vec<Buffer>      // #[path] include of src/buffer.rs (verbatim)
  tree:    window::Tree     // #[path] include of src/window.rs (verbatim, pure)
  active:  usize
  msg:     String
  killed:  String
  prev_cmd: u8
  moved:   bool
  undo:    Vec<UndoEntry>   // tag 1 typing / 2 kill / 0 ungrouped, cap 200
  group_alive: bool, touched: bool   // the T-9 touched protocol (as P2 Sim)
}
```

The op loop mirrors the real main loop iteration EXACTLY (main.rs:772-869):

1. top: EOF check n/a (session kept); `sync_in`: clamp the ACTIVE leaf's
   point AND mark to `bufs[bi].len()`, writing BOTH leaf and buffer
   (main.rs:171-184, M-15);
2. `touched = false`; dispatch the op (one `Ed` method mirror each);
3. `sync_out`: buffer point/mark -> active leaf ONLY when `!moved`
   (main.rs:186-191);
4. bottom: `if !touched { group_alive = false }`; `prev_cmd = lead byte`
   (the same byte table as P2, meta/arrow = `0x1b`).

`bi()` = `tree.leaves[active].buffer`. Edits act on the BUFFER
(aliasing leaves share text, keep independent point/mark/scroll -
window.rs:151-156). `C-x o` mirrors `other_window` (main.rs:235-254):
sync_leaf, active = next leaf in `flatten()` order wrapping, `moved=true`.
`C-x 1` mirrors `collapse` (active -> returned 0, remap). `C-x 2/3`
mirror `split` (NEW sibling = exact `{buffer,point,mark,scroll}` copy as
child b; ACTIVE STAYS on the original leaf - main.rs:223-233).
`C-x C-f` mirrors `show_buffer` (single-leaf tree: split-below + attach
NEW leaf active; split tree: replace in place; `attach_leaf` resets
point/mark/scroll = 0 - main.rs:38-65). `C-x k` mirrors kill_buffer
(fresh empty buffer, `undo.retain(|e| e.bi != bi)`, reset EVERY aliasing
leaf - main.rs:356-391). `C-x b` mirrors switch_buffer
(attach/reset; no-match CREATES a new buffer, main.rs:420-425; listform
empty-input CYCLES `bi+1` wrapping - main.rs:410-413).

## 2. Geometry: verbatim, not mirrored

`window::layout` is pure: the model calls the PRODUCTION layout() to get
the `Vec<Frame>` (window.rs:82-125: h1=height/2 + BoundaryRow at
top+h1; height==2 -> 1/1 NO boundary; w1=width/2 + BoundaryCol;
width==2 -> 1/1; width<2 -> S-12 full-overlap). All `Frame` variants are
consumed by the renderer, so the include stays gate-clean (MA-2).

## 3. Renderer (mirror of the screen.rs pipeline, ASCII-pool scope)

Per frame draw (screen.rs:152-291):

| Rule (screen.rs) | Model renderer |
|---|---|
| size: not-a-tty -> 24x80 fixed (screen.rs:145-151) | grid 24 rows x 80 cols, constants (the pty is 80x24; the model never queries termsize) |
| layout height = rows-1, tree never on the last row (screen.rs:154) | tree frames over rows 0..22; row 23 = status/echo |
| scroll prepass per leaf: from stored scroll; `pline < scroll -> scroll = pline`; `pline >= scroll+fheight -> scroll = pline-fheight+1`; write back (screen.rs:182-189) | identical, mutating model leaf.scroll BEFORE frame drawing |
| `split_lines`: trailing final line always; "" -> 1; "\n" -> 2 (screen.rs:97-110) | mirror (unit-pinned in-target) |
| `point_pos` [line, col], point>len clamps (screen.rs:126-141) | mirror; ASCII -> col == char col |
| Window frame: `li = scroll + r`; `li < lines.len()` -> text row; else a SINGLE `@` at the frame's col, SPACES padded across the frame width (`draw_cell(top + r, col, "@", width, &[])`, screen.rs:246 + pad loop screen.rs:330-334; the tmux row reads `@` after trim) - NOT a full `@` row, which false-fails every end-of-buffer row |
| `draw_cell`: control chars (`c < ' '`) -> SPACE; truncate by CELLS; fullwidth drop n/a (ASCII); pad to width (screen.rs:311-364) | mirror incl. control->space (needed: BEL from `Undefined command: C-x \x07` msg, NUL from the C-q quirk) |
| BoundaryRow: '-' x width; BoundaryCol: '|' per row (screen.rs:206-223) | same |
| status: ONE line, row rows-1, ACTIVE leaf only; label precedence msg verbatim > `Buf {name} *` > `Buf {name}`; `format!("{} {}/{} ({}%)", label, st_line, st_total, st_pct)`; `st_pct = (point*100).checked_div(len).unwrap_or(0)`; defaults 1/1 0% (screen.rs:193-278) | mirror with the EXACT format strings |
| cursor park (screen.rs:280-291) | NOT modeled: plain capture cannot see it (05 §3) |
| SGR styles (style_esc) | NOT modeled: plain capture has no SGR layer (05 §1) |

Grid assembly: 2D char grid initialized to spaces; each Frame paints its
segment(s); every grid row is `trim_end`-ed. Comparison: tmux `capture()`
(already per-line `trim_end`, support mod.rs:169-172) row-by-row against
the model grid rows. The status row participates in the same comparison
(msg precedence and exact strings are THE core of P5's status checks).

## 4. Minibuffer machine (mirror of terminal.rs read_line_c + callers)

A prompt op = prefix bytes + scripted tail (a byte list: input chars,
`Tab` 0x09, `Enter` 0x0d, `C-g` 0x07). The model executes the tail
against the documented reader rules (terminal.rs:449-611) - plus the ISARCH custom loop, which lives in main.rs:428-519 (NOT terminal.rs) and whose arms are mirrored in full below:

| Reader behavior | Model |
|---|---|
| Enter over a displayed candidate span selects that candidate; else the typed input (terminal.rs:464-474) | mirror (caret tracking: C-b/C-f/C-a/C-e/arrows, printable insert at caret, backspace; terminal.rs:541-609) |
| Tab: 0 hits -> bell (echo unchanged); 1 hit -> suffix appended to input; >1 -> common prefix + `"  "` + candidates joined by ONE space appended AFTER the input; list becomes selectable; `list_width` = 2 + per-candidate (cells+1) (terminal.rs:481-539, 349-360) | mirror incl. the echo string builder (ASCII: cells == char count) |
| C-g: return `Some("")`; echo gains `" [Cancelled]"` on the prompt row (terminal.rs:476-479) | mirror; caller sees "" (the per-caller effect table below) |
| file completion: `file_candidates(input)` = fs listing after the last `/`, prefix filter, dirs get `/`, sorted (terminal.rs:261-344) | model performs the SAME fs read (deterministic: the harness seeds the dirs it lists; 03 §P6 seeds `/tmp/emur-pbt-<id>-d/`) |
| `expand_tilde` (terminal.rs:244-259; called by C-x C-f/C-w/C-i at main.rs:262/319/337) | mirror; the ASCII pool never generates `~` paths, so the rule is a one-line pass-through (recorded for completeness of the reader->caller path) |
| echo wrap: the prompt echo writes at the LAST screen row; a > 80-cell echo WRAPS and scrolls the pane (capture row 23 shows the overflow fragment, grid shifts) | EXCLUDED: 02 §4 + 03 §P6 pin an echo budget of `prompt + input + list <= 80 CELLS` (ASCII: cells == chars); the model ASSERTS the budget before every mid-prompt comparison (5 §10) |

Per-entry effects (the msg strings are compared VERBATIM on the status
row - screen.rs:271; e2e-pinned wording):

| Entry (op tail) | Success/failure effects (main.rs) |
|---|---|
| `C-x C-f` + path + Enter | reuse-already-showing: msg "" no-op; reuse: show_buffer + moved; fresh load: msg "" + new buffer; read err: `Failed to open {path}: {e}` (NO buffer created; main.rs:15 formats `io::Error` Display - the P5 fail op is ENOENT ONLY, the model pins the literal `No such file or directory (os error 2)`, stable on Linux CI; errno-map rule recorded: reading a DIRECTORY yields `Is a directory (os error 21)` - not generated in v1); empty path: silent (main.rs:256-297) |
| `C-x C-s` (no prompt) | writes the bound filename: ok `Wrote {path}` + modified=false; fail `Failed to write {path}`; UNBOUND -> `No file name for this buffer` (main.rs:299-313; save() at 299). GENERATOR RULE (protects the seed-untouched invariant, gate J2): the op is ONLY issued when the active buffer is UNBOUND (novel-created) or was bound to the OUTPATH by a prior `C-x C-w` in the sequence - a seed-BOUND buffer never receives `C-x C-s` |
| `C-x C-w` + path + Enter | ok: `Wrote {path}` + filename binds + modified=false; fail: `Failed to write {path}` + filename untouched (main.rs:315-331) |
| `C-x C-i` + path + Enter | ok: `Inserted file {path}` (text inserted AT point, undo tag 0 push CONDITIONAL on text change - an EMPTY file inserts nothing and pushes no entry, main.rs:343-344); err: `Failed to open {path}: {e}` (main.rs:333-354) |
| `C-x k` + yes\|no\|C-g + Enter | modified -> prompt `Kill buffer {name}? (yes\|no) `; yes/y -> fresh empty buffer + undo retain + ALL alias leaves reset + `Killed buffer {name}`; else `Cancelled`; unmodified -> NO prompt, proceeds directly (main.rs:356-391) |
| `C-x b` + name + Enter | match: attach+reset+`moved=true`+msg ""; NO match: CREATE empty buffer named sel, attach+reset+`moved=true`, msg "" (main.rs:417-425) |
| `C-x b` + C-g | Cancelled (main.rs:414-415) |
| `C-x C-b` + Enter | listform prompt `Buffers: {names} Enter=next, name: `; empty -> CYCLE bi+1 wrapping (main.rs:410-413); `C-x C-b` + C-g -> empty return -> ALSO cycles (same branch - a designed P6 case) |
| `C-l` + digits + Enter | digits gate; `0`/non-digits -> `Invalid line number: {q}`; past-end clamps to last line start col 0; ok msg ""; empty -> `Cancelled` (main.rs:650-692) |
| `C-r` + q + Enter | empty -> Cancelled; found -> point = match START, msg ""; not found -> `Search string not found: {q}` (main.rs:521-535) |
| `C-s` + chars + Enter\|C-g | FULL loop mirror (main.rs:428-519): per query char, search `find_forward(q, last+1)` then wrap `find_forward(q, 0)`; HIT -> point = match END, echo the char, `last = end` (497-506); MISS -> bell `\x07`, the char is DROPPED from the query string `qs` and point is UNTOUCHED (the qs append is in the hit arm at 501; miss arm 509-510); `C-s` mid-search = repeat the current query from `last+1`, no echo (484-486); DEL `0x7f` pops one query char and RE-SEARCHES from 0, `last = end - qs.len()` (469-480); Enter accepts (break); ESC accepts AND swallows one arrow `ESC [ t` t in 0x41-44 = previous_line/next_line/forward_char/backward_char (447-467); C-g -> point restored to ENTRY value + msg `Cancelled` (468, 513-515); the scripted tail ends with Enter or C-g only (law) |
| `C-x` + `C-g` | `Undefined command: C-x \x07` - BEL renders as SPACE on the status row (draw_cell) - visible in every later frame until replaced (P5 asserts this grid text) |
| ESC + C-g | MetaUnbound = SILENT (msg untouched) |

Mid-prompt snapshot (P6): the echo row string is modeled
(`\x1b[2K` + prompt + input + tab-list suffix, control->space,
trim_end) so a capture taken BETWEEN `Tab` and `Enter` compares exactly.

## 5. Undo

Reuse the P2 Sim logic verbatim (tags, merge, touched protocol, 200 cap,
pop+restore; `C-x k` retain). Multi-buffer: entries carry `bi` and can
target a NON-active buffer; `restore()` sets the buffer's point/mark -
the ACTIVE leaf keeps its own point/mark and `sync_in`/`sync_out` move
states as the loop law says. Status line reflects the active leaf -
undoing buffer B while viewing A shows A's geometry with B's text width
(`st_total` uses the ACTIVE buffer, not the undone one). The model
encodes exactly this (global-stack design, README-documented).
