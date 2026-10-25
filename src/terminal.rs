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

// T-5: fd 0 is hard-wired. When stdin is NOT a tty (pipe/redirect)
// tcgetattr fails and NOTHING is configured: the guard carries saved =
// None and restores nothing. The design gives up raw mode gracefully
// there - tmux e2e drives emur through a pty, and a keyboard-less stdin
// has no control codes to deliver raw anyway.
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
            // T-9: read() must return as soon as ONE byte arrives
            // (blocking read with no timer). ICANON off alone leaves the
            // inherited c_cc values in play; set them explicitly.
            t.c_cc[VMIN] = 1;
            t.c_cc[VTIME] = 0;
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
// candidate followed by one space (the format echoed by Tab), in DISPLAY
// CELLS (Design Doc C / T-7: fullwidth chars occupy 2 terminal columns).
pub fn list_width(list: Vec<String>) -> usize {
    let mut w = 0;
    if !list.is_empty() {
        w = 2;
        let mut j = 0;
        while j < list.len() {
            w += cells_of(&chars_of(list[j].clone())) + 1;
            j += 1;
        }
    }
    w
}

// Sum of display cells for a char slice (Design Doc C). char_width is the
// single width source (screen.rs); echo math goes through here.
pub fn cells_of(s: &[char]) -> usize {
    let mut w = 0;
    let mut k = 0;
    while k < s.len() {
        w += super::screen::char_width(s[k]);
        k += 1;
    }
    w
}

// The candidate whose echoed span covers the cursor position cur in the
// echo area (input of inp CELLS, then "  cand1 cand2 ... ", caret_cells
// CELLS from the prompt start). Spans are inclusive caret ranges in CELLS:
// the caret right before a candidate's first char and right after its last
// char both count as on it; the separator spaces between candidates are
// not on any candidate. "" when on none.
pub fn cand_at(list: Vec<String>, inp: usize, cur: usize) -> String {
    let mut s = inp + 2;
    let mut j = 0;
    while j < list.len() {
        let l = cells_of(&chars_of(list[j].clone()));
        if cur >= s && cur <= s + l {
            return list[j].clone();
        }
        s += l + 1;
        j += 1;
    }
    "".to_string()
}

// Park the real terminal cursor at echo-column cur of the minibuffer row
// (base column offset plen CELLS, 1-based screen row row).
fn echo_goto(row: usize, plen: usize, cur: usize) {
    print!("\x1b[{};{}H", row, plen + cur + 1);
}

// Erase from the caret's CELL column (col, screen 1-based via col+1) to the
// end of the row, then echo the input tail out[cur..]; leaves the real
// cursor after the printed tail. cur is a CHAR index into out (Design Doc
// C / Major-1 gate fix: col comes in as a cell column computed by the
// caller's caret_col - NO cell-minus-char subtraction inside: a combining
// mark (width 0) makes cells_of(out[..cur]) < cur and the old alias
// (plen_c + cells) - cur underflowed).
fn echo_tail(row: usize, col: usize, cur: usize, out: &[char]) {
    print!("\x1b[{};{}H\x1b[K", row, col + 1);
    let mut k = cur;
    while k < out.len() {
        print!("{}", out[k]);
        k += 1;
    }
}

// Caret column in CELLS, prompt-relative (Design Doc C / T-7). cur is the
// read_line_c caret coordinate, dual-unit as before: cur <= out.len() is a
// CHAR boundary in the input (cells = width of out[..cur]); cur > out.len()
// sits (cur - out.len()) CELLS into the echoed candidate list (list_width
// is cell-based, so list-domain units are already cells). This function is
// the single conversion point from caret coords to screen columns.
fn caret_col(plen_c: usize, out: &[char], cur: usize) -> usize {
    if cur <= out.len() {
        plen_c + cells_of(&out[..cur])
    } else {
        plen_c + cells_of(out) + (cur - out.len())
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
    let sz = crate::screen::term_size();
    let row: usize = if sz.rows > 0 { sz.rows as usize } else { 24 };
    let plen_c = cells_of(&chars_of(prompt.clone())); // prompt width in CELLS
    print!("\x1b[{};1H\x1b[2K", row);
    print!("{}", prompt);
    flush_stdout();
    let mut out: Vec<char> = vec![];    // the typed input
    let mut list: Vec<String> = vec![]; // candidates echoed after the input
    let mut cur: usize = 0;             // caret coord, dual-unit (see caret_col)
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
                let sel = cand_at(list.clone(), cells_of(&out), caret_col(0, &out, cur));
                if !sel.is_empty() {
                    ret = sel;
                }
            }
            echo_goto(row, 0, caret_col(plen_c, &out, tl));
            print!("\r\n");
            flush_stdout();
            return Some(ret);
        }
        if b == 0x07 { // Ctrl-g: cancel
            echo_goto(row, 0, caret_col(plen_c, &out, tl));
            print!(" [Cancelled]\r\n");
            flush_stdout();
            return Some("".to_string());
        }
        if b == 0x09 { // Tab: minibuffer completion
            let inp = str_of(out.clone());
            let pool = if file_comp { file_candidates(inp.clone()) } else { cands.clone() };
            let hits = completions(inp.clone(), pool);
            if hits.is_empty() {
                // T-6: no completion matches -> emacs rings the bell
                // (the old branch fell through SILENTLY: no echo, no
                // feedback). \x07 is BEL; terminals render it audibly
                // (or visibly, per bell settings) and tmux passes it
                // through (last-line "Bell" flag).
                print!("\x07");
                flush_stdout();
            } else if hits.len() == 1 {
                cur = out.len();
                if !list.is_empty() { // input changes: the old list echo is stale
                    list = vec![];
                    print!("\x1b[{};{}H\x1b[K", row, caret_col(plen_c, &out, cur) + 1);
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
                    print!("\x1b[{};{}H\x1b[K", row, caret_col(plen_c, &out, cur) + 1);
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
                echo_goto(row, 0, caret_col(plen_c, &out, cur));
                flush_stdout();
            }
            continue;
        }
        if b == 0x06 { // Ctrl-f: one echo char right
            if cur < tl {
                cur += 1;
                echo_goto(row, 0, caret_col(plen_c, &out, cur));
                flush_stdout();
            }
            continue;
        }
        if b == 0x01 { // Ctrl-a: echo line start
            cur = 0;
            echo_goto(row, 0, caret_col(plen_c, &out, cur));
            flush_stdout();
            continue;
        }
        if b == 0x05 { // Ctrl-e: echo line end
            cur = tl;
            echo_goto(row, 0, caret_col(plen_c, &out, cur));
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
                        echo_goto(row, 0, caret_col(plen_c, &out, cur));
                        flush_stdout();
                    }
                } else if t == 0x43 { // Right: one echo char right
                    if cur < tl {
                        cur += 1;
                        echo_goto(row, 0, caret_col(plen_c, &out, cur));
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
                echo_tail(row, caret_col(plen_c, &out, cur), cur, &out);
                echo_goto(row, 0, caret_col(plen_c, &out, cur));
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
        echo_tail(row, caret_col(plen_c, &out, cur - 1), cur - 1, &out);
        echo_goto(row, 0, caret_col(plen_c, &out, cur));
        flush_stdout();
    }
}

// Read a line of input in raw mode with local echo, printed on the bottom
// (status) row so it replaces the status line like Emacs minibuffer prompts.
// EOF collapses to the empty string (= cancel), same as Ctrl-g.
// B-10: prompt accepts &str / String (the core read_line_c keeps String;
// the wrappers convert), so call sites drop the .to_string() ceremony.
pub fn read_line(prompt: impl Into<String>) -> String {
    read_line_c(prompt.into(), vec![], false).unwrap_or("".to_string())
}

// Minibuffer line with Tab completion over given candidates (buffer names).
pub fn read_line_buffers(prompt: impl Into<String>, names: Vec<String>) -> String {
    read_line_c(prompt.into(), names, false).unwrap_or("".to_string())
}

// Minibuffer line with Tab completion over the filesystem.
pub fn read_line_files(prompt: impl Into<String>) -> String {
    read_line_c(prompt.into(), vec![], true).unwrap_or("".to_string())
}

// Minibuffer line exposing the EOF distinction (None): for call sites where
// "stdin closed" must behave differently from "cancelled" (exit confirm).
pub fn read_line_eof(prompt: impl Into<String>, cands: Vec<String>, file_comp: bool) -> Option<String> {
    read_line_c(prompt.into(), cands, file_comp)
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
    assert!(cand_at(l.clone(), 1, 1).is_empty()); // caret in the input
    assert!(cand_at(l.clone(), 1, 2).is_empty()); // caret in the separator gap
    assert!(cand_at(l.clone(), 1, 9).is_empty()); // caret past the trailing space
    assert!(cand_at(vec![], 0, 0).is_empty());
}
#[test]
fn cells_of_cjk() {
    let s: Vec<char> = "abあ".chars().collect();
    assert!(cells_of(&s) == 4); // 1 + 1 + 2
    assert!(cells_of(&[]) == 0);
    let t: Vec<char> = "a\u{0301}b".chars().collect();
    assert!(cells_of(&t) == 2); // combining mark adds 0
}
#[test]
fn list_width_cjk() {
    assert!(list_width(vec!["あい".to_string()]) == 7); // 2 sep + 4 cells + 1
    assert!(list_width(vec!["aa".to_string(), "ab".to_string()]) == 8); // ASCII unchanged
}
#[test]
fn cand_at_cjk_cells() {
    // Design Doc C / T-7: spans are built from CELLS, so the cell-end of a
    // fullwidth candidate (cell 12) hits while the char-based span (end 11)
    // would miss it.
    let l = vec!["あa.txt".to_string(), "あi.txt".to_string()]; // 7 cells each
    assert!(cand_at(l.clone(), 3, 5) == "あa.txt"); // caret at cand1 start
    assert!(cand_at(l.clone(), 3, 12) == "あa.txt"); // cand1 cell-end (chars: miss)
    assert!(cand_at(l.clone(), 3, 13) == "あi.txt"); // cand2 start
    assert!(cand_at(l.clone(), 3, 4).is_empty()); // separator gap
}
#[test]
fn caret_col_units() {
    let out: Vec<char> = "/xあ".chars().collect(); // 3 chars, 4 cells
    assert!(caret_col(11, &out, 0) == 11); // prompt start
    assert!(caret_col(11, &out, 2) == 13); // after "/x"
    assert!(caret_col(11, &out, 3) == 15); // after the fullwidth char
    assert!(caret_col(11, &out, 4) == 16); // list domain: +1 CELL offset
    // Major-1 (gate): combining marks make cells < chars. caret_col only
    // ADDED widths (no cell-minus-char subtraction), so echo_tail callers
    // are underflow-proof: this used to panic in debug builds.
    let m: Vec<char> = "a\u{0301}\u{0301}\u{0301}".chars().collect(); // 4 chars, 1 cell
    assert!(caret_col(0, &m, 4) == 1);
    assert!(caret_col(0, &m, 1) == 1); // caret after base, before marks
}
}
