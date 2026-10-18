// Single-window rendering: status/echo line at the top, buffer text below,
// terminal cursor parked at the window's point.
// Screen via the curses crate; manual ANSI fallback when the terminal
// size is unknown (not a tty) or curses is not available.
use super::buffer::Buffer;
use curses::refresh;
use curses::canvas::get_size;
use curses::canvas::text as canvas_text;
use curses::ansi::move_to;

pub fn draw(buf: &Buffer, msg: String, mut scroll: usize) {
    let size = get_size();
    let tty = size.rows > 0 && size.cols > 0;
    let mut rows = size.rows as usize;
    let mut cols = size.cols as usize;
    if !tty {
        rows = 24; // not a tty (pipe/redirect): fall back to a fixed screen
        cols = 80;
    }
    refresh();

    let text = buf.text();
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

    let mut pline = 0;
    let mut pcol = 0;
    let mut seen = 0;
    for c in text.chars() {
        if seen >= buf.point {
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

    let height = if rows > 1 { rows - 1 } else { 1 };
    if pline < scroll {
        scroll = pline;
    }
    if pline >= scroll + height {
        scroll = pline - height + 1;
    }

    let status = if msg.len() > 0 {
        msg.to_string()
    } else if buf.modified {
        format!("Buf {} *", buf.name).to_string()
    } else {
        format!("Buf {}", buf.name).to_string()
    };
    draw_row(0, status, cols, tty);

    let mut i = 0;
    while i < height {
        let li = scroll + i;
        if li < lines.len() {
            draw_row(1 + i, lines[li].clone(), cols, tty);
        } else {
            draw_row(1 + i, "@".to_string(), cols, tty);
        }
        i += 1;
    }

    let crow = 1 + (pline - scroll);
    let ccol = if pcol + 1 < cols { pcol } else { cols - 1 };
    if tty {
        move_to(ccol as u8 + 1, crow as u8 + 1);
    } else {
        print!("\x1b[{};{}H", crow + 1, ccol + 1);
    }
}

fn draw_row(row: usize, line: String, cols: usize, tty: bool) {
    let mut chars: Vec<char> = vec![];
    for c in line.chars() {
        if chars.len() >= cols {
            break;
        }
        chars.push(c);
    }
    while chars.len() < cols {
        chars.push(' ');
    }
    let padded = chars.iter().collect::<String>();
    if tty {
        // canvas::text(x=col, y=row) emits ESC[y;xH (1-based): col=1, y=row+1.
        canvas_text(1, row as u8 as i8 + 1, &padded, vec![]);
    } else {
        print!("\x1b[{};1H", row + 1);
        print!("{}", padded);
    }
}
