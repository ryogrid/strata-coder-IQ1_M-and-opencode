// Multi-window rendering: buffer text per window frame (laid out by the
// binary split tree), status line pinned to the bottom row of the screen
// (always displayed: buffer name, modified flag, cursor line/total, and
// position percentage), terminal cursor parked at the active window's point.
// Screen via the termsize crate (SIZE query only, R-5); manual ANSI
// rendering is used in all cases, with a fixed 24x80 fallback when the
// size is unknown (not a tty).
use super::buffer::Buffer;
use super::window::Tree;
use super::highlight::styles;
use super::highlight::detect;
use super::highlight::hchars_of;

// Terminal SIZE via termsize directly (R-5: the curses dependency was a
// thin wrapper around exactly this call). rows == 0 / cols == 0 means
// "size unknown" (not a tty); callers apply their own fallback.
pub fn term_size() -> termsize::Size {
    termsize::get().unwrap_or(termsize::Size { rows: 0, cols: 0 })
}

// Hand-rolled ANSI only: 0-based home sequences (ESC[0;0H) and
// attribute resets are misprocessed by ConPTY-based terminals (Windows
// Terminal), which shifts each line by one char. Manual moves
// are always 1-based ESC[<row>;<col>H and every cell is flushed atomically.
pub fn screen_clear() {
    print!("\x1b[2J\x1b[1;1H");
}

// Per-frame render cache for one buffer (Design Doc B / S-1): text(), lines,
// line offsets and style tokens are each computed ONCE per frame per buffer,
// shared by all windows showing it. Cost is ~15 bytes/char (chars 4B, toks
// 8B, lines 1-3B UTF-8, per-line overhead) - a 451KB buffer peaks ~7-9MB
// per frame, freed when draw_all returns.
struct BufView {
    chars: Vec<char>,
    lines: Vec<String>,
    offs: Vec<usize>,
    toks: Vec<usize>,
}

fn view_of(views: &mut [Option<BufView>], bufs: &[Buffer], bidx: usize) {
    if views[bidx].is_none() {
        let buf = &bufs[bidx];
        let chars: Vec<char> = buf.text().chars().collect();
        let lines = split_lines(&chars);
        let offs = line_offsets(&lines);
        let toks = styles(&chars, detect(&buf.name));
        views[bidx] = Some(BufView { chars, lines, offs, toks });
    }
}

// Display width of one char in terminal CELLS (Design Doc C / S-2): the
// single width source of the codebase - draw_cell and point_pos here, the
// minibuffer echo math in terminal.rs (cells_of) all use it.
// Approximated wcwidth: the listed ranges are 2 cells (CJK ideographs incl.
// ext. A/B+, kana, CJK/fullwidth symbols & forms, Hangul, compat, the emoji
// block U+1F300-1F64F); combining marks are 0; everything else is 1
// (control chars included - draw_cell renders them as a space, consistent).
pub fn char_width(c: char) -> usize {
    let u = c as u32;
    if (0x0300..=0x036F).contains(&u) {
        return 0; // combining diacritics: no advance
    }
    if u < 0x1100 {
        return 1; // ASCII, latin, greek, cyrillic, arabic, hebrew, ...
    }
    const WIDE: [(u32, u32); 14] = [
        (0x1100, 0x11FF),
        (0x2E80, 0x33FF),
        (0x3400, 0x4DBF),
        (0x4E00, 0x9FFF),
        (0xA960, 0xA97F),
        (0xAC00, 0xD7A3),
        (0xF900, 0xFAFF),
        (0xFE10, 0xFE19),
        (0xFE30, 0xFE6F),
        (0xFF00, 0xFF60),
        (0xFFE0, 0xFFE6),
        (0x1F300, 0x1F64F),
        (0x20000, 0x2FFFD),
        (0x30000, 0x3FFFD),
    ];
    let mut k = 0;
    while k < WIDE.len() {
        if u >= WIDE[k].0 && u <= WIDE[k].1 {
            return 2;
        }
        k += 1;
    }
    1
}

pub fn split_lines(chars: &[char]) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    let mut current: Vec<char> = vec![];
    for c in chars {
        if *c == '\n' {
            lines.push(current.iter().collect::<String>());
            current = vec![];
        } else {
            current.push(*c);
        }
    }
    lines.push(current.iter().collect::<String>());
    lines
}

pub fn line_offsets(lines: &[String]) -> Vec<usize> {
    let mut offs: Vec<usize> = vec![];
    let mut acc = 0;
    for l in lines {
        offs.push(acc);
        let mut k = 0;
        for _c in l.chars() {
            k += 1;
        }
        acc += k + 1;
    }
    offs
}

pub fn point_pos(chars: &[char], point: usize) -> Vec<usize> {
    let mut pline = 0;
    let mut pcol = 0; // DISPLAY CELLS (S-2): fullwidth chars advance 2
    for (seen, c) in chars.iter().enumerate() {
        if seen >= point {
            break;
        }
        if *c == '\n' {
            pline += 1;
            pcol = 0;
        } else {
            pcol += char_width(*c);
        }
    }
    vec![pline, pcol]
}

pub fn draw_all(bufs: &[Buffer], mut tree: Tree, active: usize, msg: String) -> Tree {
    let size = term_size();
    let tty = size.rows > 0 && size.cols > 0;
    let mut rows = size.rows as usize;
    let mut cols = size.cols as usize;
    if !tty {
        rows = 24; // not a tty (pipe/redirect): fall back to a fixed screen
        cols = 80;
    }
    screen_clear();
    super::terminal::flush_stdout();

    let height = if rows > 1 { rows - 1 } else { 1 };
    let frames = tree.layout(tree.root, 0, height, 0, cols);

    let mut views: Vec<Option<BufView>> = (0..bufs.len()).map(|_| None).collect();
    // S-10: point_pos is computed ONCE per leaf here and reused by the
    // cursor placement below (was computed twice per active leaf).
    let mut leaf_pos: Vec<(usize, usize)> = vec![(0, 0); tree.leaves.len()];

    let mut f = 0;
    while f < frames.len() {
        if frames[f].kind > 0 {
            f += 1; // boundary frames carry no buffer/scroll state
            continue;
        }
        let leaf_idx = frames[f].leaf;
        let bidx = tree.leaves[leaf_idx].buffer;
        view_of(&mut views, bufs, bidx);
        let pos = point_pos(&views[bidx].as_ref().unwrap().chars, tree.leaves[leaf_idx].point);
        leaf_pos[leaf_idx] = (pos[0], pos[1]);
        let pline = pos[0];
        // S-6: scroll is RECOMPUTED from point every frame (follow the
        // point line by line; NO stateful page turns). The leaf's stored
        // scroll is only the starting point of this frame's clamp, so
        // repeated draws are idempotent - matching README's scroll text.
        let mut scroll = tree.leaves[leaf_idx].scroll;
        if pline < scroll {
            scroll = pline;
        }
        if pline >= scroll + frames[f].height {
            scroll = pline - frames[f].height + 1;
        }
        tree.leaves[leaf_idx].scroll = scroll;
        f += 1;
    }

    let abuf = &bufs[tree.leaves[active].buffer];
    let mut st_line = 1;
    let mut st_total = 1;
    let mut st_pct = 0;

    let mut crow = 1;
    let mut ccol = 0;
    let mut have_cursor = false;
    let mut i = 0;
    while i < frames.len() {
        if frames[i].kind == 1 {
            // Horizontal window boundary: a row of dashes.
            let mut dash = "".to_string();
            let mut q = 0;
            while q < frames[i].width {
                dash += "-";
                q += 1;
            }
            draw_cell(frames[i].top, frames[i].col, dash, frames[i].width, vec![]);
            i += 1;
            continue;
        }
        if frames[i].kind == 2 {
            // Vertical window boundary: a column of bars.
            let mut rr = 0;
            while rr < frames[i].height {
                draw_cell(frames[i].top + rr, frames[i].col, "|".to_string(), 1, vec![]);
                rr += 1;
            }
            i += 1;
            continue;
        }
        let leaf_idx = frames[i].leaf;
        let bidx = tree.leaves[leaf_idx].buffer;
        view_of(&mut views, bufs, bidx);
        let buf = &bufs[bidx];
        let view = views[bidx].as_ref().unwrap();
        let mut r = 0;
        while r < frames[i].height {
            let li = tree.leaves[leaf_idx].scroll + r;
            if li < view.lines.len() {
                let off = view.offs[li];
                let mut line_styles: Vec<usize> = vec![];
                let mut k = 0;
                while k < view.lines[li].len() {
                    if off + k < view.toks.len() {
                        line_styles.push(view.toks[off + k]);
                    } else {
                        line_styles.push(0);
                    }
                    k += 1;
                }
                draw_cell(frames[i].top + r, frames[i].col, view.lines[li].clone(), frames[i].width, line_styles);
            } else {
                draw_cell(frames[i].top + r, frames[i].col, "@".to_string(), frames[i].width, vec![]);
            }
            r += 1;
        }
        if leaf_idx == active {
            let (pline, pcol) = leaf_pos[leaf_idx]; // computed once in the prepass (S-10)
            st_line = pline + 1;
            st_total = view.lines.len();
            let len = buf.len();
            st_pct = (tree.leaves[leaf_idx].point * 100).checked_div(len).unwrap_or(0);
            crow = frames[i].top + (pline - tree.leaves[leaf_idx].scroll);
            // pcol is in DISPLAY CELLS (S-2). Clamped to the last cell; on a
            // fullwidth char straddling the frame edge the caret lands on its
            // 2nd cell and the terminal snaps it back - accepted (Design Doc C).
            let maxc = if frames[i].width > 0 { frames[i].width - 1 } else { 0 };
            let pc = if pcol < maxc { pcol } else { maxc };
            ccol = frames[i].col + pc;
            have_cursor = true;
        }
        i += 1;
    }

    let label = if !msg.is_empty() {
        msg.to_string()
    } else if abuf.modified {
        format!("Buf {} *", abuf.name).to_string()
    } else {
        format!("Buf {}", abuf.name).to_string()
    };
    let status = format!("{} {}/{} ({}%)", label, st_line, st_total, st_pct);
    draw_cell(rows - 1, 0, status, cols, vec![]);

    if have_cursor {
        if rows > 1 && crow > rows - 2 {
            crow = rows - 2; // never park the cursor on the status row.
            // S-7: DEFENSIVE - the layout height is rows-1 (the status
            // row is excluded), and the scroll clamp keeps
            // crow = top + (pline - scroll) <= top + height - 1 <= rows-2
            // for every split shape, so crow > rows-2 is UNREACHABLE
            // while that invariant holds. The guard protects the status
            // row if that ever changes (point/caret logic untouched).
        }
        print!("\x1b[{};{}H", crow + 1, ccol + 1);
        super::terminal::flush_stdout();
    }
    super::terminal::flush_stdout();
    tree
}

pub fn style_esc(s: usize) -> String {
    if s == 1 {
        return "\x1b[36m".to_string();
    }
    if s == 2 {
        return "\x1b[35m".to_string();
    }
    if s == 3 {
        return "\x1b[32m".to_string();
    }
    if s == 4 {
        return "\x1b[34m".to_string();
    }
    if s == 5 {
        return "\x1b[1;36m".to_string();
    }
    "\x1b[0m".to_string()
}

fn draw_cell(row: usize, col: usize, line: String, width: usize, styles: Vec<usize>) {
    let mut chars: Vec<char> = vec![];
    let mut stys: Vec<usize> = vec![];
    let src: Vec<char> = line.chars().collect();
    let mut cells = 0;
    for (idx, c) in src.into_iter().enumerate() {
        // Control chars (CR, TAB, ...) must never reach the screen: a stray
        // CR over a padded cell wipes the line (CRLF files looked blank).
        let d = if c < ' ' { ' ' } else { c };
        let w = char_width(d);
        // Truncate by CELLS and never split a fullwidth char across the
        // frame edge: a wide char that does not fit is not drawn (S-2).
        if cells + w > width {
            break;
        }
        chars.push(d);
        stys.push(if idx < styles.len() { styles[idx] } else { 0 });
        cells += w;
    }
    while cells < width {
        chars.push(' ');
        stys.push(0);
        cells += 1;
    }
    // Build the line with SGR escapes injected on style changes; escapes are
    // zero-width so padding/width maths above stays correct.
    let mut out: Vec<char> = vec![];
    for cc in hchars_of("\x1b[0m") {
        out.push(cc);
    }
    let mut cur = 0;
    let mut j = 0;
    while j < chars.len() {
        let s = stys[j];
        if s != cur {
            for cc in hchars_of(&style_esc(s)) {
                out.push(cc);
            }
            cur = s;
        }
        out.push(chars[j]);
        j += 1;
    }
    if cur != 0 {
        for cc in hchars_of("\x1b[0m") {
            out.push(cc);
        }
    }
    let padded = out.iter().collect::<String>();
    print!("\x1b[{};{}H", row + 1, col + 1);
    print!("{}", padded);
    super::terminal::flush_stdout();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn char_width_matrix() {
        assert!(char_width('a') == 1);
        assert!(char_width(' ') == 1);
        assert!(char_width('\t') == 1); // rendered as space by draw_cell
        assert!(char_width('あ') == 2); // hiragana
        assert!(char_width('ア') == 2); // katakana
        assert!(char_width('、') == 2); // U+3001 CJK punctuation
        assert!(char_width('　') == 2); // fullwidth space
        assert!(char_width('Ａ') == 2); // fullwidth A
        assert!(char_width('가') == 2); // hangul syllable
        assert!(char_width('\u{0301}') == 0); // combining acute
        assert!(char_width('\u{1F600}') == 2); // emoji (covered block)
        assert!(char_width('\u{1F680}') == 1); // KNOWN GAP (Design Doc C §2.1)
        assert!(char_width('é') == 1); // latin-1
        assert!(char_width('\u{FF71}') == 1); // halfwidth katakana: past the FF00..FF60 range
        assert!(char_width('\u{4E00}') == 2); // CJK unified start
        assert!(char_width('\u{33FF}') == 2); // range end of the catch-all
        assert!(char_width('\u{20000}') == 2); // ext. B start
    }
    #[test]
    fn point_pos_cells() {
        let cs: Vec<char> = "あa".chars().collect();
        assert!(point_pos(&cs, 0) == vec![0, 0]);
        assert!(point_pos(&cs, 1) == vec![0, 2]); // past the fullwidth char
        assert!(point_pos(&cs, 2) == vec![0, 3]);
        let cs2: Vec<char> = "a\nあ".chars().collect();
        assert!(point_pos(&cs2, 3) == vec![1, 2]);
    }
    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }
    #[test]
    fn split_lines_counts_trailing_empty() {
        assert!(split_lines(&chars("a\nb\n")) == vec!["a", "b", ""]);
        assert!(split_lines(&chars("a\nb")) == vec!["a", "b"]);
        assert!(split_lines(&chars("")) == vec![""]);
        assert!(split_lines(&chars("\n")) == vec!["", ""]);
    }
    #[test]
    fn line_offsets_count_chars_plus_newline() {
        assert!(line_offsets(&split_lines(&chars("ab\nc\n"))) == vec![0, 3, 5]);
        // multibyte chars count as ONE each (char-based, not bytes)
        assert!(line_offsets(&split_lines(&chars("あb\nx"))) == vec![0, 3]);
    }
    #[test]
    fn point_pos_lines_and_clamp() {
        let c = chars("ab\nc");
        assert!(point_pos(&c, 0) == vec![0, 0]);
        assert!(point_pos(&c, 2) == vec![0, 2]);
        assert!(point_pos(&c, 3) == vec![1, 0]);
        assert!(point_pos(&c, 99) == vec![1, 1]); // clamped to the scan end
        // fullwidth advances 2 cells, combining marks 0 (Design Doc C)
        let c = chars("あb\nx");
        assert!(point_pos(&c, 2) == vec![0, 3]);
        assert!(point_pos(&c, 3) == vec![1, 0]);
        let c = chars("a\u{300}bc");
        assert!(point_pos(&c, 3) == vec![0, 2]);
    }
}
