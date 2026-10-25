mod terminal;
mod buffer;
mod screen;
mod window;
mod highlight;

// Returns the file contents, or Err(reason) when the file cannot be read
// (M-9: callers must report this - loading silently as empty lost errors).
fn read_file(path: String) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) => Err(format!("{}", e)),
    }
}

fn write_file(path: String, text: String) -> bool {
    std::fs::write(path, text.as_bytes()).is_ok()
}

fn find_buffer(bufs: &[buffer::Buffer], name: String) -> Vec<usize> {
    let mut j = 0;
    while j < bufs.len() {
        if bufs[j].name == name {
            return vec![j];
        }
        j += 1;
    }
    vec![]
}

// Show buffer nb in the active window: a lone window splits below and the
// buffer shows in the NEW window (the old one keeps its buffer); in a split
// tree the active window's buffer is replaced. point/mark/scroll reset to 0.
// Shared by the fresh-load and the reuse path of C-x C-f (M-6).
fn show_buffer(tree: &mut window::Tree, active: &mut usize, nb: usize) {
    if tree.leaves.len() == 1 {
        tree.split(*active, true);
        *active = tree.leaves.len() - 1;
        attach_leaf(tree, *active, nb);
    } else {
        attach_leaf(tree, *active, nb);
    }
}

// M-13: write the live buffer positions back into the ACTIVE leaf's
// per-leaf display cache (each leaf caches its own point/mark: split
// leaves aliasing one buffer may sit at different positions; C-x k
// resets ALL aliases - M-15). Used by C-x o (before cycling) and the
// loop-bottom sync of ordinary commands.
fn sync_leaf(tree: &mut window::Tree, bufs: &[buffer::Buffer], active: usize, bi: usize) {
    tree.leaves[active].point = bufs[bi].point;
    tree.leaves[active].mark = bufs[bi].mark;
}

// M-13: attach buffer nb to leaf `li` with fresh per-leaf positions.
// Used by the C-x b switch paths and both show_buffer branches.
fn attach_leaf(tree: &mut window::Tree, li: usize, nb: usize) {
    tree.leaves[li].buffer = nb;
    tree.leaves[li].point = 0;
    tree.leaves[li].mark = 0;
    tree.leaves[li].scroll = 0;
}

// Quit confirmation (Emacs: "Modified buffers exist; exit anyway? (yes|no)"):
// exiting via Ctrl-c / Ctrl-x Ctrl-c asks in the minibuffer when any buffer
// has unsaved modifications. Only "yes" (or "y") exits; anything else cancels.
// EOF (stdin closed) cannot be answered, so it exits: unsaved changes are
// lost (documented in README, Design Doc A / M-4 policy).
fn confirm_exit(bufs: &[buffer::Buffer]) -> bool {
    let mut dirty = false;
    let mut j = 0;
    while j < bufs.len() {
        if bufs[j].modified { dirty = true; }
        j += 1;
    }
    if !dirty {
        return true;
    }
    let sel = terminal::read_line_eof("Modified buffers exist; exit anyway? (yes|no) ", vec![], false);
    match sel {
        None => true, // EOF: no input possible, exit with data loss
        Some(s) => s == "yes" || s == "y",
    }
}

fn main() {
    // Visible panic report; the actual tty restore happens through TtyGuard's
    // Drop (unwinding runs drops), so no panic can leave the shell in raw mode.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("emur panicked - restoring terminal: {}", info);
    }));
    let _guard = terminal::save_raw_mode(); // bound by name: Drop at end of main
    terminal::install_restore_signals();
    let args: Vec<String> = std::env::args().collect();
    let mut msg = "".to_string();
    let mut bufs = vec![buffer::Buffer::new("*demo*", "This is the first line\nof the demo buffer.\n")];
    if args.len() > 1 {
        let path = args[1].to_string();
        match read_file(path.to_string()) {
            Ok(text) => {
                bufs[0] = buffer::Buffer::new(path.clone(), text);
                bufs[0].filename = Some(path);
            }
            Err(e) => {
                // M-9: report the error; the buffer stays EMPTY with NO
                // filename, so C-x C-s cannot clobber the unreadable file.
                bufs[0] = buffer::Buffer::new(path.clone(), "");
                msg = format!("Failed to open {}: {}", path, e);
            }
        }
    }
    let mut tree = window::new_tree(0);
    let mut active = 0;
    let mut killed = "".to_string();
    let mut prev_cmd = 0;
    loop {
        tree = screen::draw_all(&bufs, tree, active, msg.to_string());
        let bi = tree.leaves[active].buffer;
        // M-15: a leaf's point/mark go STALE when its buffer is replaced in
        // place (C-x k while several leaves alias one buffer). Clamp before
        // copying into the buffer: insert/kill at point > len panics in
        // move_gap (index out of bounds).
        let lp = if tree.leaves[active].point > bufs[bi].len() { bufs[bi].len() } else { tree.leaves[active].point };
        let lm = if tree.leaves[active].mark > bufs[bi].len() { bufs[bi].len() } else { tree.leaves[active].mark };
        tree.leaves[active].point = lp;
        tree.leaves[active].mark = lm;
        bufs[bi].point = lp;
        bufs[bi].mark = lm;
        let rb = terminal::read_byte_eof();
        if rb[1] == 1 {
            break; // EOF (no byte read at all)
        }
        let b = rb[0];
        if b == 0x03 { // Ctrl-c: quit (asks first when buffers are dirty)
            if confirm_exit(&bufs) {
                break;
            }
            msg = "Cancelled".to_string();
        }
        let mut moved = false;
        if b == 0x18 {
            let n = terminal::read_byte();
            if n == 0x03 { // Ctrl-x Ctrl-c: quit (asks first when buffers are dirty)
                if confirm_exit(&bufs) {
                    break;
                }
                msg = "Cancelled".to_string();
            } else if n == 0x31 { // Ctrl-x 1: delete-other-windows
                active = tree.collapse(active); // S-3 compaction remaps the leaf index
                msg = "".to_string();
            } else if n == 0x32 { // Ctrl-x 2: split-window-below
                tree.split(active, true);
                msg = "".to_string();
            } else if n == 0x33 { // Ctrl-x 3: split-window-right
                tree.split(active, false);
                msg = "".to_string();
            } else if n == 0x6f { // Ctrl-x o: other-window
                sync_leaf(&mut tree, &bufs, active, bi);
                let order = tree.flatten();
                let mut j = 0;
                while j < order.len() {
                    if order[j] == active {
                        if j + 1 < order.len() {
                            active = order[j + 1];
                        } else {
                            active = order[0];
                        }
                        break;
                    }
                    j += 1;
                }
                moved = true;
                msg = "".to_string();
            } else if n == 0x06 { // Ctrl-x Ctrl-f: open file; in a lone window split below, in a split tree load into the active window
                let path = terminal::read_line_files("Find file: ");
                if !path.is_empty() {
                    // M-6: an already-open file REUSES its buffer (Emacs
                    // find-file). Matching is by buffer name (= path at load,
                    // the same basis as find_buffer/C-x b); a C-x C-w-bound
                    // buffer keeps its own name and is not matched. No disk
                    // re-read: buffer edits and the modified flag survive.
                    let existing = find_buffer(&bufs, path.clone());
                    if let Some(&nb) = existing.first() {
                        if tree.leaves[active].buffer == nb {
                            msg = "".to_string(); // already showing it: no-op
                        } else {
                            show_buffer(&mut tree, &mut active, nb);
                            moved = true;
                            msg = "".to_string();
                        }
                    } else {
                        match read_file(path.to_string()) {
                            Err(e) => {
                                // M-9: an unreadable file does NOT become an empty
                                // loaded buffer; report it, keep the current window.
                                msg = format!("Failed to open {}: {}", path, e);
                            }
                            Ok(text) => {
                                let nb = bufs.len();
                                bufs.push(buffer::Buffer::new(path.clone(), text));
                                bufs[nb].filename = Some(path.clone());
                                bufs[nb].modified = false;
                                show_buffer(&mut tree, &mut active, nb);
                                moved = true;
                                msg = "".to_string();
                            }
                        }
                    }
                }
            } else if n == 0x13 { // Ctrl-x Ctrl-s: save
                let path = bufs[bi].filename.clone().map(|p| p.to_string()).unwrap_or("".to_string());
                if !path.is_empty() {
                    if write_file(path.to_string(), bufs[bi].text()) {
                        bufs[bi].modified = false;
                        msg = format!("Wrote {}", path);
                    } else {
                        msg = format!("Failed to write {}", path);
                    }
                } else {
                    msg = "No file name for this buffer".to_string();
                }
            } else if n == 0x17 { // Ctrl-x Ctrl-w: write-file
                let path = terminal::read_line_files("Write file: ");
                if !path.is_empty() {
                    // M-8: the filename binds ONLY on success; a failed
                    // write leaves save-target (and C-x C-s) untouched.
                    if write_file(path.to_string(), bufs[bi].text()) {
                        bufs[bi].filename = Some(path.clone());
                        bufs[bi].modified = false;
                        msg = format!("Wrote {}", path);
                    } else {
                        msg = format!("Failed to write {}", path);
                    }
                }
            } else if n == 0x09 { // Ctrl-x Ctrl-i: insert-file
                let path = terminal::read_line_files("Insert file: ");
                if !path.is_empty() {
                    match read_file(path.to_string()) {
                        Ok(text) => {
                            bufs[bi].insert_string(&text);
                            msg = format!("Inserted file {}", path);
                        }
                        Err(e) => {
                            // M-9: report the failure; insert nothing
                            msg = format!("Failed to open {}: {}", path, e);
                        }
                    }
                }
            } else if n == 0x6b { // Ctrl-x k: kill-buffer (asks first when modified, M-5)
                let name = bufs[bi].name.clone();
                let proceed = if !bufs[bi].modified {
                    true
                } else {
                    // EOF (stdin closed) reads as "" = Cancelled; the main
                    // loop's own EOF read then exits (M-4 policy: unconfirmed
                    // exit loses data, which is documented).
                    let sel = terminal::read_line(format!("Kill buffer {}? (yes|no) ", name));
                    sel == "yes" || sel == "y"
                };
                if !proceed {
                    msg = "Cancelled".to_string();
                } else {
                    bufs[bi] = buffer::Buffer::new(name.clone(), "");
                    bufs[bi].filename = None;
                    bufs[bi].modified = false;
                    // M-15: leaves ALIASING this buffer (C-x 2/C-x 3/C-x C-f
                    // reuse keep point/mark/scroll per leaf) keep positions
                    // from the killed content: reset EVERY leaf showing bi,
                    // not just the active one.
                    let mut lk = 0;
                    while lk < tree.leaves.len() {
                        if tree.leaves[lk].buffer == bi {
                            tree.leaves[lk].point = 0;
                            tree.leaves[lk].mark = 0;
                            tree.leaves[lk].scroll = 0;
                        }
                        lk += 1;
                    }
                    msg = format!("Killed buffer {}", name);
                }
            } else if n == 0x62 || n == 0x02 { // Ctrl-x b / Ctrl-x Ctrl-b: switch-to-buffer & buffer list select
                let mut names = "".to_string();
                let mut bnames: Vec<String> = vec![];
                let mut j = 0;
                while j < bufs.len() {
                    bnames.push(bufs[j].name.clone());
                    if j > 0 { names = format!("{} {}", names, bufs[j].name); } else { names = bufs[j].name.clone(); }
                    j += 1;
                }
                let prompt = if n == 0x02 { format!("Buffers: {} Enter=next, name: ", names) } else { "Switch to buffer: ".to_string() };
                let sel = terminal::read_line_buffers(prompt, bnames);
                let mut ti = 999999;
                if !sel.is_empty() {
                    let hit = find_buffer(&bufs, sel.to_string());
                    if !hit.is_empty() { ti = hit[0]; }
                } else if n == 0x02 { // Ctrl-x Ctrl-b + Enter: cycle to the next buffer
                    let nxt = if bi + 1 < bufs.len() { bi + 1 } else { 0 };
                    ti = nxt;
                }
                if ti == 999999 && sel.is_empty() {
                    msg = "Cancelled".to_string();
                } else if ti < bufs.len() {
                    attach_leaf(&mut tree, active, ti);
                    moved = true;
                    msg = "".to_string();
                } else {
                    bufs.push(buffer::Buffer::new(sel, ""));
                    attach_leaf(&mut tree, active, bufs.len() - 1);
                    moved = true;
                    msg = "".to_string();
                }
            } else {
                // M-7: an unbound C-x sequence reports itself explicitly
                // (was silent: the previous status line just stayed).
                msg = format!("Undefined command: C-x {}", n as char);
            }
        } else if b == 0x13 { // Ctrl-s: incremental forward search
            // Type the query one char at a time; each char extends the search
            // (wrapping at the buffer end) and moves point to the match end.
            // A char that matches nowhere rings the bell and is ignored.
            // C-s while searching repeats the current query. Enter accepts the
            // current match; Ctrl-g cancels (point restored); ESC also ends.
            let orig = bufs[bi].point;
            let mut qs: Vec<char> = vec![];
            let mut last = orig; // start position of the current match
            let szs = screen::term_size();
            let prow = if szs.rows > 0 { szs.rows } else { 24 };
            print!("\x1b[{};1H\x1b[2K", prow);
            print!("Search: ");
            terminal::flush_stdout();
            let mut cancelled = false;
            loop {
                let rb2 = terminal::read_byte_eof();
                if rb2[1] == 1 { break; } // EOF: keep the current match
                let c2 = rb2[0];
                if c2 == 0x0D || c2 == 0x0A { break; } // accept
                if c2 == 0x1b {
                    // M-3: ESC accepts the search. Arrow keys arrive as
                    // ESC [ <ABCD>; swallow the whole sequence here, else
                    // the trailing "[B" etc. self-insert into the buffer.
                    // The arrow then acts as a plain arrow key (Emacs
                    // isearch: arrows accept and move).
                    let n2 = terminal::read_byte();
                    if n2 == 0x5b {
                        let t = terminal::read_byte();
                        if t == 0x41 {
                            bufs[bi].previous_line(); // Up
                        } else if t == 0x42 {
                            bufs[bi].next_line(); // Down
                        } else if t == 0x43 {
                            bufs[bi].forward_char(); // Right
                        } else if t == 0x44 {
                            bufs[bi].backward_char(); // Left
                        }
                    }
                    break;
                }
                if c2 == 0x07 { cancelled = true; break; } // Ctrl-g: cancel
                if c2 == 0x7f { // backspace: drop the last query char
                    if !qs.is_empty() { qs.pop(); }
                    if !qs.is_empty() {
                        let q2 = terminal::str_of(qs.clone());
                        let hit = bufs[bi].find_forward(&q2, 0);
                        if let Some(end) = hit {
                            bufs[bi].point = end;
                            last = end - qs.len();
                        }
                    }
                    continue;
                }
                let mut attempt: Vec<char> = vec![]; // query to search for
                let from = last + 1;
                let mut echo_ch = ' '.to_string(); // char to echo on the search line
                if c2 == 0x13 { // Ctrl-s while searching: repeat the query
                    if qs.is_empty() { continue; } // still waiting for chars
                    attempt = qs.clone();
                } else { // a printable char extends the query
                    let cc = match terminal::read_char_eof(c2) {
                        Some(c) => c,
                        None => break, // EOF: keep the current match
                    };
                    attempt = qs.clone();
                    attempt.push(cc);
                    echo_ch = cc.to_string();
                }
                let q2 = terminal::str_of(attempt.clone());
                let hit = bufs[bi]
                    .find_forward(&q2, from)
                    .or_else(|| bufs[bi].find_forward(&q2, 0)); // wrap once
                if let Some(end) = hit {
                    qs = attempt;
                    bufs[bi].point = end;
                    last = end - qs.len();
                    if c2 != 0x13 {
                        print!("{}", echo_ch); // echo the query char
                    }
                    terminal::flush_stdout();
                } else {
                    print!("\x07"); // bell: char ignored
                    terminal::flush_stdout();
                }
            }
            if cancelled {
                bufs[bi].point = orig;
                msg = "Cancelled".to_string();
            } else {
                msg = "".to_string();
            }
        } else if b == 0x12 { // Ctrl-r: reverse search
            let q = terminal::read_line("Reverse find: ");
            if !q.is_empty() {
                let hit = bufs[bi].find_backward(&q, bufs[bi].point);
                if let Some(start) = hit {
                    bufs[bi].point = start;
                    msg = "".to_string();
                } else {
                    msg = format!("Search string not found: {}", q);
                }
            } else {
                msg = "Cancelled".to_string();
            }
        } else if b == 0x07 { // Ctrl-g: cancel
            msg = "Cancelled".to_string();
        } else if b == 0x1b {
            let n = terminal::read_byte(); // Meta prefix
            if n == 0x5b { // ANSI arrow keys: ESC [ A/B/C/D
                let t = terminal::read_byte();
                if t == 0x41 {
                    bufs[bi].previous_line(); // Up
                } else if t == 0x42 {
                    bufs[bi].next_line(); // Down
                } else if t == 0x43 {
                    bufs[bi].forward_char(); // Right
                } else if t == 0x44 {
                    bufs[bi].backward_char(); // Left
                }
            } else if n == 0x62 {
                bufs[bi].backward_word();
            } else if n == 0x66 {
                bufs[bi].forward_word();
            } else if n == 0x3c {
                bufs[bi].beginning_of_buffer(); // C-</C-> arrive as Meta form over raw ttys
            } else if n == 0x3e {
                bufs[bi].end_of_buffer();
            }
        } else if b == 0x00 { // Ctrl-space: set the mark (region start)
            bufs[bi].mark = bufs[bi].point;
            msg = "".to_string();
        } else if b == 0x01 {
            bufs[bi].beginning_of_line(); // Ctrl-a
        } else if b == 0x02 {
            bufs[bi].backward_char(); // Ctrl-b
        } else if b == 0x06 {
            bufs[bi].forward_char(); // Ctrl-f
        } else if b == 0x0e {
            bufs[bi].next_line(); // Ctrl-n
        } else if b == 0x10 {
            bufs[bi].previous_line(); // Ctrl-p
        } else if b == 0x7f {
            bufs[bi].delete_backward(); // backspace (DEL)
        } else if b == 0x04 {
            bufs[bi].delete_forward(); // Ctrl-d
        } else if b == 0x0f {
            bufs[bi].insert_newline_after(); // Ctrl-o
        } else if b == 0x14 {
            bufs[bi].transpose_chars(); // Ctrl-t
        } else if b == 0x0b { // Ctrl-k: kill to end of line, feeds Ctrl-y
            let cut = bufs[bi].kill_to_end_of_line();
            if prev_cmd == 0x0b {
                killed = killed.to_string() + &cut; // consecutive kills accumulate
            } else {
                killed = cut;
            }
        } else if b == 0x19 { // Ctrl-y: paste the killed text at point
            bufs[bi].insert_string(&killed);
        } else if b == 0x05 {
            bufs[bi].end_of_line(); // Ctrl-e
        } else if b == 0x0c { // Ctrl-l: go-to-line (minibuffer reads a line number)
            let q = terminal::read_line("Go to line: ");
            if q.is_empty() {
                msg = "Cancelled".to_string();
            } else {
                // M-1: digits-only gate (parse alone accepts a leading '+'),
                // u64 parse (a 15-digit number is an Ok value, never a u32
                // overflow panic), and a total-line-count scan that bounds
                // the movement loop: huge numbers clamp to the last line
                // (documented behavior) and can no longer spin the loop.
                let digits_ok = q.bytes().all(|c| c.is_ascii_digit());
                let want = if digits_ok { q.parse::<u64>().ok() } else { None };
                match want {
                    None => msg = format!("Invalid line number: {}", q),
                    Some(0) => msg = format!("Invalid line number: {}", q),
                    Some(want_ln) => {
                        // M-10: ONE O(n) scan. tpos = start of line min(want_ln, total):
                        // each '\n' seen starts line `line`; only starts up to want_ln are
                        // recorded, so a want_ln past the end clamps to the last line (M-1).
                        // (Old code: a total-line count scan, then beginning_of_buffer +
                        // next_line per line = two full passes; the moves from position 0
                        // always land at column 0 = the line start, same landing point.)
                        let mut tpos: usize = 0; // start of line 1
                        let mut line: u64 = 1;
                        let mut i = 0;
                        while i < bufs[bi].len() {
                            if bufs[bi].char_at(i) == '\n' {
                                line += 1;
                                if line <= want_ln {
                                    tpos = i + 1;
                                } else {
                                    break; // tpos final: want_ln line start recorded
                                }
                            }
                            i += 1;
                        }
                        bufs[bi].point = tpos;
                        msg = "".to_string();
                    }
                }
            }
        } else if b == 0x11 { // Ctrl-q: quote next key
            // NB: at EOF the leading byte arrives as 0 (read_byte), which is
            // Ctrl-space's NUL - a literal NUL gets inserted once. Bounded:
            // the loop-top read_byte_eof then ends the session. (Design Doc A)
            let n = terminal::read_byte();
            if let Some(c) = terminal::read_char_eof(n) {
                bufs[bi].insert_char(c);
            } // EOF: insert nothing
        } else if b == 0x0a || b == 0x0d {
            bufs[bi].insert_char('\n'); // Ctrl-j / Ctrl-m: newline
        } else if (0x20..0x7f).contains(&b) || b >= 0x80 {
            // self-insert (ASCII, and UTF-8 lead bytes: read_char_eof consumes
            // the continuation bytes)
            if let Some(c) = terminal::read_char_eof(b) {
                bufs[bi].insert_char(c);
            } // EOF: insert nothing
        }
        if !moved {
            sync_leaf(&mut tree, &bufs, active, bi);
        }
        prev_cmd = b;
    }
}
