// P0 - gap-buffer differential model test (in-process, NO tmux).
// Design: ../../test-design/pbt-design/03-properties.md §P0.
//
// The production gap buffer is compiled verbatim (design gate MA-2:
// per-target include, and this file uses EVERY pub API item, keeping the
// include gate-clean without #[allow]). Every op is mirrored on a naive
// Vec<char> model, and the full state is compared after EVERY op.

use proptest::prelude::*;

#[path = "../../src/buffer.rs"]
mod buffer;

use buffer::{is_word_char, Buffer, UndoState};

// Naive text model: plain Vec<char>, same point/mark contracts.
struct Ref {
    s: Vec<char>,
    point: usize,
    mark: usize,
    modified: bool,
}

impl Ref {
    fn len(&self) -> usize {
        self.s.len()
    }
    fn line_start(&self, mut pos: usize) -> usize {
        while pos > 0 && self.s[pos - 1] != '\n' {
            pos -= 1;
        }
        pos
    }
    fn line_end(&self, mut pos: usize) -> usize {
        while pos < self.len() && self.s[pos] != '\n' {
            pos += 1;
        }
        pos
    }
    fn line_column(&self, pos: usize) -> usize {
        pos - self.line_start(pos)
    }

    // --- mirrors of buffer.rs (algorithm-for-algorithm) ---

    fn insert_char(&mut self, c: char) {
        self.s.insert(self.point, c);
        self.point += 1;
        self.mark = self.point;
        self.modified = true;
    }
    fn delete_forward(&mut self) {
        if self.point < self.len() {
            self.s.remove(self.point);
            self.modified = true;
        }
    }
    fn delete_backward(&mut self) {
        if self.point > 0 {
            self.s.remove(self.point - 1);
            self.point -= 1;
            self.mark = self.point;
            self.modified = true;
        }
    }
    fn forward_char(&mut self) {
        if self.point < self.len() {
            let mut target = self.point + 1;
            if self.mark > self.point && self.mark <= self.line_end(self.point) {
                target = self.mark;
            }
            if target < self.point {
                target = self.point;
            }
            self.point = target;
        }
    }
    fn backward_char(&mut self) {
        if self.point > 0 {
            let mut target = self.point - 1;
            if self.mark < self.point && self.mark >= self.line_start(self.point) {
                target = self.mark;
            }
            if target > self.point {
                target = self.point;
            }
            self.point = target;
        }
    }
    fn beginning_of_line(&mut self) {
        let ls = self.line_start(self.point);
        let mut target = ls;
        if self.mark != self.point && self.mark >= ls && self.mark < self.point {
            target = self.mark;
        }
        self.point = target;
    }
    fn end_of_line(&mut self) {
        let le = self.line_end(self.point);
        let mut target = le;
        if self.mark != self.point && self.mark > self.point && self.mark <= le {
            target = self.mark;
        } else if self.mark != self.point && self.mark < self.point
            && self.mark >= self.line_start(self.point)
        {
            target = self.point;
        }
        self.point = target;
    }
    fn next_line(&mut self) {
        let col = self.line_column(self.point);
        let lend = self.line_end(self.point);
        if lend < self.len() {
            let start = lend + 1;
            let tend = self.line_end(start);
            let max_col = tend - start;
            self.point = start + if col < max_col { col } else { max_col };
        }
    }
    fn previous_line(&mut self) {
        let ls = self.line_start(self.point);
        if ls > 0 {
            let col = self.line_column(self.point);
            let prev_end = ls - 1;
            let prev_start = self.line_start(prev_end);
            let max_col = prev_end - prev_start;
            self.point = prev_start + if col < max_col { col } else { max_col };
        }
    }
    fn insert_newline_after(&mut self) {
        self.s.insert(self.point, '\n');
        self.mark = self.point; // point STAYS (mirrors buffer.rs)
        self.modified = true;
    }
    fn swap_at(&mut self, i: usize) {
        if i + 1 < self.len() {
            self.s.swap(i, i + 1);
            self.modified = true;
        }
    }
    fn transpose_chars(&mut self) {
        if self.point > 0 {
            if self.point < self.len() {
                self.s.swap(self.point - 1, self.point);
                self.point += 1;
                self.modified = true;
            } else if self.len() > 1 {
                let l = self.len();
                self.s.swap(l - 2, l - 1);
                self.modified = true;
            }
        }
    }
    fn kill_to_end_of_line(&mut self) -> String {
        let ls = self.line_start(self.point);
        let le = self.line_end(self.point);
        let hi = if self.mark > self.point && self.mark >= ls && self.mark <= le {
            self.mark
        } else {
            self.len()
        };
        if self.point < self.len() && self.s[self.point] == '\n' && self.point < hi {
            self.s.remove(self.point);
            self.modified = true;
            "\n".to_string()
        } else {
            let stop = if le < hi { le } else { hi };
            let n = stop.saturating_sub(self.point);
            let mut cut: Vec<char> = vec![];
            for _ in 0..n {
                cut.push(self.s[self.point]);
                self.s.remove(self.point);
            }
            if n > 0 {
                self.modified = true;
            }
            cut.iter().collect::<String>()
        }
    }
    fn forward_word(&mut self) {
        while self.point < self.len() && !is_word_char(self.s[self.point]) {
            self.point += 1;
        }
        while self.point < self.len() && is_word_char(self.s[self.point]) {
            self.point += 1;
        }
    }
    fn backward_word(&mut self) {
        if self.point > 0 && is_word_char(self.s[self.point - 1]) {
            while self.point > 0 && is_word_char(self.s[self.point - 1]) {
                self.point -= 1;
            }
        } else {
            while self.point > 0 && !is_word_char(self.s[self.point - 1]) {
                self.point -= 1;
            }
            while self.point > 0 && is_word_char(self.s[self.point - 1]) {
                self.point -= 1;
            }
        }
    }
    // find mirrors (03 §P0-6): forward returns match END, backward START.
    fn find_forward(&self, pat: &[char], from: usize) -> Option<usize> {
        let m = pat.len();
        if m == 0 || from + m > self.len() {
            return None;
        }
        let mut i = from;
        while i + m <= self.len() {
            if &self.s[i..i + m] == pat {
                return Some(i + m);
            }
            i += 1;
        }
        None
    }
    fn find_backward(&self, pat: &[char], from: usize) -> Option<usize> {
        let m = pat.len();
        if m == 0 || m > self.len() || from == 0 {
            return None;
        }
        let mut i = from - 1;
        if i > self.len() - m {
            i = self.len() - m;
        }
        loop {
            if &self.s[i..i + m] == pat {
                return Some(i);
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
        None
    }
    fn text(&self) -> String {
        self.s.iter().collect::<String>()
    }
    // restore mirror (T-9 C-/): full rebuild, clamped positions, marked
    fn restore(&mut self, st: &UndoState) {
        self.s = st.text.chars().collect();
        let n = self.s.len();
        self.point = st.point.min(n);
        self.mark = st.mark.min(n);
        self.modified = true;
    }
}


// ---- op set (EVERY pub API item of buffer.rs is reachable from here,
// gate MA-2), and the full-state sweep after every op (03 §P0) ----

#[derive(Debug, Clone)]
enum Op {
    Ins(char),
    InsStr(Vec<char>),
    DelF,
    DelB,
    Fwd,
    Bwd,
    Bol,
    Eol,
    Next,
    Prev,
    Bob,
    Eob,
    Open,
    Swap(usize),
    Transpose,
    Kill,
    FwdWord,
    BwdWord,
    Mark,
    FindF(String, usize),
    FindB(String, usize),
    MoveGap(usize),
    // T-9 undo pair: snapshot() fidelity check, then restore() of a
    // stored pair (LIFO order is NOT enforced: restore() takes ANY
    // state, and the full-state sweep verifies b == r after it).
    Snap,
    Restore(usize),
}

#[derive(Default)]
struct Ret {
    cut: Option<(String, String)>,           // buffer vs ref kill cuts
    find: Option<(Option<usize>, Option<usize>)>, // buffer vs ref find result
    snap: Option<(UndoState, UndoState)>,    // snapshot() vs ref-built state
}

fn apply(b: &mut Buffer, r: &mut Ref, op: &Op, snaps: &mut Vec<(UndoState, UndoState)>) -> Ret {
    let mut ret = Ret::default();
    match op {
        Op::Ins(c) => {
            b.insert_char(*c);
            r.insert_char(*c);
        }
        Op::InsStr(v) => {
            let s: String = v.iter().collect();
            b.insert_string(&s);
            for &c in v {
                r.insert_char(c);
            }
        }
        Op::DelF => {
            b.delete_forward();
            r.delete_forward();
        }
        Op::DelB => {
            b.delete_backward();
            r.delete_backward();
        }
        Op::Fwd => {
            b.forward_char();
            r.forward_char();
        }
        Op::Bwd => {
            b.backward_char();
            r.backward_char();
        }
        Op::Bol => {
            b.beginning_of_line();
            r.beginning_of_line();
        }
        Op::Eol => {
            b.end_of_line();
            r.end_of_line();
        }
        Op::Next => {
            b.next_line();
            r.next_line();
        }
        Op::Prev => {
            b.previous_line();
            r.previous_line();
        }
        Op::Bob => {
            b.beginning_of_buffer();
            r.point = 0;
        }
        Op::Eob => {
            b.end_of_buffer();
            r.point = r.len();
        }
        Op::Open => {
            b.insert_newline_after();
            r.insert_newline_after();
        }
        Op::Swap(i) => {
            // guard evaluated once (b.len() == r.len() by the invariants)
            if i + 1 < b.len() {
                b.swap_at(*i);
                r.swap_at(*i);
            }
        }
        Op::Transpose => {
            b.transpose_chars();
            r.transpose_chars();
        }
        Op::Kill => {
            let kb = b.kill_to_end_of_line();
            let kr = r.kill_to_end_of_line();
            ret.cut = Some((kb, kr));
        }
        Op::FwdWord => {
            b.forward_word();
            r.forward_word();
        }
        Op::BwdWord => {
            b.backward_word();
            r.backward_word();
        }
        Op::Mark => {
            // mirrors C-SPC (main.rs set_mark)
            b.mark = b.point;
            r.mark = r.point;
        }
        Op::FindF(pat, from) => {
            let vb = b.find_forward(pat, *from);
            let vr = r.find_forward(&pat.chars().collect::<Vec<char>>(), *from);
            ret.find = Some((vb, vr));
        }
        Op::FindB(pat, from) => {
            let vb = b.find_backward(pat, *from);
            let vr = r.find_backward(&pat.chars().collect::<Vec<char>>(), *from);
            ret.find = Some((vb, vr));
        }
        Op::MoveGap(pos) => {
            // direct pub-API exercise; clamped to the logical range (the
            // internal callers always pass point <= len, same contract)
            let p = *pos;
            let p = if p > b.len() { b.len() } else { p };
            b.move_gap(p);
        }
        Op::Snap => {
            // store the pair, and report the buffer snapshot vs a
            // ref-built state (snapshot() fidelity: text/point/mark
            // read through the gap layout must equal the model's)
            let sb = b.snapshot();
            let sr = UndoState { text: r.text(), point: r.point, mark: r.mark };
            snaps.push((sb.clone(), sr.clone()));
            ret.snap = Some((sb, sr));
        }
        Op::Restore(idx) => {
            if !snaps.is_empty() {
                let (sb, sr) = snaps.remove(*idx % snaps.len());
                b.restore(&sb);
                r.restore(&sr);
            }
        }
    }
    ret
}

// Full-state sweep (03 §P0 invariants 1-6).
fn check(b: &Buffer, r: &Ref) -> Option<String> {
    if b.len() != r.len() {
        return Some(format!("len {} vs {}", b.len(), r.len()));
    }
    if b.len() != b.chars.len() - (b.gap_end - b.gap_start) {
        return Some("len() identity broken".to_string());
    }
    if !(b.gap_start <= b.gap_end && b.gap_end <= b.chars.len()) {
        return Some(format!(
            "gap window ill-formed {}/{} of {}",
            b.gap_start, b.gap_end, b.chars.len()
        ));
    }
    if b.text() != r.text() {
        return Some(format!("text differs: {:?} vs {:?}", b.text(), r.text()));
    }
    if b.point != r.point {
        return Some(format!("point {} vs {}", b.point, r.point));
    }
    if b.mark != r.mark {
        return Some(format!("mark {} vs {}", b.mark, r.mark));
    }
    if b.modified != r.modified {
        return Some(format!("modified {} vs {}", b.modified, r.modified));
    }
    for i in 0..b.len() {
        if b.char_at(i) != r.s[i] {
            return Some(format!("char_at[{}]: {:?} vs {:?}", i, b.char_at(i), r.s[i]));
        }
        let ix = b.to_index(i);
        if ix >= b.chars.len() || b.chars[ix] != r.s[i] {
            return Some(format!("to_index[{}]={} exposes wrong cell", i, ix));
        }
    }
    for p in 0..=b.len() {
        if b.line_start(p) != r.line_start(p) {
            return Some(format!("line_start[{}] {} vs {}", p, b.line_start(p), r.line_start(p)));
        }
        if b.line_end(p) != r.line_end(p) {
            return Some(format!("line_end[{}] {} vs {}", p, b.line_end(p), r.line_end(p)));
        }
        if b.line_column(p) != r.line_column(p) {
            return Some(format!("line_column[{}]", p));
        }
    }
    None
}

// Intentional small duplication of support::cases() (support/mod.rs):
// the P0 binary is tmux-free and deliberately does NOT include the
// support module (every pub item there is tmux-scoped; including it
// here would dead_code-warn under the zero-warning gate).
fn crate_cases(default: u32) -> u32 {
    std::env::var("PBT_CASES")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn p0_cfg() -> proptest::test_runner::Config {
    let mut c = proptest::test_runner::Config::with_cases(crate_cases(512));
    c.max_shrink_time = 15_000;
    c.failure_persistence = None;
    // timeout NOT set: P0 is in-process; no fork mode, no tmux.
    c
}

fn char_pool() -> impl Strategy<Value = char> {
    prop::sample::select(vec!['a', 'b', '\n', ' ', 'A', 'z', '0', '.', '-', '/', '\u{7}', 'あ', '字', 'e'])
}

fn pat_pool() -> impl Strategy<Value = String> {
    prop::sample::select(vec!["a", "ab", "ba", "z", "aa", "\n", " ", "あ", "abc", ""])
        .prop_map(|s: &str| s.to_string())
}

fn arb_ops() -> impl Strategy<Value = Vec<Op>> {
    let one = prop_oneof![
        12 => char_pool().prop_map(Op::Ins),
        4 => prop::collection::vec(char_pool(), 0..8usize).prop_map(Op::InsStr),
        6 => Just(Op::DelF),
        6 => Just(Op::DelB),
        4 => Just(Op::Fwd),
        4 => Just(Op::Bwd),
        3 => Just(Op::Bol),
        3 => Just(Op::Eol),
        3 => Just(Op::Next),
        3 => Just(Op::Prev),
        2 => Just(Op::Bob),
        2 => Just(Op::Eob),
        2 => Just(Op::Open),
        2 => (0usize..32).prop_map(Op::Swap),
        2 => Just(Op::Transpose),
        6 => Just(Op::Kill),
        3 => Just(Op::FwdWord),
        3 => Just(Op::BwdWord),
        3 => Just(Op::Mark),
        3 => (pat_pool(), 0usize..40).prop_map(|(p, f)| Op::FindF(p, f)),
        3 => (pat_pool(), 0usize..40).prop_map(|(p, f)| Op::FindB(p, f)),
        2 => (0usize..40).prop_map(Op::MoveGap),
        2 => Just(Op::Snap),
        3 => (0usize..12).prop_map(Op::Restore),
    ];
    prop::collection::vec(one, 1..=200usize)
}

proptest! {
    #![proptest_config(p0_cfg())]

    #[test]
    fn p0_gap_buffer_matches_naive(
        seed in prop::collection::vec(char_pool(), 0..30usize),
        ops in arb_ops(),
    ) {
        let seed_s: String = seed.iter().collect();
        let mut b = Buffer::new("t", seed_s.clone());
        let mut r = Ref { s: seed.clone(), point: 0, mark: 0, modified: false };
        // startup field reads (gate MA-2 field reachability)
        prop_assert_eq!(&b.name, "t");
        prop_assert!(b.filename.is_none());
        prop_assert!(!b.modified);
        prop_assert_eq!(b.text(), seed_s);

        let mut snaps: Vec<(UndoState, UndoState)> = vec![];
        for (i, op) in ops.iter().enumerate() {
            let ret = apply(&mut b, &mut r, op, &mut snaps);
            if let Some((kb, kr)) = ret.cut {
                prop_assert_eq!(kb, kr, "op{} {:?}: kill cut", i, op);
            }
            if let Some((fb, fr)) = ret.find {
                prop_assert_eq!(fb, fr, "op{} {:?}: find result", i, op);
            }
            if let Some((sb, sr)) = ret.snap {
                prop_assert_eq!(sb, sr, "op{} {:?}: snapshot fidelity", i, op);
            }
            let fail = check(&b, &r);
            prop_assert!(fail.is_none(), "op{} {:?}: {}", i, op, fail.unwrap_or_default());
        }
    }
}
