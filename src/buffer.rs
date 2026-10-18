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

pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
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
            self.chars[self.gap_end] = self.chars[self.gap_start];
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

    pub fn char_at(self: &Buffer, pos: usize) -> char {
        self.chars[self.to_index(pos)]
    }

    pub fn line_start(self: &Buffer, mut pos: usize) -> usize {
        while pos > 0 && self.char_at(pos - 1) != '\n' {
            pos -= 1;
        }
        pos
    }

    pub fn line_end(self: &Buffer, mut pos: usize) -> usize {
        while pos < self.len() && self.char_at(pos) != '\n' {
            pos += 1;
        }
        pos
    }

    pub fn line_column(self: &Buffer, pos: usize) -> usize {
        pos - self.line_start(pos)
    }

    pub fn forward_char(self: &mut Buffer) {
        if self.point < self.len() {
            self.point += 1;
        }
    }

    pub fn backward_char(self: &mut Buffer) {
        if self.point > 0 {
            self.point -= 1;
        }
    }

    pub fn beginning_of_line(self: &mut Buffer) {
        self.point = self.line_start(self.point);
    }

    pub fn end_of_line(self: &mut Buffer) {
        self.point = self.line_end(self.point);
    }

    pub fn next_line(self: &mut Buffer) {
        let col = self.line_column(self.point);
        let lend = self.line_end(self.point);
        if lend < self.len() {
            let start = lend + 1;
            let tend = self.line_end(start);
            let max_col = tend - start;
            self.point = start + if col < max_col { col } else { max_col };
        }
    }

    pub fn previous_line(self: &mut Buffer) {
        let ls = self.line_start(self.point);
        if ls > 0 {
            let col = self.line_column(self.point);
            let prev_end = ls - 1;
            let prev_start = self.line_start(prev_end);
            let max_col = prev_end - prev_start;
            self.point = prev_start + if col < max_col { col } else { max_col };
        }
    }

    pub fn beginning_of_buffer(self: &mut Buffer) {
        self.point = 0;
    }

    pub fn end_of_buffer(self: &mut Buffer) {
        self.point = self.len();
    }

    pub fn insert_newline_after(self: &mut Buffer) {
        self.move_gap(self.point);
        if self.gap_start < self.gap_end {
            self.chars[self.gap_start] = '\n';
            self.gap_start += 1;
        } else {
            self.chars.insert(self.gap_start, '\n');
            self.gap_end += 1;
        }
        self.mark = self.point;
        self.modified = true;
    }

    pub fn swap_at(self: &mut Buffer, i: usize) {
        let a = self.to_index(i);
        let b = self.to_index(i + 1);
        let tmp = self.chars[a];
        self.chars[a] = self.chars[b];
        self.chars[b] = tmp;
        self.modified = true;
    }

    pub fn transpose_chars(self: &mut Buffer) {
        if self.point > 0 {
            if self.point < self.len() {
                self.swap_at(self.point - 1);
                self.point += 1;
            } else if self.len() > 1 {
                self.swap_at(self.len() - 2);
            }
        } else if self.len() > 1 {
            self.swap_at(0);
            self.point = 1;
        }
    }

    pub fn kill_to_end_of_line(self: &mut Buffer) {
        if self.point < self.len() && self.char_at(self.point) == '\n' {
            self.delete_forward();
        } else {
            let n = self.line_end(self.point) - self.point;
            let mut i = 0;
            while i < n {
                self.delete_forward();
                i += 1;
            }
        }
    }

    pub fn forward_word(self: &mut Buffer) {
        while self.point < self.len() && !is_word_char(self.char_at(self.point)) {
            self.point += 1;
        }
        while self.point < self.len() && is_word_char(self.char_at(self.point)) {
            self.point += 1;
        }
    }

    pub fn backward_word(self: &mut Buffer) {
        if self.point > 0 && is_word_char(self.char_at(self.point - 1)) {
            while self.point > 0 && is_word_char(self.char_at(self.point - 1)) {
                self.point -= 1;
            }
        } else {
            while self.point > 0 && !is_word_char(self.char_at(self.point - 1)) {
                self.point -= 1;
            }
            while self.point > 0 && is_word_char(self.char_at(self.point - 1)) {
                self.point -= 1;
            }
        }
    }

    pub fn find_forward(self: &Buffer, pat: &String, from: usize) -> Vec<usize> {
        let mut tchars: Vec<char> = vec![];
        for c in self.text().chars() {
            tchars.push(c);
        }
        let mut pchars: Vec<char> = vec![];
        for c in pat.chars() {
            pchars.push(c);
        }
        if pchars.len() == 0 || from + pchars.len() > tchars.len() {
            return vec![];
        }
        let mut i = from;
        while i + pchars.len() <= tchars.len() {
            let mut ok = true;
            let mut k = 0;
            while k < pchars.len() {
                if tchars[i + k] != pchars[k] {
                    ok = false;
                }
                k += 1;
            }
            if ok {
                return vec![i + pchars.len()];
            }
            i += 1;
        }
        vec![]
    }

    pub fn find_backward(self: &Buffer, pat: &String, from: usize) -> Vec<usize> {
        let mut tchars: Vec<char> = vec![];
        for c in self.text().chars() {
            tchars.push(c);
        }
        let mut pchars: Vec<char> = vec![];
        for c in pat.chars() {
            pchars.push(c);
        }
        if pchars.len() == 0 || pchars.len() > tchars.len() || from == 0 {
            return vec![];
        }
        let mut i = from - 1;
        if i > tchars.len() - pchars.len() {
            i = tchars.len() - pchars.len();
        }
        while true {
            let mut ok = true;
            let mut k = 0;
            while k < pchars.len() {
                if tchars[i + k] != pchars[k] {
                    ok = false;
                }
                k += 1;
            }
            if ok {
                return vec![i];
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
        vec![]
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
#[test]
fn cursor_movement() {
    let mut b = Buffer::new("t".to_string(), "one\ntwo22\nthree".to_string());
    b.point = 6;
    b.beginning_of_line();
    assert!(b.point == 4);
    b.next_line();
    assert!(b.point == 10);
    b.end_of_line();
    assert!(b.point == 15);
    b.previous_line();
    assert!(b.point == 9);
    b.backward_char();
    b.backward_char();
    assert!(b.point == 7);
    b.forward_char();
    assert!(b.point == 8);
}
#[test]
fn editing_and_navigation() {
    let mut b = Buffer::new("t".to_string(), "hello big world".to_string());
    b.point = 2;
    b.forward_word();
    assert!(b.point == 5);
    b.forward_word();
    assert!(b.point == 9);
    b.backward_word();
    assert!(b.point == 6);
    b.point = 11;
    b.forward_word();
    assert!(b.point == 15);
    b.end_of_buffer();
    assert!(b.point == 15);
    b.backward_word();
    assert!(b.point == 10);
    b.kill_to_end_of_line();
    assert!(b.text() == "hello big ");
    b.point = 10;
    b.transpose_chars();
    assert!(b.text() == "hello bi g");
    b.point = 5;
    b.insert_newline_after();
    assert!(b.text() == "hello\n bi g");
    b.beginning_of_buffer();
    assert!(b.point == 0);
}
}
