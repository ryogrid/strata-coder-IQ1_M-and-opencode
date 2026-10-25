mod terminal;
mod buffer;
mod screen;
mod window;
mod highlight;
mod keymap;

use keymap::Cmd;

// Returns the file contents, or Err(reason) when the file cannot be read
// (M-9: callers must report this - loading silently as empty lost errors).
fn read_file(path: &str) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) => Err(format!("{}", e)),
    }
}

fn write_file(path: &str, text: &str) -> bool {
    std::fs::write(path, text.as_bytes()).is_ok()
}

fn find_buffer(bufs: &[buffer::Buffer], name: &str) -> Vec<usize> {
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


// M-11 (Design Doc D): the whole mutable loop state in ONE owner. Key
// dispatch resolves through the keymap binding table; each command is one
// method (bodies are the old if-else chain branches, migrated verbatim).
struct Ed {
    bufs: Vec<buffer::Buffer>,
    tree: window::Tree,
    active: usize,
    msg: String,
    killed: String,
    // M-12: byte of the PREVIOUS top-level command only (a simplified
    // Emacs last-command: the C-k kill-run test is prev_cmd == 0x0b;
    // bytes consumed INSIDE a C-s/C-x prefix do not touch it mid-command,
    // and chaining C-y to C-y is intentionally not expressible). README
    // documents exactly this "any other command ends the run".
    prev_cmd: u8,
    moved: bool,
}

impl Ed {
    fn bi(&self) -> usize {
        self.tree.leaves[self.active].buffer
    }

    fn frame(&mut self) {
        // M-11: draw_all consumes and returns the Tree; swap the field
        // out and back (the dummy tree from new_tree is dropped unused).
        let t = std::mem::replace(&mut self.tree, window::new_tree(0));
        self.tree = screen::draw_all(&self.bufs, t, self.active, self.msg.to_string());
    }

    fn sync_in(&mut self) {
        // M-15: a leaf's point/mark go STALE when its buffer is replaced in
        // place (C-x k while several leaves alias one buffer). Clamp before
        // copying into the buffer: insert/kill at point > len panics in
        // move_gap (index out of bounds). The clamp writes BOTH ways
        // (leaf and buffer).
        let bi = self.bi();
        let lp = if self.tree.leaves[self.active].point > self.bufs[bi].len() { self.bufs[bi].len() } else { self.tree.leaves[self.active].point };
        let lm = if self.tree.leaves[self.active].mark > self.bufs[bi].len() { self.bufs[bi].len() } else { self.tree.leaves[self.active].mark };
        self.tree.leaves[self.active].point = lp;
        self.tree.leaves[self.active].mark = lm;
        self.bufs[bi].point = lp;
        self.bufs[bi].mark = lm;
    }

    fn sync_out(&mut self) {
        if !self.moved {
            let bi = self.bi();
            sync_leaf(&mut self.tree, &self.bufs, self.active, bi);
        }
    }

    fn quit(&mut self) -> bool {
        // true = the main loop breaks. C-c cancel leaves msg Cancelled and
        // lands on the plain map's silent Noop (no 0x03 arm, Doc D §2.3).
        if confirm_exit(&self.bufs) {
            return true;
        }
        self.msg = "Cancelled".to_string();
        false
    }

    fn arrow(&mut self, d: usize) {
        // ESC [ t is read by main (3-byte order, M-3); d from arrow_key.
        let bi = self.bi();
        if d == 0 {
            self.bufs[bi].previous_line(); // Up
        } else if d == 1 {
            self.bufs[bi].next_line(); // Down
        } else if d == 2 {
            self.bufs[bi].forward_char(); // Right
        } else {
            self.bufs[bi].backward_char(); // Left
        }
    }

    fn delete_other_windows(&mut self) {
     // Ctrl-x 1: delete-other-windows
        self.active = self.tree.collapse(self.active); // S-3 compaction remaps the leaf index
        self.msg = "".to_string();
    }

    fn split_below(&mut self) {
     // Ctrl-x 2: split-window-below
        self.tree.split(self.active, true);
        self.msg = "".to_string();
    }

    fn split_right(&mut self) {
     // Ctrl-x 3: split-window-right
        self.tree.split(self.active, false);
        self.msg = "".to_string();
    }

    fn other_window(&mut self) {
    let bi = self.bi();
     // Ctrl-x o: other-window
        sync_leaf(&mut self.tree, &self.bufs, self.active, bi);
        let order = self.tree.flatten();
        let mut j = 0;
        while j < order.len() {
            if order[j] == self.active {
                if j + 1 < order.len() {
                    self.active = order[j + 1];
                } else {
                    self.active = order[0];
                }
                break;
            }
            j += 1;
        }
        self.moved = true;
        self.msg = "".to_string();
    }

    fn find_file(&mut self) {
    // Ctrl-x Ctrl-f: open file; in a lone window split below, in a split
    // tree load into the active window
        let path = terminal::read_line_files("Find file: ");
        // T-8: ~ expands to $HOME BEFORE matching/loading, so
        // buffer names (and C-x C-s targets) are real paths.
        let path = terminal::expand_tilde(path);
        if !path.is_empty() {
            // M-6: an already-open file REUSES its buffer (Emacs
            // find-file). Matching is by buffer name (= path at load,
            // the same basis as find_buffer/C-x b); a C-x C-w-bound
            // buffer keeps its own name and is not matched. No disk
            // re-read: buffer edits and the modified flag survive.
            let existing = find_buffer(&self.bufs, &path);
            if let Some(&nb) = existing.first() {
                if self.tree.leaves[self.active].buffer == nb {
                    self.msg = "".to_string(); // already showing it: no-op
                } else {
                    show_buffer(&mut self.tree, &mut self.active, nb);
                    self.moved = true;
                    self.msg = "".to_string();
                }
            } else {
                match read_file(&path) {
                    Err(e) => {
                        // M-9: an unreadable file does NOT become an empty
                        // loaded buffer; report it, keep the current window.
                        self.msg = format!("Failed to open {}: {}", path, e);
                    }
                    Ok(text) => {
                        let nb = self.bufs.len();
                        self.bufs.push(buffer::Buffer::new(path.clone(), text));
                        self.bufs[nb].filename = Some(path.clone());
                        self.bufs[nb].modified = false;
                        show_buffer(&mut self.tree, &mut self.active, nb);
                        self.moved = true;
                        self.msg = "".to_string();
                    }
                }
            }
        }
    }

    fn save(&mut self) {
    let bi = self.bi();
     // Ctrl-x Ctrl-s: save
        let path = self.bufs[bi].filename.clone().unwrap_or_default();
        if !path.is_empty() {
            if write_file(&path, &self.bufs[bi].text()) {
                self.bufs[bi].modified = false;
                self.msg = format!("Wrote {}", path);
            } else {
                self.msg = format!("Failed to write {}", path);
            }
        } else {
            self.msg = "No file name for this buffer".to_string();
        }
    }

    fn write_file_cmd(&mut self) {
    let bi = self.bi();
     // Ctrl-x Ctrl-w: write-file
        let path = terminal::read_line_files("Write file: ");
        let path = terminal::expand_tilde(path); // T-8
        if !path.is_empty() {
            // M-8: the filename binds ONLY on success; a failed
            // write leaves save-target (and C-x C-s) untouched.
            if write_file(&path, &self.bufs[bi].text()) {
                self.bufs[bi].filename = Some(path.clone());
                self.bufs[bi].modified = false;
                self.msg = format!("Wrote {}", path);
            } else {
                self.msg = format!("Failed to write {}", path);
            }
        }
    }

    fn insert_file(&mut self) {
    let bi = self.bi();
     // Ctrl-x Ctrl-i: insert-file
        let path = terminal::read_line_files("Insert file: ");
        let path = terminal::expand_tilde(path); // T-8
        if !path.is_empty() {
            match read_file(&path) {
                Ok(text) => {
                    self.bufs[bi].insert_string(&text);
                    self.msg = format!("Inserted file {}", path);
                }
                Err(e) => {
                    // M-9: report the failure; insert nothing
                    self.msg = format!("Failed to open {}: {}", path, e);
                }
            }
        }
    }

    fn kill_buffer(&mut self) {
    let bi = self.bi();
     // Ctrl-x k: kill-buffer (asks first when modified, M-5)
        let name = self.bufs[bi].name.clone();
        let proceed = if !self.bufs[bi].modified {
            true
        } else {
            // EOF (stdin closed) reads as "" = Cancelled; the main
            // loop's own EOF read then exits (M-4 policy: unconfirmed
            // exit loses data, which is documented).
            let sel = terminal::read_line(format!("Kill buffer {}? (yes|no) ", name));
            sel == "yes" || sel == "y"
        };
        if !proceed {
            self.msg = "Cancelled".to_string();
        } else {
            self.bufs[bi] = buffer::Buffer::new(name.clone(), "");
            self.bufs[bi].filename = None;
            self.bufs[bi].modified = false;
            // M-15: leaves ALIASING this buffer (C-x 2/C-x 3/C-x C-f
            // reuse keep point/mark/scroll per leaf) keep positions
            // from the killed content: reset EVERY leaf showing bi,
            // not just the active one.
            let mut lk = 0;
            while lk < self.tree.leaves.len() {
                if self.tree.leaves[lk].buffer == bi {
                    self.tree.leaves[lk].point = 0;
                    self.tree.leaves[lk].mark = 0;
                    self.tree.leaves[lk].scroll = 0;
                }
                lk += 1;
            }
            self.msg = format!("Killed buffer {}", name);
        }
    }

    fn switch_buffer(&mut self, listform: bool) {
    let bi = self.bi();
     // Ctrl-x b / Ctrl-x Ctrl-b: switch-to-buffer & buffer list select
        let mut names = "".to_string();
        let mut bnames: Vec<String> = vec![];
        let mut j = 0;
        while j < self.bufs.len() {
            bnames.push(self.bufs[j].name.clone());
            if j > 0 { names = format!("{} {}", names, self.bufs[j].name); } else { names = self.bufs[j].name.clone(); }
            j += 1;
        }
        let prompt = if listform { format!("Buffers: {} Enter=next, name: ", names) } else { "Switch to buffer: ".to_string() };
        let sel = terminal::read_line_buffers(prompt, bnames);
        let mut ti = 999999;
        if !sel.is_empty() {
            let hit = find_buffer(&self.bufs, &sel);
            if !hit.is_empty() { ti = hit[0]; }
        } else if listform { // Ctrl-x Ctrl-b + Enter: cycle to the next buffer
            let nxt = if bi + 1 < self.bufs.len() { bi + 1 } else { 0 };
            ti = nxt;
        }
        if ti == 999999 && sel.is_empty() {
            self.msg = "Cancelled".to_string();
        } else if ti < self.bufs.len() {
            attach_leaf(&mut self.tree, self.active, ti);
            self.moved = true;
            self.msg = "".to_string();
        } else {
            self.bufs.push(buffer::Buffer::new(sel, ""));
            attach_leaf(&mut self.tree, self.active, self.bufs.len() - 1);
            self.moved = true;
            self.msg = "".to_string();
        }
    }

    fn isearch(&mut self) {
    let bi = self.bi();
        // Type the query one char at a time; each char extends the search
    // (wrapping at the buffer end) and moves point to the match end.
    // A char that matches nowhere rings the bell and is ignored.
    // C-s while searching repeats the current query. Enter accepts the
    // current match; Ctrl-g cancels (point restored); ESC also ends.
    let orig = self.bufs[bi].point;
    let mut qs: Vec<char> = vec![];
    let mut last = orig; // start position of the current match
    let szs = screen::term_size();
    let prow = if szs.rows > 0 { szs.rows } else { 24 };
    terminal::emit(format!("\x1b[{};1H\x1b[2KSearch: ", prow));
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
                    self.bufs[bi].previous_line(); // Up
                } else if t == 0x42 {
                    self.bufs[bi].next_line(); // Down
                } else if t == 0x43 {
                    self.bufs[bi].forward_char(); // Right
                } else if t == 0x44 {
                    self.bufs[bi].backward_char(); // Left
                }
            }
            break;
        }
        if c2 == 0x07 { cancelled = true; break; } // Ctrl-g: cancel
        if c2 == 0x7f { // backspace: drop the last query char
            if !qs.is_empty() { qs.pop(); }
            if !qs.is_empty() {
                let q2 = terminal::str_of(&qs);
                let hit = self.bufs[bi].find_forward(&q2, 0);
                if let Some(end) = hit {
                    self.bufs[bi].point = end;
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
        let q2 = terminal::str_of(&attempt);
        let hit = self.bufs[bi]
            .find_forward(&q2, from)
            .or_else(|| self.bufs[bi].find_forward(&q2, 0)); // wrap once
        if let Some(end) = hit {
            qs = attempt;
            self.bufs[bi].point = end;
            last = end - qs.len();
            if c2 != 0x13 {
                terminal::emit(echo_ch.clone()); // echo the query char
            }
            // (C-s repeats print nothing: the old trailing flush
            // was an empty-buffer no-op)
        } else {
            terminal::emit("\x07".to_string()); // bell: char ignored
        }
    }
    if cancelled {
        self.bufs[bi].point = orig;
        self.msg = "Cancelled".to_string();
    } else {
        self.msg = "".to_string();
    }
    }

    fn find_backward(&mut self) {
    let bi = self.bi();
        let q = terminal::read_line("Reverse find: ");
    if !q.is_empty() {
        let hit = self.bufs[bi].find_backward(&q, self.bufs[bi].point);
        if let Some(start) = hit {
            self.bufs[bi].point = start;
            self.msg = "".to_string();
        } else {
            self.msg = format!("Search string not found: {}", q);
        }
    } else {
        self.msg = "Cancelled".to_string();
    }
    }

    fn backward_word(&mut self) {
    let bi = self.bi();
            self.bufs[bi].backward_word();
    }

    fn forward_word(&mut self) {
    let bi = self.bi();
            self.bufs[bi].forward_word();
    }

    fn beginning_of_buffer(&mut self) {
    let bi = self.bi();
            self.bufs[bi].beginning_of_buffer(); // C-</C-> arrive as Meta form over raw ttys
    }

    fn end_of_buffer(&mut self) {
    let bi = self.bi();
            self.bufs[bi].end_of_buffer();
    }

    fn set_mark(&mut self) {
    let bi = self.bi();
        self.bufs[bi].mark = self.bufs[bi].point;
    self.msg = "".to_string();
    }

    fn beginning_of_line(&mut self) {
    let bi = self.bi();
        self.bufs[bi].beginning_of_line(); // Ctrl-a
    }

    fn backward_char(&mut self) {
    let bi = self.bi();
        self.bufs[bi].backward_char(); // Ctrl-b
    }

    fn forward_char(&mut self) {
    let bi = self.bi();
        self.bufs[bi].forward_char(); // Ctrl-f
    }

    fn next_line(&mut self) {
    let bi = self.bi();
        self.bufs[bi].next_line(); // Ctrl-n
    }

    fn previous_line(&mut self) {
    let bi = self.bi();
        self.bufs[bi].previous_line(); // Ctrl-p
    }

    fn delete_backward(&mut self) {
    let bi = self.bi();
        self.bufs[bi].delete_backward(); // backspace (DEL)
    }

    fn delete_forward(&mut self) {
    let bi = self.bi();
        self.bufs[bi].delete_forward(); // Ctrl-d
    }

    fn open_line(&mut self) {
    let bi = self.bi();
        self.bufs[bi].insert_newline_after(); // Ctrl-o
    }

    fn transpose_chars(&mut self) {
    let bi = self.bi();
        self.bufs[bi].transpose_chars(); // Ctrl-t
    }

    fn kill_line(&mut self) {
    let bi = self.bi();
        let cut = self.bufs[bi].kill_to_end_of_line();
    if self.prev_cmd == 0x0b {
        self.killed.push_str(&cut); // consecutive kills accumulate
    } else {
        self.killed = cut;
    }
    }

    fn yank(&mut self) {
    let bi = self.bi();
        self.bufs[bi].insert_string(&self.killed);
    }

    fn end_of_line(&mut self) {
    let bi = self.bi();
        self.bufs[bi].end_of_line(); // Ctrl-e
    }

    fn goto_line(&mut self) {
    let bi = self.bi();
        let q = terminal::read_line("Go to line: ");
    if q.is_empty() {
        self.msg = "Cancelled".to_string();
    } else {
        // M-1: digits-only gate (parse alone accepts a leading '+'),
        // u64 parse (a 15-digit number is an Ok value, never a u32
        // overflow panic), and a total-line-count scan that bounds
        // the movement loop: huge numbers clamp to the last line
        // (documented behavior) and can no longer spin the loop.
        let digits_ok = q.bytes().all(|c| c.is_ascii_digit());
        let want = if digits_ok { q.parse::<u64>().ok() } else { None };
        match want {
            None => self.msg = format!("Invalid line number: {}", q),
            Some(0) => self.msg = format!("Invalid line number: {}", q),
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
                while i < self.bufs[bi].len() {
                    if self.bufs[bi].char_at(i) == '\n' {
                        line += 1;
                        if line <= want_ln {
                            tpos = i + 1;
                        } else {
                            break; // tpos final: want_ln line start recorded
                        }
                    }
                    i += 1;
                }
                self.bufs[bi].point = tpos;
                self.msg = "".to_string();
            }
        }
    }
    }

    fn quote_insert(&mut self) {
    let bi = self.bi();
        // NB: at EOF the leading byte arrives as 0 (read_byte), which is
    // Ctrl-space's NUL - a literal NUL gets inserted once. Bounded:
    // the loop-top read_byte_eof then ends the session. (Design Doc A)
    let n = terminal::read_byte();
    if let Some(c) = terminal::read_char_eof(n) {
        self.bufs[bi].insert_char(c);
    } // EOF: insert nothing
    }

    fn newline(&mut self) {
    let bi = self.bi();
        self.bufs[bi].insert_char('\n'); // Ctrl-j / Ctrl-m: newline
    }

    fn self_insert(&mut self, b: u8) {
    let bi = self.bi();
        // self-insert (ASCII, and UTF-8 lead bytes: read_char_eof consumes
    // the continuation bytes)
    if let Some(c) = terminal::read_char_eof(b) {
        self.bufs[bi].insert_char(c);
    } // EOF: insert nothing
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
        let path = args[1].clone();
        match read_file(&path) {
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
    // M-11 (Design Doc D): all loop state in Ed; dispatch through the
    // keymap table; one method per command (chain bodies verbatim).
    let mut ed = Ed {
        bufs,
        tree: window::new_tree(0),
        active: 0,
        msg,
        killed: "".to_string(),
        prev_cmd: 0,
        moved: false,
    };
    loop {
        ed.frame();
        ed.sync_in();
        let rb = terminal::read_byte_eof();
        if rb[1] == 1 {
            break; // EOF (no byte read at all)
        }
        let b = rb[0];
        ed.moved = false;
        // The TOP dispatch is exhaustive over Cmd (no `_`): a new variant
        // breaks the build here (Design Doc D §3.1). Byte-map sub-matches
        // carry `_ => {}` for variants their byte map cannot produce.
        match keymap::plain_key(b) {
            Cmd::Quit => {
                if ed.quit() {
                    break;
                }
            }
            Cmd::PrefixCtrlX => {
                let n = terminal::read_byte();
                match keymap::ctrl_x_key(n) {
                    Cmd::Quit => {
                        if ed.quit() {
                            break;
                        }
                    }
                    Cmd::DeleteOtherWindows => ed.delete_other_windows(),
                    Cmd::SplitBelow => ed.split_below(),
                    Cmd::SplitRight => ed.split_right(),
                    Cmd::OtherWindow => ed.other_window(),
                    Cmd::FindFile => ed.find_file(),
                    Cmd::Save => ed.save(),
                    Cmd::WriteFile => ed.write_file_cmd(),
                    Cmd::InsertFile => ed.insert_file(),
                    Cmd::KillBuffer => ed.kill_buffer(),
                    Cmd::SwitchBuffer(listform) => ed.switch_buffer(listform),
                    Cmd::Unbound(_, nb) => {
                        // M-7: an unbound C-x sequence reports itself
                        // explicitly (prefix EOF reads byte 0 -> "C-x \0").
                        ed.msg = format!("Undefined command: C-x {}", nb as char);
                    }
                    _ => {}
                }
            }
            Cmd::PrefixMeta => {
                let n = terminal::read_byte();
                match keymap::meta_key(n) {
                    Cmd::PrefixArrow => {
                        let t = terminal::read_byte();
                        if let Cmd::Arrow(d) = keymap::arrow_key(t) {
                            ed.arrow(d);
                        } // unbound t: silent (as today)
                    }
                    Cmd::BackwardWord => ed.backward_word(),
                    Cmd::ForwardWord => ed.forward_word(),
                    Cmd::BeginningOfBuffer => ed.beginning_of_buffer(),
                    Cmd::EndOfBuffer => ed.end_of_buffer(),
                    _ => {} // MetaUnbound: silent
                }
            }
            Cmd::Isearch => ed.isearch(),
            Cmd::FindBackward => ed.find_backward(),
            Cmd::Cancel => ed.msg = "Cancelled".to_string(),
            Cmd::Noop => {} // unbound plain byte: SILENT (Doc D §2.3)
            Cmd::SetMark => ed.set_mark(),
            Cmd::BeginningOfLine => ed.beginning_of_line(),
            Cmd::BackwardChar => ed.backward_char(),
            Cmd::ForwardChar => ed.forward_char(),
            Cmd::NextLine => ed.next_line(),
            Cmd::PreviousLine => ed.previous_line(),
            Cmd::DeleteBackward => ed.delete_backward(),
            Cmd::DeleteForward => ed.delete_forward(),
            Cmd::OpenLine => ed.open_line(),
            Cmd::TransposeChars => ed.transpose_chars(),
            Cmd::KillLine => ed.kill_line(),
            Cmd::Yank => ed.yank(),
            Cmd::EndOfLine => ed.end_of_line(),
            Cmd::GotoLine => ed.goto_line(),
            Cmd::QuoteInsert => ed.quote_insert(),
            Cmd::Newline => ed.newline(),
            Cmd::SelfInsert => ed.self_insert(b),
            // not producible by plain_key (Meta/C-x-only commands):
            Cmd::PrefixArrow => {}
            Cmd::Arrow(_) => {}
            Cmd::Unbound(..) => {}
            Cmd::MetaUnbound => {}
            Cmd::BeginningOfBuffer | Cmd::EndOfBuffer => {}
            Cmd::BackwardWord | Cmd::ForwardWord => {}
            Cmd::FindFile | Cmd::Save => {}
            Cmd::WriteFile | Cmd::InsertFile => {}
            Cmd::KillBuffer | Cmd::SwitchBuffer(_) => {}
            Cmd::DeleteOtherWindows | Cmd::SplitBelow => {}
            Cmd::SplitRight | Cmd::OtherWindow => {}
        }
        ed.sync_out();
        ed.prev_cmd = b;
    }
}
