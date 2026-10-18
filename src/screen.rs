// Multi-window rendering: status/echo line at the top, buffer text per
// window frame (laid out by the binary split tree), terminal cursor parked
// at the active window's point.
// Screen via the curses crate; manual ANSI fallback when the terminal
// size is unknown (not a tty) or curses is not available.
use super::buffer::Buffer;
use super::window::Tree;
use curses::refresh;
use curses::canvas::get_size;
use curses::canvas::text as canvas_text;
use curses::ansi::move_to;

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
    refresh();

    let height = if rows > 1 { rows - 1 } else { 1 };
    let frames = tree.layout(tree.root, 1, height, 0, cols);

    let mut f = 0;
    while f < frames.len() {
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
    let status = if msg.len() > 0 {
        msg.to_string()
    } else if abuf.modified {
        format!("Buf {} *", abuf.name).to_string()
    } else {
        format!("Buf {}", abuf.name).to_string()
    };
    draw_cell(0, 0, status, cols, tty);

    let mut crow = 1;
    let mut ccol = 0;
    let mut have_cursor = false;
    let mut i = 0;
    while i < frames.len() {
        let leaf_idx = frames[i].leaf;
        let buf = &bufs[tree.leaves[leaf_idx].buffer];
        let lines = split_lines(buf.text());
        let mut r = 0;
        while r < frames[i].height {
            let li = tree.leaves[leaf_idx].scroll + r;
            if li < lines.len() {
                draw_cell(frames[i].top + r, frames[i].col, lines[li].clone(), frames[i].width, tty);
            } else {
                draw_cell(frames[i].top + r, frames[i].col, "@".to_string(), frames[i].width, tty);
            }
            r += 1;
        }
        if leaf_idx == active {
            let pos = point_pos(buf.text(), tree.leaves[leaf_idx].point);
            let pline = pos[0];
            let pcol = pos[1];
            crow = frames[i].top + (pline - tree.leaves[leaf_idx].scroll);
            let maxc = if frames[i].width > 0 { frames[i].width - 1 } else { 0 };
            let pc = if pcol < maxc { pcol } else { maxc };
            ccol = frames[i].col + pc;
            have_cursor = true;
        }
        i += 1;
    }

    if have_cursor {
        if tty {
            move_to(ccol as u8 + 1, crow as u8 + 1);
        } else {
            print!("\x1b[{};{}H", crow + 1, ccol + 1);
        }
    }
    super::terminal::flush_stdout();
    tree
}

fn draw_cell(row: usize, col: usize, line: String, width: usize, tty: bool) {
    let mut chars: Vec<char> = vec![];
    for c in line.chars() {
        if chars.len() >= width {
            break;
        }
        // Control chars (CR, TAB, ...) must never reach the screen: a stray
        // CR over a padded cell wipes the line (CRLF files looked blank).
        let d = if c < ' ' { ' ' } else { c };
        chars.push(d);
    }
    while chars.len() < width {
        chars.push(' ');
    }
    let padded = chars.iter().collect::<String>();
    if tty {
        // canvas::text(x=col, y=row) emits ESC[y;xH (1-based): col=1, y=row+1.
        canvas_text(col as u8 as i8 + 1, row as u8 as i8 + 1, &padded, vec![]);
    } else {
        print!("\x1b[{};{}H", row + 1, col + 1);
        print!("{}", padded);
    }
}
