mod terminal;
mod buffer;

fn main() {
    terminal::enter_raw_mode();
    loop {
        let b = terminal::read_byte();
        if b == 0x03 {
            break; // Ctrl-c
        }
        if b == 0x18 {
            let n = terminal::read_byte();
            if n == 0x03 {
                break; // Ctrl-x Ctrl-c
            }
        }
    }
}
