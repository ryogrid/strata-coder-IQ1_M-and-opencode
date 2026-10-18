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
    match so.flush() {
        Ok(_) => {}
        Err(_) => {}
    }
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
            match tcsetattr(fd, TCSANOW, &t) {
                Ok(_) => {}
                Err(_) => {}
            }
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

// --- minibuffer completion helpers ------------------------------------------

pub fn chars_of(s: String) -> Vec<char> {
    let mut out: Vec<char> = vec![];
    for c in s.chars() {
        out.push(c);
    }
    return out;
}

pub fn str_of(cs: Vec<char>) -> String {
    return cs.iter().collect::<String>();
}

// True when name starts with the input characters.
pub fn has_prefix(name: String, input: String) -> bool {
    let n = chars_of(name);
    let t = chars_of(input);
    if t.len() > n.len() {
        return false;
    }
    let mut i = 0;
    while i < t.len() {
        if n[i] != t[i] {
            return false;
        }
        i += 1;
    }
    return true;
}

// Candidates matching the typed input (all candidates when input is empty).
pub fn completions(input: String, cands: Vec<String>) -> Vec<String> {
    let mut hits: Vec<String> = vec![];
    let mut j = 0;
    while j < cands.len() {
        if has_prefix(cands[j].clone(), input.clone()) {
            hits.push(cands[j].clone());
        }
        j += 1;
    }
    return hits;
}

// Longest common prefix of all candidates.
pub fn common_prefix(list: Vec<String>) -> String {
    if list.len() == 0 {
        return "".to_string();
    }
    let first = chars_of(list[0].clone());
    let mut out: Vec<char> = vec![];
    let mut i = 0;
    while i < first.len() {
        let c = first[i];
        let mut j = 1;
        let mut all = true;
        while j < list.len() {
            let cs = chars_of(list[j].clone());
            if i >= cs.len() || cs[i] != c {
                all = false;
                break;
            }
            j += 1;
        }
        if all {
            out.push(c);
        } else {
            break;
        }
        i += 1;
    }
    return str_of(out);
}

// Path candidates from the filesystem: completes the tail after the last '/'
// against entries of the directory part ('.' when no '/' typed).
pub fn file_candidates(input: String) -> Vec<String> {
    let cs = chars_of(input);
    let mut slash = 0;
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '/' {
            slash = i + 1;
        }
        i += 1;
    }
    let mut head: Vec<char> = vec![];
    i = 0;
    while i < slash {
        head.push(cs[i]);
        i += 1;
    }
    let mut tail: Vec<char> = vec![];
    while i < cs.len() {
        tail.push(cs[i]);
        i += 1;
    }
    let mut dir = ".".to_string();
    if slash > 0 {
        let mut hd: Vec<char> = vec![];
        let mut k = 0;
        while k < slash - 1 {
            hd.push(head[k]);
            k += 1;
        }
        dir = str_of(hd);
    }
    let tail_s = str_of(tail);
    let head_s = str_of(head);
    let mut cands: Vec<String> = vec![];
    match std::fs::read_dir(dir) {
        Ok(rd) => {
            for it in rd {
                match it {
                    Ok(e) => {
                        match e.path().to_str() {
                            Some(fp) => {
                                let fps = fp.to_string();
                                let pcs = chars_of(fps);
                                let mut last = 0;
                                let mut k = 0;
                                while k < pcs.len() {
                                    if pcs[k] == '/' {
                                        last = k + 1;
                                    }
                                    k += 1;
                                }
                                let mut nm: Vec<char> = vec![];
                                k = last;
                                while k < pcs.len() {
                                    nm.push(pcs[k]);
                                    k += 1;
                                }
                                let nms = str_of(nm);
                                if has_prefix(nms.clone(), tail_s.clone()) {
                                    cands.push(format!("{}{}", head_s, nms));
                                }
                            }
                            None => {}
                        }
                    }
                    Err(_) => {}
                }
            }
        }
        Err(_) => {}
    }
    return cands;
}

// Read a line in raw mode with local echo on the bottom (status) row, with
// Tab completion against cands (or the filesystem when file_comp is set).
// On a unique match the suffix is echoed; multiple matches echo their
// common prefix and list the candidates. Returns the completed string.
pub fn read_line_c(prompt: String, cands: Vec<String>, file_comp: bool) -> String {
    let sz = curses::canvas::get_size();
    let row = if sz.rows > 0 { sz.rows } else { 24 };
    print!("\x1b[{};1H\x1b[2K", row);
    print!("{}", prompt);
    flush_stdout();
    let mut out: Vec<char> = vec![];
    loop {
        let b = read_byte();
        if b == 0x0D || b == 0x0A {
            print!("\r\n");
            flush_stdout();
            return str_of(out);
        }
        if b == 0x07 {
            print!(" [Cancelled]\r\n");
            flush_stdout();
            return "".to_string();
        }
        if b == 0x09 { // Tab: minibuffer completion
            let inp = str_of(out.clone());
            let pool = if file_comp { file_candidates(inp.clone()) } else { cands.clone() };
            let hits = completions(inp.clone(), pool);
            if hits.len() == 1 {
                let full = chars_of(hits[0].clone());
                let mut k = chars_of(inp.clone()).len();
                while k < full.len() {
                    out.push(full[k]);
                    print!("{}", full[k]);
                    k += 1;
                }
                flush_stdout();
            } else if hits.len() > 1 {
                let cp = common_prefix(hits.clone());
                let cpc = chars_of(cp);
                let mut k = chars_of(inp.clone()).len();
                while k < cpc.len() {
                    out.push(cpc[k]);
                    print!("{}", cpc[k]);
                    k += 1;
                }
                print!("  ");
                let mut m = 0;
                while m < hits.len() {
                    print!("{} ", hits[m]);
                    m += 1;
                }
                flush_stdout();
            }
            continue;
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

// Read a line of input in raw mode with local echo, printed on the bottom
// (status) row so it replaces the status line like Emacs minibuffer prompts.
pub fn read_line(prompt: String) -> String {
    return read_line_c(prompt, vec![], false);
}

// Minibuffer line with Tab completion over given candidates (buffer names).
pub fn read_line_buffers(prompt: String, names: Vec<String>) -> String {
    return read_line_c(prompt, names, false);
}

// Minibuffer line with Tab completion over the filesystem.
pub fn read_line_files(prompt: String) -> String {
    return read_line_c(prompt, vec![], true);
}
