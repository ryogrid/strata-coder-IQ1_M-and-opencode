mod terminal;
mod buffer;
mod screen;
mod window;
mod highlight;

fn read_file(path: String) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => "".to_string(),
    }
}

fn write_file(path: String, text: String) -> bool {
    match std::fs::write(path, text.as_bytes()) {
        Ok(_) => true,
        Err(_) => false,
    }
}

fn find_buffer(bufs: &[buffer::Buffer], name: String) -> Vec<usize> {
    let mut j = 0;
    while j < bufs.len() {
        if bufs[j].name == name {
            return vec![j];
        }
        j += 1;
    }
    return vec![];
}

// Quit confirmation (Emacs: "Modified buffers exist; exit anyway? (yes|no)"):
// exiting via Ctrl-c / Ctrl-x Ctrl-c asks in the minibuffer when any buffer
// has unsaved modifications. EOF (stdin closed / session killed) exits
// unconditionally. Only "yes" (or "y") exits; anything else cancels.
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
    let sel = terminal::read_line("Modified buffers exist; exit anyway? (yes|no) ".to_string());
    return sel == "yes".to_string() || sel == "y".to_string();
}

fn main() {
    let saved_tty = terminal::save_raw_mode();
    let args: Vec<String> = std::env::args().collect();
    let mut bufs = vec![buffer::Buffer::new("*demo*".to_string(), "This is the first line\nof the demo buffer.\n".to_string())];
    if args.len() > 1 {
        let path = args[1].to_string();
        let text = read_file(path.to_string());
        bufs[0] = buffer::Buffer::new(path.to_string(), text);
        bufs[0].filename = Some(path.to_string());
    }
    let mut tree = window::new_tree(0);
    let mut active = 0;
    let mut msg = "".to_string();
    let mut killed = "".to_string();
    let mut prev_cmd = 0;
    loop {
        tree = screen::draw_all(&bufs, tree, active, msg.to_string());
        let bi = tree.leaves[active].buffer;
        bufs[bi].point = tree.leaves[active].point;
        bufs[bi].mark = tree.leaves[active].mark;
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
            }
            if n == 0x31 { // Ctrl-x 1: delete-other-windows
                tree.collapse(active);
                msg = "".to_string();
            } else if n == 0x32 { // Ctrl-x 2: split-window-below
                tree.split(active, true);
                msg = "".to_string();
            } else if n == 0x33 { // Ctrl-x 3: split-window-right
                tree.split(active, false);
                msg = "".to_string();
            } else if n == 0x6f { // Ctrl-x o: other-window
                tree.leaves[active].point = bufs[bi].point;
                tree.leaves[active].mark = bufs[bi].mark;
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
            } else if n == 0x06 { // Ctrl-x Ctrl-f: open file in a split window below
                let path = terminal::read_line_files("Find file: ".to_string());
                if path.len() > 0 {
                    let nb = bufs.len();
                    bufs.push(buffer::Buffer::new(path.to_string(), read_file(path.to_string())));
                    bufs[nb].filename = Some(path.clone());
                    bufs[nb].modified = false;
                    tree.split(active, true);
                    let copy_idx = tree.leaves.len() - 1;
                    tree.leaves[copy_idx].buffer = nb;
                    tree.leaves[copy_idx].point = 0;
                    tree.leaves[copy_idx].mark = 0;
                    tree.leaves[copy_idx].scroll = 0;
                    active = copy_idx;
                    moved = true;
                    msg = "".to_string();
                }
            } else if n == 0x13 { // Ctrl-x Ctrl-s: save
                let path = bufs[bi].filename.clone().map(|p| p.to_string()).unwrap_or("".to_string());
                if path.len() > 0 {
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
                let path = terminal::read_line_files("Write file: ".to_string());
                if path.len() > 0 {
                    bufs[bi].filename = Some(path.clone());
                    if write_file(path.to_string(), bufs[bi].text()) {
                        bufs[bi].modified = false;
                        msg = format!("Wrote {}", path);
                    } else {
                        msg = format!("Failed to write {}", path);
                    }
                }
            } else if n == 0x09 { // Ctrl-x Ctrl-i: insert-file
                let path = terminal::read_line_files("Insert file: ".to_string());
                if path.len() > 0 {
                    for c in read_file(path.to_string()).chars() {
                        bufs[bi].insert_char(c);
                    }
                    msg = format!("Inserted file {}", path);
                }
            } else if n == 0x6b { // Ctrl-x k: kill-buffer
                let name = bufs[bi].name.clone();
                bufs[bi] = buffer::Buffer::new(name.to_string(), "".to_string());
                bufs[bi].filename = None;
                bufs[bi].modified = false;
                msg = format!("Killed buffer {}", name);
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
                if sel.len() > 0 {
                    let hit = find_buffer(&bufs, sel.to_string());
                    if hit.len() > 0 { ti = hit[0]; }
                } else if n == 0x02 { // Ctrl-x Ctrl-b + Enter: cycle to the next buffer
                    let nxt = if bi + 1 < bufs.len() { bi + 1 } else { 0 };
                    ti = nxt;
                }
                if ti == 999999 && sel.len() == 0 {
                    msg = "Cancelled".to_string();
                } else if ti < bufs.len() {
                    tree.leaves[active].buffer = ti;
                    tree.leaves[active].point = 0;
                    tree.leaves[active].mark = 0;
                    tree.leaves[active].scroll = 0;
                    moved = true;
                    msg = "".to_string();
                } else {
                    bufs.push(buffer::Buffer::new(sel.to_string(), "".to_string()));
                    tree.leaves[active].buffer = bufs.len() - 1;
                    tree.leaves[active].point = 0;
                    tree.leaves[active].mark = 0;
                    tree.leaves[active].scroll = 0;
                    moved = true;
                    msg = "".to_string();
                }
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
            let szs = curses::canvas::get_size();
            let prow = if szs.rows > 0 { szs.rows } else { 24 };
            print!("\x1b[{};1H\x1b[2K", prow);
            print!("{}", "Search: ".to_string());
            terminal::flush_stdout();
            let mut cancelled = false;
            loop {
                let rb2 = terminal::read_byte_eof();
                if rb2[1] == 1 { break; } // EOF: keep the current match
                let c2 = rb2[0];
                if c2 == 0x0D || c2 == 0x0A || c2 == 0x1b { break; } // accept
                if c2 == 0x07 { cancelled = true; break; } // Ctrl-g: cancel
                if c2 == 0x7f { // backspace: drop the last query char
                    if qs.len() > 0 { qs.pop(); }
                    if qs.len() > 0 {
                        let q2 = terminal::str_of(qs.clone());
                        let hit = bufs[bi].find_forward(&q2, 0);
                        if hit.len() > 0 {
                            bufs[bi].point = hit[0];
                            last = hit[0] - qs.len();
                        }
                    }
                    continue;
                }
                let mut attempt: Vec<char> = vec![]; // query to search for
                let from = last + 1;
                let mut echo_ch = ' '.to_string(); // char to echo on the search line
                if c2 == 0x13 { // Ctrl-s while searching: repeat the query
                    if qs.len() == 0 { continue; } // still waiting for chars
                    attempt = qs.clone();
                } else { // a printable char extends the query
                    let cc = terminal::read_char_from(c2);
                    attempt = qs.clone();
                    attempt.push(cc);
                    echo_ch = cc.to_string();
                }
                let q2 = terminal::str_of(attempt.clone());
                let hit = bufs[bi].find_forward(&q2, from);
                let hit2 = if hit.len() > 0 { hit } else { bufs[bi].find_forward(&q2, 0) };
                if hit2.len() > 0 {
                    qs = attempt;
                    bufs[bi].point = hit2[0];
                    last = hit2[0] - qs.len();
                    if c2 != 0x13 {
                        print!("{}", echo_ch); // echo the query char
                    }
                    terminal::flush_stdout();
                } else {
                    print!("{}", "\x07".to_string()); // bell: char ignored
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
            let q = terminal::read_line("Reverse find: ".to_string());
            if q.len() > 0 {
                let hit = bufs[bi].find_backward(&q, bufs[bi].point);
                if hit.len() > 0 {
                    bufs[bi].point = hit[0];
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
            for c in killed.chars() {
                bufs[bi].insert_char(c);
            }
        } else if b == 0x05 {
            bufs[bi].end_of_line(); // Ctrl-e
        } else if b == 0x0c { // Ctrl-l: go-to-line (minibuffer reads a line number)
            let q = terminal::read_line("Go to line: ".to_string());
            if q.len() == 0 {
                msg = "Cancelled".to_string();
            } else {
                let mut want_ln: u32 = 0;
                let mut digits = true;
                for c in q.chars() {
                    if c >= '0' && c <= '9' {
                        want_ln = want_ln * 10 + ((c as u32) - ('0' as u32));
                    } else {
                        digits = false;
                    }
                }
                if !digits || want_ln == 0 {
                    msg = format!("Invalid line number: {}", q);
                } else {
                    bufs[bi].beginning_of_buffer();
                    let mut k: u32 = 1;
                    while k < want_ln {
                        bufs[bi].next_line(); // next_line stops at the last line (clamps)
                        k += 1;
                    }
                    msg = "".to_string();
                }
            }
        } else if b == 0x11 {
            let n = terminal::read_byte(); // Ctrl-q: quote next key
            bufs[bi].insert_char(terminal::read_char_from(n));
        } else if b == 0x0a || b == 0x0d {
            bufs[bi].insert_char('\n'); // Ctrl-j / Ctrl-m: newline
        } else if b >= 0x20 && b < 0x7f {
            bufs[bi].insert_char(terminal::read_char_from(b)); // self-insert (ASCII)
        } else if b >= 0x80 {
            bufs[bi].insert_char(terminal::read_char_from(b)); // self-insert (UTF-8)
        }
        if !moved {
            tree.leaves[active].point = bufs[bi].point;
            tree.leaves[active].mark = bufs[bi].mark;
        }
        prev_cmd = b;
    }
    terminal::restore_mode(saved_tty);
}
