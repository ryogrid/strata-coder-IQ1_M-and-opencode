// A buffer backed by a gap buffer (the same conceptual structure Emacs uses).
// All positions are logical char indices (0..len). The gap is the half-open
// array range [gap_start, gap_end). Invariant: chars inside the gap window is
// '\0' PADDING, never live text - only text()/char_at/swap_at (via to_index)
// may address text, and they only accept logical positions pos < len().
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

// Gap pre-allocation size: insertions consume the gap in O(1); an exhausted
// gap is re-widened in place at the insertion point (O(n) once per GAP
// inserts), making insert amortized O(1) (Design Doc B / B-1).
const GAP: usize = 8192;

impl Buffer {
    pub fn new(name: String, text: String) -> Buffer {
        let mut chars = text.chars().collect::<Vec<char>>();
        let n = chars.len();
        // Reserve the gap at the END: window [n, n+GAP), size GAP.
        // (gap_start = gap_end would be a size-0 gap: len() would inflate.)
        chars.resize(n + GAP, '\0');
        Buffer {
            name,
            filename: None,
            modified: false,
            chars,
            gap_start: n,
            gap_end: n + GAP,
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
        if self.gap_start == self.gap_end {
            self.refill_gap(); // gap exhausted: widen in place at this point
        }
        self.chars[self.gap_start] = c;
        self.gap_start += 1;
        self.point += 1;
        self.mark = self.point;
        self.modified = true;
    }

    // Insert s at point, advancing it (amortized O(len(s)) with the gap).
    pub fn insert_string(self: &mut Buffer, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    // Widen the (empty) gap window at gap_start by splicing GAP padding in
    // place, pushing the right segment back. A per-char Vec::insert loop
    // would be O(n*GAP); tail resize would not put the gap at this point.
    fn refill_gap(self: &mut Buffer) {
        let gs = self.gap_start;
        self.chars.splice(gs..gs, std::iter::repeat_n('\0', GAP));
        self.gap_end = gs + GAP;
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
            let mut target = self.point + 1;
            if self.mark > self.point && self.mark <= self.line_end(self.point) {
                target = self.mark; // the mark (region end) sits ahead on this line
            }
            if target < self.point {
                target = self.point;
            }
            self.point = target;
        }
    }

    pub fn backward_char(self: &mut Buffer) {
        if self.point > 0 {
            let mut target = self.point - 1;
            if self.mark < self.point && self.mark >= self.line_start(self.point) {
                target = self.mark; // the mark (region start) sits behind on this line
            }
            if target > self.point {
                target = self.point;
            }
            self.point = target;
        }
    }

    pub fn beginning_of_line(self: &mut Buffer) {
        let ls = self.line_start(self.point);
        let mut target = ls;
        if self.mark != self.point && self.mark >= ls && self.mark < self.point {
            target = self.mark; // region start inside this line
        }
        self.point = target;
    }

    pub fn end_of_line(self: &mut Buffer) {
        let le = self.line_end(self.point);
        let mut target = le;
        if self.mark != self.point && self.mark > self.point && self.mark <= le {
            target = self.mark; // region end inside this line
        } else if self.mark != self.point && self.mark < self.point && self.mark >= self.line_start(self.point) {
            target = self.point; // point already sits at the region end
        }
        self.point = target;
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
        if self.gap_start == self.gap_end {
            self.refill_gap();
        }
        // NB: the old Vec::insert branch forgot `gap_start += 1`, so the
        // inserted '\n' was swallowed by the gap window (silent C-o no-op
        // on a fresh buffer - Design Doc B / F-4). Consume the gap instead.
        self.chars[self.gap_start] = '\n';
        self.gap_start += 1;
        self.mark = self.point;
        self.modified = true;
    }

    pub fn swap_at(self: &mut Buffer, i: usize) {
        let a = self.to_index(i);
        let b = self.to_index(i + 1);
        self.chars.swap(a, b);
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

    pub fn kill_to_end_of_line(self: &mut Buffer) -> String {
        let ls = self.line_start(self.point);
        let le = self.line_end(self.point);
        let hi = if self.mark != self.point && self.mark >= ls && self.mark <= le {
            if self.mark > self.point { self.mark } else { self.point }
        } else {
            self.len()
        };
        if self.point < self.len() && self.char_at(self.point) == '\n' && self.point < hi {
            self.delete_forward();
            "\n".to_string()
        } else {
            let stop = if le < hi { le } else { hi };
            let n = stop.saturating_sub(self.point);
            let mut cut: Vec<char> = vec![];
            let mut i = 0;
            while i < n {
                cut.push(self.char_at(self.point));
                self.delete_forward();
                i += 1;
            }
            cut.iter().collect::<String>()
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

    // Scan with char_at (O(1) through the gap): no full text() copy per call
    // (B-2/B-4). Returns the match END position, as before.
    pub fn find_forward(self: &Buffer, pat: &str, from: usize) -> Option<usize> {
        let pchars: Vec<char> = pat.chars().collect();
        let m = pchars.len();
        if m == 0 || from + m > self.len() {
            return None;
        }
        let mut i = from;
        while i + m <= self.len() {
            let mut ok = true;
            let mut k = 0;
            while k < m {
                if self.char_at(i + k) != pchars[k] {
                    ok = false;
                    break;
                }
                k += 1;
            }
            if ok {
                return Some(i + m);
            }
            i += 1;
        }
        None
    }

    // Backward scan: returns the match START position, as before.
    pub fn find_backward(self: &Buffer, pat: &str, from: usize) -> Option<usize> {
        let pchars: Vec<char> = pat.chars().collect();
        let m = pchars.len();
        if m == 0 || m > self.len() || from == 0 {
            return None;
        }
        let mut i = from - 1;
        if i > self.len() - m {
            i = self.len() - m;
        }
        loop {
            let mut ok = true;
            let mut k = 0;
            while k < m {
                if self.char_at(i + k) != pchars[k] {
                    ok = false;
                    break;
                }
                k += 1;
            }
            if ok {
                return Some(i);
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
        None
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
fn region_probe() {
    let mut b = Buffer::new("t".to_string(), "one\ntwo22\nthree".to_string());
    assert!(b.mark == 0 && b.point == 0);
    b.next_line();
    assert!(b.point == 4);
    assert!(b.mark == 0);
    b.forward_char();
    assert!(b.point == 5);
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
fn gap_prealloc_identity() {
    // Design Doc B / B-1: the gap window is pre-allocated at the END with
    // SIZE GAP (gap_start = n, gap_end = n + GAP), so len() stays exact.
    let mut b = Buffer::new("t".to_string(), "hello".to_string());
    assert!(b.text() == "hello");
    assert!(b.len() == 5);
    assert!(b.chars.len() == 5 + GAP);
    assert!(b.gap_start == 5 && b.gap_end == 5 + GAP);
    b.point = 2;
    b.insert_char('X'); // slides the gap to the insertion point, then O(1)
    assert!(b.text() == "heXllo");
    assert!(b.len() == 6);
}
#[test]
fn gap_refill_amortized() {
    // Typing at the front slides the pre-allocated gap once, then inserts
    // are O(1); at exhaustion the gap is re-widened IN PLACE at the point.
    let mut b = Buffer::new("t".to_string(), "hello".to_string());
    for k in 0..(GAP + 3) {
        b.insert_char(char::from(b'a' + (k % 26) as u8));
        assert!(b.chars.len() - b.len() <= GAP); // never widens past GAP
    }
    assert!(b.len() == 5 + GAP + 3);
    assert!(b.text().starts_with("abc") && b.text().ends_with("hello"));
}
#[test]
fn c_o_fresh_and_refill_boundary() {
    // F-4: the old Vec::insert branch of insert_newline_after lacked
    // gap_start += 1, swallowing the '\n' into the gap (silent C-o no-op).
    let mut b = Buffer::new("t".to_string(), "one\ntwo".to_string());
    b.point = 3;
    b.insert_newline_after(); // fresh buffer: pre-allocated gap covers it
    assert!(b.text() == "one\n\ntwo");
    let mut c = Buffer::new("t".to_string(), "abc".to_string());
    c.end_of_buffer();
    for _ in 0..GAP {
        c.insert_char('x'); // consume the whole gap at the tail
    }
    assert!(c.gap_start == c.gap_end);
    c.insert_newline_after(); // refill boundary: must not be swallowed
    assert!(c.text().ends_with("\n"));
    assert!(c.len() == 3 + GAP + 1);
}
#[test]
fn find_matrix() {
    let b = Buffer::new("t".to_string(), "hello hello".to_string());
    assert!(b.find_forward(&"hello".to_string(), 0) == Some(5));
    assert!(b.find_forward(&"hello".to_string(), 1) == Some(11));
    assert!(b.find_forward(&"hello".to_string(), 7).is_none());
    assert!(b.find_forward(&"x".to_string(), 0).is_none());
    assert!(b.find_forward(&"".to_string(), 0).is_none());
    assert!(b.find_forward(&"hello".to_string(), 9).is_none()); // from + m > len
    assert!(b.find_backward(&"hello".to_string(), 11) == Some(6));
    assert!(b.find_backward(&"hello".to_string(), 5) == Some(0));
    assert!(b.find_backward(&"hello".to_string(), 0).is_none());
    assert!(b.find_backward(&"x".to_string(), 5).is_none());
    // the gap must not distort scans: edit, then search the edited text
    let mut m = Buffer::new("t".to_string(), "hello hello".to_string());
    m.point = 5;
    m.insert_char('!');
    assert!(m.find_forward(&"o! ".to_string(), 0) == Some(7));
    assert!(m.find_backward(&"! he".to_string(), 12) == Some(5));
}
#[test]
fn insert_string_advances_point() {
    let mut b = Buffer::new("t".to_string(), "ab".to_string());
    b.point = 1;
    b.insert_string("xy");
    assert!(b.text() == "axyb");
    assert!(b.point == 3);
    assert!(b.modified);
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
    b.point = 10;
    b.mark = b.point; // no active region: kill goes to end of line
    let cut = b.kill_to_end_of_line();
    assert!(cut == "world".to_string());
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
