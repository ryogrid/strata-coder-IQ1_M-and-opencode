use termios::*;
use std::io::stdin;
use std::io::Read;
use std::io::stdout;
use std::io::Write;

// Flush the userspace stdout buffer. Rust line-buffers stdout and the editor
// writes no newlines during frames, so without this the screen (and query
// prompts) would not update until the 4 KiB buffer boundary.
pub fn flush_stdout() {
    let mut so = stdout();
    so.flush();
}

// Put the controlling tty into raw mode so control codes arrive as raw bytes.
pub fn save_raw_mode() -> Option<Termios> {
    let fd = 0; // stdin
    let t = Termios::from_fd(fd);
    match t {
        Ok(t) => {
            let orig = t;
            let mut t = t;
            t.c_lflag &= !(ICANON | ECHO | ECHOE | ECHOK | ECHONL | ISIG | IEXTEN);
            t.c_oflag &= !OPOST;
            t.c_iflag &= !(INLCR | IGNCR | ICRNL | IGNBRK | IXON | IXOFF);
            tcsetattr(fd, TCSANOW, &t);
            return Some(orig);
        }
        Err(_) => {
            return None; // not a tty: nothing configured or saved
        }
    }
}

// Restore the tty settings captured by save_raw_mode so the controlling
// shell is not left in raw mode when the editor exits.
pub fn restore_mode(saved: Option<Termios>) {
    match saved {
        Some(t) => {
            let fd = 0;
            let _ = tcsetattr(fd, TCSANOW, &t);
        }
        None => {}
    }
}

// Read a single byte from stdin (raw mode). Returns [byte, eof]:
// a real NUL byte (Ctrl-space) is Ok(1) and must not be confused with EOF,
// which happens when read() returns no bytes at all.
pub fn read_byte_eof() -> Vec<u8> {
    let mut buf = [0u8; 1];
    let r = stdin().lock().read(&mut buf);
    match r {
        Ok(1) => vec![buf[0], 0],
        _ => vec![0, 1],
    }
}

// Read a single byte from stdin (raw mode). Returns 0 on EOF.
pub fn read_byte() -> u8 {
    read_byte_eof()[0]
}

// Read one UTF-8 code point given its first byte.
pub fn read_char_from(first: u8) -> char {
    let extra: usize =
        if first < 0x80 {
            0
        } else if first >= 0xC0 && first < 0xE0 {
            1
        } else if first >= 0xE0 && first < 0xF0 {
            2
        } else if first >= 0xF0 && first < 0xF8 {
            3
        } else {
            0
        };
    let mut bytes: Vec<u8> = vec![first];
    for _ in 0..extra {
        bytes.push(read_byte());
    }
    match String::from_utf8(bytes) {
        Ok(s) => s.chars().nth(0).unwrap_or(' '),
        _ => ' ',
    }
}

// Read a line of input in raw mode with local echo, printed on the bottom
// (status) row so it replaces the status line like Emacs minibuffer prompts.
pub fn read_line(prompt: String) -> String {
    let sz = curses::canvas::get_size();
    let row = if sz.rows > 0 { sz.rows } else { 24 };
    print!("\x1b[{};1H\x1b[2K", row);
    print!("{}", prompt);
    flush_stdout();
    let mut out: Vec<char> = vec![];
    loop {
        let b = read_byte();
        if b == 0x0D || b == 0x0A || b == 0 {
            print!("\r\n");
            flush_stdout();
            return out.iter().collect::<String>();
        }
        if b == 0x07 { // Ctrl-g cancel
            print!(" [Cancelled]\r\n");
            flush_stdout();
            return "".to_string();
        }
        if b == 0x7f { // backspace
            if out.len() > 0 {
                out.pop();
                print!("\x08 \x08");
                flush_stdout();
            }
            continue;
        }
        let c = read_char_from(b);
        out.push(c);
        print!("{}", c);
        flush_stdout();
    }
}
