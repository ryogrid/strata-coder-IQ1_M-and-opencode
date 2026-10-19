// Multi-window rendering: buffer text per window frame (laid out by the
// binary split tree), status line pinned to the bottom row of the screen
// (always displayed: buffer name, modified flag, cursor line/total, and
// position percentage), terminal cursor parked at the active window's point.
// Screen via the curses crate; manual ANSI fallback when the terminal
// size is unknown (not a tty) or curses is not available.
use super::buffer::Buffer;
use super::window::Tree;
use super::highlight::styles;
use super::highlight::detect;
use super::highlight::hchars_of;
use curses::canvas::get_size;

// Hand-rolled ANSI only: the curses crate emits 0-based home sequences
// (ESC[0;0H) and attribute resets that ConPTY-based terminals (Windows
// Terminal) misprocess, which shifts each line by one char. Manual moves
// are always 1-based ESC[<row>;<col>H and every cell is flushed atomically.
pub fn screen_clear() {
    print!("\x1b[2J\x1b[1;1H");
}

pub fn split_lines(text: String) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    let mut current: Vec<char> = vec![];
    for c in text.chars() {
        if c == '\n' {
            lines.push(current.iter().collect::<String>());
            current = vec![];
        } else {
            current.push(c);
        }
    }
    lines.push(current.iter().collect::<String>());
    lines
}

pub fn line_offsets(lines: Vec<String>) -> Vec<usize> {
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

pub fn point_pos(text: String, point: usize) -> Vec<usize> {
    let mut pline = 0;
    let mut pcol = 0;
    let mut seen = 0;
    for c in text.chars() {
        if seen >= point {
            break;
        }
        if c == '\n' {
            pline += 1;
            pcol = 0;
        } else {
            pcol += 1;
        }
        seen += 1;
    }
    vec![pline, pcol]
}

pub fn draw_all(bufs: &[Buffer], mut tree: Tree, active: usize, msg: String) -> Tree {
    let size = get_size();
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

    let mut f = 0;
    while f < frames.len() {
        if frames[f].kind > 0 {
            f += 1; // boundary frames carry no buffer/scroll state
            continue;
        }
        let leaf_idx = frames[f].leaf;
        let buf = &bufs[tree.leaves[leaf_idx].buffer];
        let pos = point_pos(buf.text(), tree.leaves[leaf_idx].point);
        let pline = pos[0];
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
                dash = dash + "-";
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
        let buf = &bufs[tree.leaves[leaf_idx].buffer];
        let lines = split_lines(buf.text());
        let lang = detect(buf.name.clone());
        let toks = styles(buf.text(), lang);
        let offs = line_offsets(lines.clone());
        let mut r = 0;
        while r < frames[i].height {
            let li = tree.leaves[leaf_idx].scroll + r;
            if li < lines.len() {
                let off = offs[li];
                let mut line_styles: Vec<usize> = vec![];
                let mut k = 0;
                while k < lines[li].len() {
                    if off + k < toks.len() {
                        line_styles.push(toks[off + k]);
                    } else {
                        line_styles.push(0);
                    }
                    k += 1;
                }
                draw_cell(frames[i].top + r, frames[i].col, lines[li].clone(), frames[i].width, line_styles);
            } else {
                draw_cell(frames[i].top + r, frames[i].col, "@".to_string(), frames[i].width, vec![]);
            }
            r += 1;
        }
        if leaf_idx == active {
            let pos = point_pos(buf.text(), tree.leaves[leaf_idx].point);
            let pline = pos[0];
            let pcol = pos[1];
            st_line = pline + 1;
            st_total = lines.len();
            let len = buf.len();
            st_pct = if len > 0 { tree.leaves[leaf_idx].point * 100 / len } else { 0 };
            crow = frames[i].top + (pline - tree.leaves[leaf_idx].scroll);
            let maxc = if frames[i].width > 0 { frames[i].width - 1 } else { 0 };
            let pc = if pcol < maxc { pcol } else { maxc };
            ccol = frames[i].col + pc;
            have_cursor = true;
        }
        i += 1;
    }

    let label = if msg.len() > 0 {
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
            crow = rows - 2; // never park the cursor on the status row
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
    let mut idx = 0;
    for c in src {
        if chars.len() >= width {
            break;
        }
        // Control chars (CR, TAB, ...) must never reach the screen: a stray
        // CR over a padded cell wipes the line (CRLF files looked blank).
        let d = if c < ' ' { ' ' } else { c };
        chars.push(d);
        stys.push(if idx < styles.len() { styles[idx] } else { 0 });
        idx += 1;
    }
    while chars.len() < width {
        chars.push(' ');
        stys.push(0);
    }
    // Build the line with SGR escapes injected on style changes; escapes are
    // zero-width so padding/width maths above stays correct.
    let mut out: Vec<char> = vec![];
    for cc in hchars_of("\x1b[0m".to_string()) {
        out.push(cc);
    }
    let mut cur = 0;
    let mut j = 0;
    while j < chars.len() {
        let s = stys[j];
        if s != cur {
            for cc in hchars_of(style_esc(s)) {
                out.push(cc);
            }
            cur = s;
        }
        out.push(chars[j]);
        j += 1;
    }
    if cur != 0 {
        for cc in hchars_of("\x1b[0m".to_string()) {
            out.push(cc);
        }
    }
    let padded = out.iter().collect::<String>();
    print!("\x1b[{};{}H", row + 1, col + 1);
    print!("{}", padded);
    super::terminal::flush_stdout();
}
