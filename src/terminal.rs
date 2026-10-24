use termios::*;
use std::io::stdin;
use std::io::Read;
use std::io::stdout;
use std::io::Write;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering;

// Flush the userspace stdout buffer. Rust line-buffers stdout and the editor
// writes no newlines during frames, so without this the screen (and query
// prompts) would not update until the 4 KiB buffer boundary.
pub fn flush_stdout() {
    let mut so = stdout();
    let _ = so.flush();
}

// Put the controlling tty into raw mode so control codes arrive as raw bytes.
// Returns a guard restoring the original settings when dropped (normal exit,
// early return, panic unwinding), so raw mode never leaks. The saved termios
// is also parked in a static slot for the signal handlers (Design Doc A).
pub struct TtyGuard {
    saved: Option<Termios>,
}

static SAVED_TERMIOS: AtomicPtr<Termios> = AtomicPtr::new(std::ptr::null_mut());

pub fn save_raw_mode() -> TtyGuard {
    let fd = 0; // stdin
    let t = Termios::from_fd(fd);
    match t {
        Ok(t) => {
            let orig = t;
            let mut t = t;
            t.c_lflag &= !(ICANON | ECHO | ECHOE | ECHOK | ECHONL | ISIG | IEXTEN);
            t.c_oflag &= !OPOST;
            t.c_iflag &= !(INLCR | IGNCR | ICRNL | IGNBRK | IXON | IXOFF);
            let _ = tcsetattr(fd, TCSANOW, &t);
            // One-time leak (process-lifetime storage): shared with the
            // signal handlers through the static slot.
            SAVED_TERMIOS.store(Box::leak(Box::new(orig)) as *mut Termios, Ordering::Relaxed);
            TtyGuard { saved: Some(orig) }
        }
        Err(_) => {
            TtyGuard { saved: None } // not a tty: nothing configured
        }
    }
}

impl Drop for TtyGuard {
    fn drop(&mut self) {
        restore_mode(self.saved);
    }
}

// Restore the tty settings captured by save_raw_mode so the controlling
// shell is not left in raw mode when the editor exits.
pub fn restore_mode(saved: Option<Termios>) {
    if let Some(t) = saved {
        let fd = 0;
        let _ = tcsetattr(fd, TCSANOW, &t);
    }
}

// SIGTERM/SIGHUP/SIGINT/SIGQUIT: restore the tty, then exit. Only external
// kills reach here (ISIG is cleared, so the tty never raises these; do not
// "test" with Ctrl-C); SIGKILL stays uncatchable - raw-mode leak is that
// path's inherent limit.
extern "C" fn restore_on_signal(sig: i32) {
    let p = SAVED_TERMIOS.load(Ordering::Relaxed);
    if !p.is_null() {
        unsafe {
            let _ = tcsetattr(0, TCSANOW, &*p);
        }
    }
    unsafe {
        libc::_exit(128 + sig);
    }
}

pub fn install_restore_signals() {
    unsafe {
        libc::signal(libc::SIGTERM, restore_on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGHUP, restore_on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, restore_on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGQUIT, restore_on_signal as *const () as libc::sighandler_t);
    }
}

// Read a single byte from stdin (raw mode). Returns [byte, eof]:
// a real NUL byte (Ctrl-space) is Ok(1) and must not be confused with EOF,
// which happens when read() returns no bytes at all.
pub fn read_byte_eof() -> Vec<u8> {
    let mut buf = [0u8; 1];
    loop {
        let r = stdin().lock().read(&mut buf);
        match r {
            Ok(1) => return vec![buf[0], 0],
            Ok(_) => return vec![0, 1], // read() delivered no byte: EOF
            // std does NOT auto-retry EINTR on StdinLock::read (it surfaces
            // Err(Interrupted), cf. rust-lang/rust#87337): this loop is the retry.
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return vec![0, 1],
        }
    }
}

// Read a single byte from stdin (raw mode). Returns 0 on EOF.
pub fn read_byte() -> u8 {
    read_byte_eof()[0]
}

// Read one UTF-8 code point given its first byte. Returns None when stdin
// hits EOF mid-sequence (continuation bytes never arrived): callers must
// then NOT insert anything.
pub fn read_char_eof(first: u8) -> Option<char> {
    let extra: usize =
        if first < 0x80 {
            0
        } else if (0xC0..0xE0).contains(&first) {
            1
        } else if (0xE0..0xF0).contains(&first) {
            2
        } else if (0xF0..0xF8).contains(&first) {
            3
        } else {
            0
        };
    let mut bytes: Vec<u8> = vec![first];
    for _ in 0..extra {
        let rb = read_byte_eof();
        if rb[1] == 1 {
            return None; // EOF: incomplete code point, insert nothing
        }
        bytes.push(rb[0]);
    }
    match String::from_utf8(bytes) {
        Ok(s) => Some(s.chars().next().unwrap_or(' ')),
        _ => Some(' '),
    }
}

// --- minibuffer completion helpers ------------------------------------------

pub fn chars_of(s: String) -> Vec<char> {
    let mut out: Vec<char> = vec![];
    for c in s.chars() {
        out.push(c);
    }
    out
}

pub fn str_of(cs: Vec<char>) -> String {
    cs.iter().collect::<String>()
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
    true
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
    hits
}

// Longest common prefix of all candidates.
pub fn common_prefix(list: Vec<String>) -> String {
    if list.is_empty() {
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
    str_of(out)
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
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if let Some(fp) = e.path().to_str() {
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
        }
    }
    cands
}

// Echo width of a candidate list: the two separator spaces plus every
// candidate followed by one space (the format echoed by Tab).
pub fn list_width(list: Vec<String>) -> usize {
    let mut w = 0;
    if !list.is_empty() {
        w = 2;
        let mut j = 0;
        while j < list.len() {
            w += chars_of(list[j].clone()).len() + 1;
            j += 1;
        }
    }
    w
}

// The candidate whose echoed span covers the cursor position cur in the
// echo area (input of inp chars, then "  cand1 cand2 ... "). Spans are
// inclusive caret ranges: the caret right before a candidate's first char
// and right after its last char both count as on it; the separator spaces
// between candidates are not on any candidate. "" when on none.
pub fn cand_at(list: Vec<String>, inp: usize, cur: usize) -> String {
    let mut s = inp + 2;
    let mut j = 0;
    while j < list.len() {
        let l = chars_of(list[j].clone()).len();
        if cur >= s && cur <= s + l {
            return list[j].clone();
        }
        s += l + 1;
        j += 1;
    }
    "".to_string()
}

// Park the real terminal cursor at echo-column cur of the minibuffer row
// (prompt width plen chars, 1-based screen row row).
fn echo_goto(row: usize, plen: usize, cur: usize) {
    print!("\x1b[{};{}H", row, plen + cur + 1);
}

// Erase from echo-column cur to the end of the row, then echo the input
// tail out[cur..]; leaves the real cursor after the printed tail.
fn echo_tail(row: usize, plen: usize, cur: usize, out: &[char]) {
    print!("\x1b[{};{}H\x1b[K", row, plen + cur + 1);
    let mut k = cur;
    while k < out.len() {
        print!("{}", out[k]);
        k += 1;
    }
}

// Read a line in raw mode with local echo on the bottom (status) row, with
// Tab completion against cands (or the filesystem when file_comp is set).
// On a unique match the suffix is echoed; multiple matches echo their
// common prefix and list the candidates after the input. The cursor can
// move over the echoed line (arrows / C-b / C-f / C-a / C-e): Enter on a
// listed candidate selects it, Enter elsewhere accepts the typed input,
// and editing the input clears a stale candidate list. Returns the
// completed string, or None on EOF (stdin closed): the command cancels,
// and the main loop's own EOF read ends the session. The ESC-branch inner
// reads intentionally stay read_byte(): an EOF byte (0) never matches
// '[', and the loop-top read_byte_eof returns None, so no hang is possible.
pub fn read_line_c(prompt: String, cands: Vec<String>, file_comp: bool) -> Option<String> {
    let sz = curses::canvas::get_size();
    let row: usize = if sz.rows > 0 { sz.rows as usize } else { 24 };
    let plen = chars_of(prompt.clone()).len();
    print!("\x1b[{};1H\x1b[2K", row);
    print!("{}", prompt);
    flush_stdout();
    let mut out: Vec<char> = vec![];    // the typed input
    let mut list: Vec<String> = vec![]; // candidates echoed after the input
    let mut cur: usize = 0;             // cursor in the echo area, 0..=width
    loop {
        let tl = out.len() + list_width(list.clone()); // echoed width
        let rb = read_byte_eof();
        if rb[1] == 1 {
            return None; // EOF: cancel this prompt
        }
        let b = rb[0];
        if b == 0x0D || b == 0x0A { // Enter: the candidate under the cursor wins
            let mut ret = str_of(out.clone());
            if !list.is_empty() {
                let sel = cand_at(list.clone(), out.len(), cur);
                if !sel.is_empty() {
                    ret = sel;
                }
            }
            echo_goto(row, plen, tl);
            print!("\r\n");
            flush_stdout();
            return Some(ret);
        }
        if b == 0x07 { // Ctrl-g: cancel
            echo_goto(row, plen, tl);
            print!(" [Cancelled]\r\n");
            flush_stdout();
            return Some("".to_string());
        }
        if b == 0x09 { // Tab: minibuffer completion
            let inp = str_of(out.clone());
            let pool = if file_comp { file_candidates(inp.clone()) } else { cands.clone() };
            let hits = completions(inp.clone(), pool);
            if hits.len() == 1 {
                cur = out.len();
                if !list.is_empty() { // input changes: the old list echo is stale
                    list = vec![];
                    print!("\x1b[{};{}H\x1b[K", row, plen + cur + 1);
                }
                let full = chars_of(hits[0].clone());
                let mut k = chars_of(inp.clone()).len();
                while k < full.len() {
                    out.push(full[k]);
                    print!("{}", full[k]);
                    k += 1;
                }
                cur = out.len();
                flush_stdout();
            } else if hits.len() > 1 {
                cur = out.len();
                if !list.is_empty() { // relisting: erase the old list echo
                    print!("\x1b[{};{}H\x1b[K", row, plen + cur + 1);
                }
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
                list = hits.clone();
                cur = out.len() + list_width(list.clone());
                flush_stdout();
            }
            continue;
        }
        if b == 0x02 { // Ctrl-b: one echo char left
            if cur > 0 {
                cur -= 1;
                echo_goto(row, plen, cur);
                flush_stdout();
            }
            continue;
        }
        if b == 0x06 { // Ctrl-f: one echo char right
            if cur < tl {
                cur += 1;
                echo_goto(row, plen, cur);
                flush_stdout();
            }
            continue;
        }
        if b == 0x01 { // Ctrl-a: echo line start
            cur = 0;
            echo_goto(row, plen, cur);
            flush_stdout();
            continue;
        }
        if b == 0x05 { // Ctrl-e: echo line end
            cur = tl;
            echo_goto(row, plen, cur);
            flush_stdout();
            continue;
        }
        if b == 0x1b { // ANSI arrow keys: ESC [ A/B/C/D (A/B unused here)
            let n = read_byte();
            if n == 0x5b {
                let t = read_byte();
                if t == 0x44 { // Left: one echo char left
                    if cur > 0 {
                        cur -= 1;
                        echo_goto(row, plen, cur);
                        flush_stdout();
                    }
                } else if t == 0x43 { // Right: one echo char right
                    if cur < tl {
                        cur += 1;
                        echo_goto(row, plen, cur);
                        flush_stdout();
                    }
                }
            }
            continue;
        }
        if b == 0x7f { // backspace: delete the char left of the cursor
            if cur > out.len() {
                cur = out.len(); // the candidate list echo is not editable
            }
            if cur > 0 {
                out.remove(cur - 1);
                cur -= 1;
                list = vec![]; // input edited: the listed candidates are stale
                echo_tail(row, plen, cur, &out);
                echo_goto(row, plen, cur);
                flush_stdout();
            }
            continue;
        }
        // printable char: insert at the cursor, clamped to the input end
        // (the candidate list echo is not editable); the echo tail shifts.
        if cur > out.len() {
            cur = out.len();
        }
        let c = read_char_eof(b)?; // EOF mid-codepoint: cancel this prompt
        out.insert(cur, c);
        cur += 1;
        if !list.is_empty() {
            list = vec![]; // input edited: the listed candidates are stale
        }
        echo_tail(row, plen, cur - 1, &out);
        echo_goto(row, plen, cur);
        flush_stdout();
    }
}

// Read a line of input in raw mode with local echo, printed on the bottom
// (status) row so it replaces the status line like Emacs minibuffer prompts.
// EOF collapses to the empty string (= cancel), same as Ctrl-g.
pub fn read_line(prompt: String) -> String {
    read_line_c(prompt, vec![], false).unwrap_or("".to_string())
}

// Minibuffer line with Tab completion over given candidates (buffer names).
pub fn read_line_buffers(prompt: String, names: Vec<String>) -> String {
    read_line_c(prompt, names, false).unwrap_or("".to_string())
}

// Minibuffer line with Tab completion over the filesystem.
pub fn read_line_files(prompt: String) -> String {
    read_line_c(prompt, vec![], true).unwrap_or("".to_string())
}

// Minibuffer line exposing the EOF distinction (None): for call sites where
// "stdin closed" must behave differently from "cancelled" (exit confirm).
pub fn read_line_eof(prompt: String, cands: Vec<String>, file_comp: bool) -> Option<String> {
    read_line_c(prompt, cands, file_comp)
}

#[cfg(test)]
mod tests {
use super::*;
#[test]
fn echo_list_width() {
    assert!(list_width(vec![]) == 0);
    // "  aa ab": 2 separators + "aa " + "ab "
    assert!(list_width(vec!["aa".to_string(), "ab".to_string()]) == 8);
    assert!(list_width(vec!["*demo*".to_string()]) == 9);
}
#[test]
fn candidate_under_cursor() {
    // echo: [aa ab], input "a" at 0, list starts at 1+2=3: aa spans 3..5, ab 6..8
    let l = vec!["aa".to_string(), "ab".to_string()];
    assert!(cand_at(l.clone(), 1, 3) == "aa"); // caret before "aa"
    assert!(cand_at(l.clone(), 1, 4) == "aa"); // caret inside "aa"
    assert!(cand_at(l.clone(), 1, 5) == "aa"); // caret after "aa"
    assert!(cand_at(l.clone(), 1, 6) == "ab"); // caret before "ab"
    assert!(cand_at(l.clone(), 1, 8) == "ab"); // caret after "ab"
    assert!(cand_at(l.clone(), 1, 1) == "");   // caret in the input
    assert!(cand_at(l.clone(), 1, 2) == "");   // caret in the separator gap
    assert!(cand_at(l.clone(), 1, 9) == "");   // caret past the trailing space
    assert!(cand_at(vec![], 0, 0) == "");
}
}
