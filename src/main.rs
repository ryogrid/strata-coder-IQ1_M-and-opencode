mod terminal;
mod buffer;

fn main() {
    terminal::enter_raw_mode();
    let mut buf = buffer::Buffer::new("*demo*".to_string(), "This is the first line\nof the demo buffer.\n".to_string());
    loop {
        let b = terminal::read_byte();
        if b == 0x03 || b == 0 {
            break; // Ctrl-c / EOF
        }
        if b == 0x18 {
            let n = terminal::read_byte();
            if n == 0x03 {
                break; // Ctrl-x Ctrl-c
            }
            continue; // other Ctrl-x keys bind later
        }
        if b == 0x1b {
            terminal::read_byte();
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
        } else if b == 0x10 {
            buf.previous_line(); // Ctrl-p
        }
    }
}
