mod terminal;
mod buffer;
mod screen;
mod window;

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

fn main() {
    terminal::enter_raw_mode();
    let mut bufs = vec![buffer::Buffer::new("*demo*".to_string(), "This is the first line\nof the demo buffer.\n".to_string())];
    let mut tree = window::new_tree(0);
    let mut active = 0;
    let mut msg = "".to_string();
    loop {
        tree = screen::draw_all(&bufs, tree, active, msg.to_string());
        let bi = tree.leaves[active].buffer;
        bufs[bi].point = tree.leaves[active].point;
        bufs[bi].mark = tree.leaves[active].mark;
        let b = terminal::read_byte();
        if b == 0x03 || b == 0 {
            break; // Ctrl-c / EOF
        }
        let mut moved = false;
        if b == 0x18 {
            let n = terminal::read_byte();
            if n == 0x03 {
                break; // Ctrl-x Ctrl-c
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
            } else if n == 0x06 { // Ctrl-x Ctrl-f: load
                let path = terminal::read_line("Find file: ".to_string());
                if path.len() > 0 {
                    bufs[bi] = buffer::Buffer::new(path.to_string(), read_file(path.to_string()));
                    bufs[bi].filename = Some(path);
                    bufs[bi].modified = false;
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
                let path = terminal::read_line("Write file: ".to_string());
                if path.len() > 0 {
                    if write_file(path.to_string(), bufs[bi].text()) {
                        bufs[bi].modified = false;
                        msg = format!("Wrote {}", path);
                    } else {
                        msg = format!("Failed to write {}", path);
                    }
                }
            } else if n == 0x09 { // Ctrl-x Ctrl-i: insert-file
                let path = terminal::read_line("Insert file: ".to_string());
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
            }
        } else if b == 0x1b {
            let n = terminal::read_byte(); // Meta prefix
            if n == 0x62 {
                bufs[bi].backward_word();
            } else if n == 0x66 {
                bufs[bi].forward_word();
            } else if n == 0x3c {
                bufs[bi].beginning_of_buffer(); // C-</C-> arrive as Meta form over raw ttys
            } else if n == 0x3e {
                bufs[bi].end_of_buffer();
            }
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
        } else if b == 0x0b {
            bufs[bi].kill_to_end_of_line(); // Ctrl-k
        } else if b == 0x05 {
            bufs[bi].end_of_line(); // Ctrl-e
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
    }
}
