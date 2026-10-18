mod terminal;
mod buffer;
mod screen;

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
    let mut buf = buffer::Buffer::new("*demo*".to_string(), "This is the first line\nof the demo buffer.\n".to_string());
    let mut msg = "".to_string();
    loop {
        screen::draw(&buf, msg.to_string(), 0);
        let b = terminal::read_byte();
        if b == 0x03 || b == 0 {
            break; // Ctrl-c / EOF
        }
        if b == 0x18 {
            let n = terminal::read_byte();
            if n == 0x03 {
                break; // Ctrl-x Ctrl-c
            }
            if n == 0x06 { // Ctrl-x Ctrl-f: load
                let path = terminal::read_line("Find file: ".to_string());
                if path.len() > 0 {
                    buf = buffer::Buffer::new(path.to_string(), read_file(path.to_string()));
                    buf.filename = Some(path);
                    buf.modified = false;
                    msg = "".to_string();
                }
            } else if n == 0x13 { // Ctrl-x Ctrl-s: save
                let path = buf.filename.clone().map(|p| p.to_string()).unwrap_or("".to_string());
                if path.len() > 0 {
                    if write_file(path.to_string(), buf.text()) {
                        buf.modified = false;
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
                    if write_file(path.to_string(), buf.text()) {
                            buf.modified = false;
                            msg = format!("Wrote {}", path);
                        } else {
                            msg = format!("Failed to write {}", path);
                        }
                }
            } else if n == 0x09 { // Ctrl-x Ctrl-i: insert-file
                let path = terminal::read_line("Insert file: ".to_string());
                if path.len() > 0 {
                    for c in read_file(path.to_string()).chars() {
                        buf.insert_char(c);
                    }
                    msg = format!("Inserted file {}", path);
                }
            } else if n == 0x6b { // Ctrl-x k: kill-buffer
                let name = buf.name;
                buf = buffer::Buffer::new(name.to_string(), "".to_string());
                buf.filename = None;
                buf.modified = false;
                msg = format!("Killed buffer {}", name);
            }
            continue;
        }
        if b == 0x1b {
            let n = terminal::read_byte(); // Meta prefix
            if n == 0x62 {
                buf.backward_word();
            } else if n == 0x66 {
                buf.forward_word();
            } else if n == 0x3c {
                buf.beginning_of_buffer(); // C-</C-> arrive as Meta form over raw ttys
            } else if n == 0x3e {
                buf.end_of_buffer();
            }
            continue; // Meta prefix binds later
        }
        if b == 0x01 {
            buf.beginning_of_line(); // Ctrl-a
        } else if b == 0x02 {
            buf.backward_char(); // Ctrl-b
        } else if b == 0x06 {
            buf.forward_char(); // Ctrl-f
        } else if b == 0x0e {
            buf.next_line(); // Ctrl-n
        } else         if b == 0x10 {
            buf.previous_line(); // Ctrl-p
        } else if b == 0x7f {
            buf.delete_backward(); // backspace (DEL)
        } else if b == 0x04 {
            buf.delete_forward(); // Ctrl-d
        } else if b == 0x0f {
            buf.insert_newline_after(); // Ctrl-o
        } else if b == 0x14 {
            buf.transpose_chars(); // Ctrl-t
        } else if b == 0x0b {
            buf.kill_to_end_of_line(); // Ctrl-k
        } else if b == 0x05 {
            buf.end_of_line(); // Ctrl-e
        } else if b == 0x11 {
            let n = terminal::read_byte(); // Ctrl-q: quote next key
            buf.insert_char(terminal::read_char_from(n));
        } else if b >= 0x20 && b < 0x7f {
            buf.insert_char(terminal::read_char_from(b)); // self-insert (ASCII)
        } else if b >= 0x80 {
            buf.insert_char(terminal::read_char_from(b)); // self-insert (UTF-8)
        }
    }
}
