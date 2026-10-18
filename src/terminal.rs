use termios::*;
use std::io::stdin;
use std::io::Read;

// Put the controlling tty into raw mode so control codes arrive as raw bytes.
pub fn enter_raw_mode() {
    let fd = 0; // stdin
    let t = Termios::from_fd(fd);
    match t {
        Ok(t) => {
            let mut t = t;
            t.c_lflag &= !(ICANON | ECHO | ECHOE | ECHOK | ECHONL | ISIG | IEXTEN);
            t.c_oflag &= !OPOST;
            t.c_iflag &= !(INLCR | IGNCR | ICRNL | IGNBRK | IXON | IXOFF);
            tcsetattr(fd, TCSANOW, &t);
        }
        Err(_) => {}, // not a tty (pipe/redirect): nothing to configure
    }
}

// Read a single byte from stdin (raw mode). Returns 0 on EOF.
pub fn read_byte() -> u8 {
    let mut buf = [0u8; 1];
    let r = stdin().lock().read(&mut buf);
    match r {
        Ok(1) => buf[0],
        _ => 0,
    }
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

// Read a line of input in raw mode with local echo. Empty string means
// cancelled (Ctrl-g) or EOF.
pub fn read_line(prompt: String) -> String {
    print!("\x1b[1;1H\x1b[2K"); // home + clear line
    print!("{}", prompt);
    let mut out: Vec<char> = vec![];
    loop {
        let b = read_byte();
        if b == 0x0D || b == 0x0A || b == 0 {
            print!("\r\n");
            return out.iter().collect::<String>();
        }
        if b == 0x07 { // Ctrl-g cancel
            print!(" [Cancelled]\r\n");
            return "".to_string();
        }
        if b == 0x7f { // backspace
            if out.len() > 0 {
                out.pop();
                print!("\x08 \x08");
            }
            continue;
        }
        let c = read_char_from(b);
        out.push(c);
        print!("{}", c);
    }
}
