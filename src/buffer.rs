// A buffer backed by a gap buffer (the same conceptual structure Emacs uses).
// All positions are logical char indices (0..len). The gap is the half-open
// array range [gap_start, gap_end).
pub struct Buffer {
    pub name: String,
    pub filename: Option<String>,
    pub modified: bool,
    pub chars: Vec<char>,
    pub gap_start: usize,
    pub gap_end: usize,
    pub point: usize,
    pub mark: usize,
}

impl Buffer {
    pub fn new(name: String, text: String) -> Buffer {
        let chars = text.chars().collect::<Vec<char>>();
        Buffer {
            name: name,
            filename: None,
            modified: false,
            chars: chars,
            gap_start: 0,
            gap_end: 0,
            point: 0,
            mark: 0,
        }
    }

    pub fn len(self: &Buffer) -> usize {
        self.chars.len() - (self.gap_end - self.gap_start)
    }

    pub fn to_index(self: &Buffer, pos: usize) -> usize {
        if pos < self.gap_start {
            pos
        } else {
            pos + (self.gap_end - self.gap_start)
        }
    }

    pub fn to_pos(self: &Buffer, idx: usize) -> usize {
        if idx < self.gap_start {
            idx
        } else if idx >= self.gap_end {
            idx - (self.gap_end - self.gap_start)
        } else {
            self.gap_start
        }
    }

    pub fn move_gap(self: &mut Buffer, pos: usize) {
        let target = self.to_index(pos);
        while self.gap_start > target {
            self.gap_start -= 1;
            self.gap_end -= 1;
            self.chars[self.gap_end - 1] = self.chars[self.gap_start];
        }
        while self.gap_end < target {
            self.chars[self.gap_start] = self.chars[self.gap_end];
            self.gap_start += 1;
            self.gap_end += 1;
        }
    }

    pub fn insert_char(self: &mut Buffer, c: char) {
        self.move_gap(self.point);
        if self.gap_start < self.gap_end {
            self.chars[self.gap_start] = c;
            self.gap_start += 1;
        } else {
            self.chars.insert(self.gap_start, c);
            self.gap_start += 1;
            self.gap_end += 1;
        }
        self.point += 1;
        self.mark = self.point;
        self.modified = true;
    }

    pub fn delete_forward(self: &mut Buffer) -> bool {
        self.move_gap(self.point);
        if self.gap_end < self.chars.len() {
            self.gap_end += 1;
            self.modified = true;
            true
        } else {
            false
        }
    }

    pub fn delete_backward(self: &mut Buffer) -> bool {
        if self.point == 0 {
            return false;
        }
        self.move_gap(self.point);
        self.gap_start -= 1;
        self.point -= 1;
        self.mark = self.point;
        self.modified = true;
        true
    }

    pub fn text(self: &Buffer) -> String {
        let mut out: Vec<char> = vec![];
        out.extend(self.chars[0..self.gap_start].iter());
        out.extend(self.chars[self.gap_end..self.chars.len()].iter());
        out.iter().collect::<String>()
    }
}

#[cfg(test)]
mod tests {
use super::*;
#[test]
fn gap_buffer_roundtrip() {
    let mut b = Buffer::new("t".to_string(), "hello".to_string());
    b.point = 2;
    b.insert_char('X');
    assert!(b.text() == "heXllo");
    b.delete_forward();
    assert!(b.text() == "heXlo");
    b.delete_backward();
    assert!(b.text() == "helo");
    assert!(b.len() == 4);
}
}
