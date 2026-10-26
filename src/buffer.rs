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
    // B-10: name/text accept anything String-shaped; callers pass &str
    // literals and owned Strings directly (no .to_string() ceremony).
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Buffer {
        let text: String = text.into();
        let mut chars = text.chars().collect::<Vec<char>>();
        let n = chars.len();
        // Reserve the gap at the END: window [n, n+GAP), size GAP.
        // (gap_start = gap_end would be a size-0 gap: len() would inflate.)
        chars.resize(n + GAP, '\0');
        Buffer {
            name: name.into(),
            filename: None,
            modified: false,
            chars,
            gap_start: n,
            gap_end: n + GAP,
            point: 0,
            mark: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.chars.len() - (self.gap_end - self.gap_start)
    }

    pub fn to_index(&self, pos: usize) -> usize {
        if pos < self.gap_start {
            pos
        } else {
            pos + (self.gap_end - self.gap_start)
        }
    }

    pub fn move_gap(&mut self, pos: usize) {
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

    pub fn insert_char(&mut self, c: char) {
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
    pub fn insert_string(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    // Widen the (empty) gap window at gap_start by splicing GAP padding in
    // place, pushing the right segment back. A per-char Vec::insert loop
    // would be O(n*GAP); tail resize would not put the gap at this point.
    fn refill_gap(&mut self) {
        let gs = self.gap_start;
        self.chars.splice(gs..gs, std::iter::repeat_n('\0', GAP));
        self.gap_end = gs + GAP;
    }

    pub fn delete_forward(&mut self) -> bool {
        self.move_gap(self.point);
        if self.gap_end < self.chars.len() {
            self.gap_end += 1;
            self.modified = true;
            true
        } else {
            false
        }
    }

    pub fn delete_backward(&mut self) -> bool {
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

    // C-w (Emacs kill-region): cut the text between mark and point.
    // Returns the cut string; point AND mark land at the region start.
    // An empty region (mark == point) cuts NOTHING (returns "").
    pub fn kill_region(&mut self) -> String {
        let lo = self.point.min(self.mark);
        let hi = self.point.max(self.mark).min(self.len()); // DEFENSIVE clamp
        if lo >= hi {
            return String::new();
        }
        self.move_gap(lo);
        // The gap window sits at lo; the logical chars [lo..hi) are the
        // physical chars right AFTER the gap - collect, then consume
        // them into the gap (the delete_forward mechanism, span-wide).
        let mut cut = String::new();
        let mut k = self.gap_end;
        while k < self.gap_end + (hi - lo) {
            cut.push(self.chars[k]);
            k += 1;
        }
        self.gap_end += hi - lo;
        self.point = lo;
        self.mark = lo;
        self.modified = true;
        cut
    }

    pub fn char_at(&self, pos: usize) -> char {
        self.chars[self.to_index(pos)]
    }

    pub fn line_start(&self, mut pos: usize) -> usize {
        while pos > 0 && self.char_at(pos - 1) != '\n' {
            pos -= 1;
        }
        pos
    }

    pub fn line_end(&self, mut pos: usize) -> usize {
        while pos < self.len() && self.char_at(pos) != '\n' {
            pos += 1;
        }
        pos
    }

    pub fn line_column(&self, pos: usize) -> usize {
        pos - self.line_start(pos)
    }

    pub fn forward_char(&mut self) {
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

    pub fn backward_char(&mut self) {
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

    pub fn beginning_of_line(&mut self) {
        let ls = self.line_start(self.point);
        let mut target = ls;
        if self.mark != self.point && self.mark >= ls && self.mark < self.point {
            target = self.mark; // region start inside this line
        }
        self.point = target;
    }

    pub fn end_of_line(&mut self) {
        let le = self.line_end(self.point);
        let mut target = le;
        if self.mark != self.point && self.mark > self.point && self.mark <= le {
            target = self.mark; // region end inside this line
        } else if self.mark != self.point && self.mark < self.point && self.mark >= self.line_start(self.point) {
            target = self.point; // point already sits at the region end
        }
        self.point = target;
    }

    pub fn next_line(&mut self) {
        let col = self.line_column(self.point);
        let lend = self.line_end(self.point);
        if lend < self.len() {
            let start = lend + 1;
            let tend = self.line_end(start);
            let max_col = tend - start;
            self.point = start + if col < max_col { col } else { max_col };
        }
    }

    pub fn previous_line(&mut self) {
        let ls = self.line_start(self.point);
        if ls > 0 {
            let col = self.line_column(self.point);
            let prev_end = ls - 1;
            let prev_start = self.line_start(prev_end);
            let max_col = prev_end - prev_start;
            self.point = prev_start + if col < max_col { col } else { max_col };
        }
    }

    pub fn beginning_of_buffer(&mut self) {
        self.point = 0;
    }

    pub fn end_of_buffer(&mut self) {
        self.point = self.len();
    }

    pub fn insert_newline_after(&mut self) {
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

    pub fn swap_at(&mut self, i: usize) {
        let a = self.to_index(i);
        let b = self.to_index(i + 1);
        self.chars.swap(a, b);
        self.modified = true;
    }

    pub fn transpose_chars(&mut self) {
        // B-5: Emacs C-t is a NO-OP at the buffer start (point 0): the
        // char AFTER point is not transposed with the one before it.
        if self.point > 0 {
            if self.point < self.len() {
                self.swap_at(self.point - 1);
                self.point += 1;
            } else if self.len() > 1 {
                self.swap_at(self.len() - 2);
            }
        }
    }

    pub fn kill_to_end_of_line(&mut self) -> String {
        let ls = self.line_start(self.point);
        let le = self.line_end(self.point);
        let hi = if self.mark > self.point && self.mark >= ls && self.mark <= le {
            self.mark // region edge AHEAD on this line: C-k stops there
        } else {
            // B-3: a mark BEHIND the point (or off-line, or ==point) is not
            // a kill boundary: kill to end of line. (Old code clamped hi to
            // point for a backward mark, making C-k a silent no-op.)
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

    pub fn forward_word(&mut self) {
        while self.point < self.len() && !is_word_char(self.char_at(self.point)) {
            self.point += 1;
        }
        while self.point < self.len() && is_word_char(self.char_at(self.point)) {
            self.point += 1;
        }
    }

    pub fn backward_word(&mut self) {
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
    pub fn find_forward(&self, pat: &str, from: usize) -> Option<usize> {
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
    pub fn find_backward(&self, pat: &str, from: usize) -> Option<usize> {
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

    pub fn text(&self) -> String {
        let mut out: Vec<char> = vec![];
        out.extend(self.chars[0..self.gap_start].iter());
        out.extend(self.chars[self.gap_end..self.chars.len()].iter());
        out.iter().collect::<String>()
    }

    // --- undo support (C-/; Design Doc D row 0x1f) -------------------------
    // Full-state snapshot for the dispatch loop's undo stack. The stack
    // lives in Ed (main.rs: grouping is a dispatch-level concern); the
    // buffer owns the snapshot/restore mechanics so the gap layout is
    // rebuilt in ONE place.
    pub fn snapshot(&self) -> UndoState {
        UndoState {
            text: self.text(),
            point: self.point,
            mark: self.mark,
        }
    }

    // Rebuild the whole buffer from a snapshot (same gap-at-end layout as
    // new()). point/mark clamp into the restored text. The buffer becomes
    // MODIFIED: an undo is itself a change relative to the on-disk file
    // (Emacs marks the buffer modified on undo; a save clears it).
    pub fn restore(&mut self, st: &UndoState) {
        let mut chars: Vec<char> = st.text.chars().collect();
        let n = chars.len();
        chars.resize(n + GAP, '\0');
        self.chars = chars;
        self.gap_start = n;
        self.gap_end = n + GAP;
        self.point = st.point.min(n);
        self.mark = st.mark.min(n);
        self.modified = true;
    }
}

// Undo snapshot: full visible state of a buffer (gap internals excluded -
// restore rebuilds them). Clone/PartialEq for the P0 differential test.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UndoState {
    pub text: String,
    pub point: usize,
    pub mark: usize,
}

#[cfg(test)]
mod tests {
use super::*;
#[test]
fn gap_buffer_roundtrip() {
    let mut b = Buffer::new("t", "hello");
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
    let mut b = Buffer::new("t", "one\ntwo22\nthree");
    assert!(b.mark == 0 && b.point == 0);
    b.next_line();
    assert!(b.point == 4);
    assert!(b.mark == 0);
    b.forward_char();
    assert!(b.point == 5);
}
#[test]
fn cursor_movement() {
    let mut b = Buffer::new("t", "one\ntwo22\nthree");
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
    let mut b = Buffer::new("t", "hello");
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
    let mut b = Buffer::new("t", "hello");
    for k in 0..(GAP + 3) {
        b.insert_char(char::from(b'a' + (k % 26) as u8));
        assert!(b.chars.len() - b.len() <= GAP); // never widens past GAP
    }
    assert!(b.len() == 5 + GAP + 3);
    assert!(b.text().starts_with("abc") && b.text().ends_with("hello"));
}
#[test]
fn kill_region_matrix() {
    // C-w: region cut; point/mark land at the region start (lo);
    // empty region cuts nothing.
    let mut b = Buffer::new("t", "hello world");
    b.point = 5;
    b.mark = 0; // mark BEHIND point
    assert!(b.kill_region() == "hello");
    assert!(b.text() == " world");
    assert!(b.point == 0 && b.mark == 0);
    let mut c = Buffer::new("t", "hello world");
    c.point = 0;
    c.mark = 5; // mark AHEAD of point: same region, same result
    assert!(c.kill_region() == "hello");
    assert!(c.text() == " world" && c.point == 0 && c.mark == 0);
    let mut d = Buffer::new("t", "abc");
    d.point = 2;
    d.mark = 2;
    assert!(d.kill_region().is_empty()); // empty region: NO cut, NO mark move
    assert!(d.text() == "abc" && d.point == 2 && d.mark == 2);
    let mut e = Buffer::new("t", "line one\nline two\n");
    e.point = 17;
    e.mark = 9; // across the newline
    assert!(e.kill_region() == "line two");
    assert!(e.text() == "line one\n\n" && e.point == 9 && e.mark == 9);
}

#[test]
fn c_o_fresh_and_refill_boundary() {
    // F-4: the old Vec::insert branch of insert_newline_after lacked
    // gap_start += 1, swallowing the '\n' into the gap (silent C-o no-op).
    let mut b = Buffer::new("t", "one\ntwo");
    b.point = 3;
    b.insert_newline_after(); // fresh buffer: pre-allocated gap covers it
    assert!(b.text() == "one\n\ntwo");
    let mut c = Buffer::new("t", "abc");
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
    let b = Buffer::new("t", "hello hello");
    assert!(b.find_forward("hello", 0) == Some(5));
    assert!(b.find_forward("hello", 1) == Some(11));
    assert!(b.find_forward("hello", 7).is_none());
    assert!(b.find_forward("x", 0).is_none());
    assert!(b.find_forward("", 0).is_none());
    assert!(b.find_forward("hello", 9).is_none()); // from + m > len
    assert!(b.find_backward("hello", 11) == Some(6));
    assert!(b.find_backward("hello", 5) == Some(0));
    assert!(b.find_backward("hello", 0).is_none());
    assert!(b.find_backward("x", 5).is_none());
    // the gap must not distort scans: edit, then search the edited text
    let mut m = Buffer::new("t", "hello hello");
    m.point = 5;
    m.insert_char('!');
    assert!(m.find_forward("o! ", 0) == Some(7));
    assert!(m.find_backward("! he", 12) == Some(5));
}
#[test]
fn insert_string_advances_point() {
    let mut b = Buffer::new("t", "ab");
    b.point = 1;
    b.insert_string("xy");
    assert!(b.text() == "axyb");
    assert!(b.point == 3);
    assert!(b.modified);
}
#[test]
fn editing_and_navigation() {
    let mut b = Buffer::new("t", "hello big world");
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
    assert!(cut == "world");
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
#[test]
fn kill_backward_mark_kills_to_eol() {
    // B-3: a mark BEHIND the point must not turn C-k into a no-op.
    let mut b = Buffer::new("t", "hello world\nx\n");
    b.point = 6; // 'w' of world; mark BEHIND on the line
    b.mark = 0;
    let cut = b.kill_to_end_of_line();
    assert!(cut == "world");
    assert!(b.text() == "hello \nx\n");
    // point on the newline with the backward mark: the newline is killed
    b.point = 6;
    let cut2 = b.kill_to_end_of_line();
    assert!(cut2 == "\n");
    assert!(b.text() == "hello x\n");
    // a forward mark still stops the kill at the region edge
    let mut c = Buffer::new("t", "abcdef\n");
    c.point = 1;
    c.mark = 3;
    assert!(c.kill_to_end_of_line() == "bc");
    assert!(c.text() == "adef\n");
}
#[test]
fn transpose_chars_noop_at_start() {
    // B-5: C-t at point 0 does NOTHING (Emacs); mid-buffer swaps + advances
    let mut b = Buffer::new("t", "ab");
    b.transpose_chars();
    assert!(b.text() == "ab");
    assert!(b.point == 0);
    b.point = 1;
    b.transpose_chars();
    assert!(b.text() == "ba");
    assert!(b.point == 2);
}
#[test]
fn movement_clamps_at_line_edges() {
    // B-9: line-boundary moves clamp, never run off the buffer
    let mut b = Buffer::new("t", "ab\nc\n"); // a0 b1 \n2 c3 \n4
    b.beginning_of_line();
    assert!(b.point == 0); // line 1 start: no-op
    b.end_of_line();
    assert!(b.point == 2); // sits ON the newline
    b.next_line();
    assert!(b.point == 4); // col 2 clamped to the SHORT line 2 end
    b.next_line();
    assert!(b.point == 5); // trailing empty line 3
    b.next_line();
    assert!(b.point == 5); // already on the last line: no move
    b.beginning_of_line();
    assert!(b.point == 5);
    b.previous_line();
    assert!(b.point == 3); // col 0 lands on the 'c' row
    b.previous_line();
    assert!(b.point == 0); // col 0 to line 1
}
#[test]
fn transpose_at_end_no_advance() {
    // B-9 boundary: point AT the buffer end swaps the last two, NO advance
    let mut b = Buffer::new("t", "ab");
    b.point = 2;
    b.transpose_chars();
    assert!(b.text() == "ba");
    assert!(b.point == 2);
}
#[test]
fn empty_buffer_edge_edits() {
    // B-9: empty buffer: deletes report false, kill yields "", C-t no-op
    let mut b = Buffer::new("t", "");
    assert!(!b.delete_forward());
    assert!(!b.delete_backward());
    assert!(b.kill_to_end_of_line() == "");
    assert!(b.text() == "");
    b.transpose_chars();
    assert!(b.point == 0);
}
#[test]
fn snapshot_restore_roundtrip() {
    // T-9: undo primitive: full state returns; gap layout survives the
    // rebuild (edits keep working after a restore)
    let mut b = Buffer::new("t", "alpha\nbeta");
    b.point = 4;
    b.mark = 2;
    let st = b.snapshot();
    b.insert_char('X');
    b.kill_to_end_of_line();
    b.insert_string("zz");
    assert!(b.text() != st.text);
    b.restore(&st);
    assert!(b.text() == "alpha\nbeta");
    assert!(b.point == 4);
    assert!(b.mark == 2);
    assert!(b.modified); // restore marks: an undo is a change vs the file
    b.insert_char('Q'); // gap layout rebuilt: editing works after restore
    assert!(b.text() == "alphQa\nbeta");
}
#[test]
fn restore_clamps_point_mark() {
    // T-9: defensive clamp in restore. A snapshot CANNOT carry positions
    // beyond its own text (snapshot() reads live fields; sync_in clamps
    // them per loop iteration), so the min() is defense-in-depth, not a
    // reachable path. Hand-built over-length states pin the clamp:
    let st = UndoState { text: "xy".to_string(), point: 9, mark: 7 };
    let mut b = Buffer::new("t", "abcdef");
    b.restore(&st);
    assert!(b.point == 2);
    assert!(b.mark == 2);
    assert!(b.text() == "xy");
    assert!(b.len() == 2);
}
#[test]
fn restore_empty_marks_modified() {
    // T-9: restore onto an unmodified buffer flips modified
    let mut b = Buffer::new("t", "abc");
    b.modified = false;
    let st = b.snapshot();
    b.delete_forward();
    b.restore(&st);
    assert!(!b.text().is_empty());
    assert!(b.modified);
}
}
