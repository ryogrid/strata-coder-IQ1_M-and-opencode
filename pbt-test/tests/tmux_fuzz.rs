// P1-P4: tmux-driven fuzz properties.
// Design: ../../test-design/pbt-design/03-properties.md, 04-tmux-harness.md.
// One binary, run with --test-threads=1 (serial law). proptest FORK mode
// is DISABLED (support::cfg timeout=0, PBTF-002): cases run in-process,
// Drop guards clean sessions, leftovers are swept (support::sweep_
// leftovers); a hung case is bounded by the CI job timeout (05).
//
// The P2 model replica compiles the production buffer.rs per-target
// (design gate MA-2: the Sim below + the line-count helpers read EVERY
// pub API item of buffer.rs, keeping the include gate-clean).

mod support;

#[path = "../../src/buffer.rs"]
mod buffer;

use buffer::Buffer;
use proptest::prelude::*;
use support::cfg;
use support::pane_sane;
use support::pane_sane_lax;
use support::pane_tail;
use support::parse_status;
use support::settle;
use support::Session;
use support::PANE_W;
use support::SETTLE_BURST_MS;
use support::SETTLE_CHECK_MS;
use support::SETTLE_PHASE_MS;

static FILESEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn tmp_paths(tag: &str) -> (String, String) {
    let id = FILESEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let stem = format!("/tmp/emur-pbt-{}-{}-{}", std::process::id(), tag, id);
    (format!("{}-seed.txt", stem), format!("{}-out.txt", stem))
}

fn ascii_printable() -> Vec<char> {
    (0x21u8..=0x7e).map(|b| b as char).collect()
}

fn cjk_pool() -> Vec<char> {
    vec!['\u{3042}', '\u{3044}', '\u{6f22}', '\u{5b57}', '\u{00e9}', '\u{00fc}']
}

// =====================================================================
// P1 - raw byte fuzz, crash-free (03 §P1). 0x03 excluded: quit-confirm
// can end the session legally (liveness law); e2e s111/s140 pin it.
// =====================================================================

#[derive(Debug, Clone)]
enum P1Op {
    Ctrl(u8),
    Char(char),
    Text(String),
    Cx(u8),
    Meta(u8),
    Arrow(u8),
}

fn bytes_excluding_03() -> Vec<u8> {
    (0u8..=255).filter(|b| *b != 0x03).collect()
}

fn p1_ops() -> impl Strategy<Value = Vec<P1Op>> {
    let ctrl: Vec<u8> = (0u8..=0x1f).filter(|b| *b != 0x03).chain([0x7fu8]).collect();
    let one = prop_oneof![
        22 => prop::sample::select(ctrl).prop_map(P1Op::Ctrl),
        20 => prop::sample::select(ascii_printable()).prop_map(P1Op::Char),
        6 => prop::collection::vec(prop::sample::select(cjk_pool()), 1..4)
            .prop_map(|v| P1Op::Text(v.into_iter().collect())),
        8 => prop::sample::select(bytes_excluding_03()).prop_map(P1Op::Cx),
        6 => prop::sample::select(bytes_excluding_03()).prop_map(P1Op::Meta),
        3 => prop::sample::select(vec![0x41u8, 0x42, 0x43, 0x44, 0x45, 0x5a])
            .prop_map(P1Op::Arrow),
    ];
    prop::collection::vec(one, 1..40usize)
}

fn send_p1(s: &Session, op: &P1Op) {
    match op {
        P1Op::Ctrl(b) => s.send_hex(&[*b]),
        P1Op::Char(c) => s.send_text(&c.to_string()),
        P1Op::Text(t) => s.send_text(t),
        P1Op::Cx(b) => s.send_hex(&[0x18, *b]),
        P1Op::Meta(b) => s.send_hex(&[0x1b, *b]),
        P1Op::Arrow(t) => s.send_hex(&[0x1b, 0x5b, *t]),
    }
}

proptest! {
    #![proptest_config(cfg(24))]

    #[test]
    fn p1_raw_byte_fuzz(ops in p1_ops()) {
        let s = Session::start(&[]);
        let mut log = String::new();
        for op in ops.iter() {
            send_p1(&s, op);
            log.push_str(&format!("{:?} ", op));
        }
        // C-g suffix flushes every prompt state (04 §3). FOUR bytes: a
        // multi-byte lead byte (0xF0..0xF7, e.g. arriving mid-isearch via
        // read_char_eof) legally consumes up to 3 continuation bytes, so
        // the flush needs 3 consumed + 1 C-g that the prompt actually
        // sees (PBTF-007). The extra bytes also cover a pending C-x
        // prefix (first C-g lands as the prefix tail = Unbound, second
        // top-level = Cancel).
        s.send_hex(&[0x07, 0x07, 0x07, 0x07]);
        settle(SETTLE_BURST_MS);
        let pane = s.capture();
        let tail = pane_tail(&pane);
        prop_assert!(!pane.contains("EMU_EXITED"), "ops[{}] tail[{}]", log, tail);
        prop_assert!(!pane.contains("panicked"), "ops[{}] tail[{}]", log, tail);
        let raw = s.capture_bytes();
        prop_assert!(!raw.contains(&0x1b), "ops[{}] escape litter", log);
        // status exists on the last rows (a long msg may WRAP at 80 cols:
        // join the last two pane lines; parse_status inspects the tail)
        let lines: Vec<&str> = pane.lines().collect();
        let joined = if lines.len() >= 2 {
            format!("{} {}", lines[lines.len() - 2], lines[lines.len() - 1])
        } else {
            lines.last().copied().unwrap_or("").to_string()
        };
        prop_assert!(parse_status(joined.trim()), "ops[{}] tail[{}]", log, tail);
        for l in pane.lines() {
            prop_assert!(l.chars().count() <= PANE_W, "ops[{}] width line[{}]", log, l);
        }
    }
}

// =====================================================================
// P2 - edit roundtrip vs model replica (03 §P2)
// =====================================================================

#[derive(Debug, Clone)]
enum P2Op {
    Char(char),
    Enter,
    Bs,
    Cd,
    Ck,
    Cy,
    Ca,
    Ce,
    Cb,
    Cf,
    Cn,
    Cp,
    Co,
    Ct,
    Cspc,
    Mb,
    Mf,
    Mlt,
    Mgt,
    Arr(u8),
    Undo,
}

fn send_p2(s: &Session, op: &P2Op) {
    match op {
        P2Op::Char(c) => s.send_text(&c.to_string()),
        P2Op::Enter => s.send_hex(&[0x0d]),
        P2Op::Bs => s.send_hex(&[0x7f]),
        P2Op::Cd => s.send_hex(&[0x04]),
        P2Op::Ck => s.send_hex(&[0x0b]),
        P2Op::Cy => s.send_hex(&[0x19]),
        P2Op::Ca => s.send_hex(&[0x01]),
        P2Op::Ce => s.send_hex(&[0x05]),
        P2Op::Cb => s.send_hex(&[0x02]),
        P2Op::Cf => s.send_hex(&[0x06]),
        P2Op::Cn => s.send_hex(&[0x0e]),
        P2Op::Cp => s.send_hex(&[0x10]),
        P2Op::Co => s.send_hex(&[0x0f]),
        P2Op::Ct => s.send_hex(&[0x14]),
        P2Op::Cspc => s.send_hex(&[0x00]),
        P2Op::Mb => s.send_hex(&[0x1b, 0x62]),
        P2Op::Mf => s.send_hex(&[0x1b, 0x66]),
        P2Op::Mlt => s.send_hex(&[0x1b, 0x3c]),
        P2Op::Mgt => s.send_hex(&[0x1b, 0x3e]),
        P2Op::Arr(t) => s.send_hex(&[0x1b, 0x5b, *t]),
        P2Op::Undo => s.send_hex(&[0x1f]),
    }
}

// The dispatch-level mirror of main.rs (03 §P2 table): EVERY op maps to
// the documented Buffer method; kill-run accumulation uses the
// top-level lead byte (only 0x0b extends a run).
struct Sim {
    buf: Buffer,
    killed: String,
    prev: u8,
    // T-9: mirrors Ed's undo stack + run grouping (main.rs). bi is
    // always 0 here (single buffer), kept for the production shape.
    undo: Vec<SimEntry>,
    group_alive: bool,
    touched: bool,
}

struct SimEntry {
    bi: usize,
    tag: u8,
    st: buffer::UndoState,
}

fn lead_byte(c: char) -> u8 {
    if c.is_ascii() {
        c as u8
    } else {
        0xc3 // UTF-8 lead of the CJK/accent pools (never 0x0b, as in the editor)
    }
}

impl Sim {
    fn op(&mut self, op: &P2Op) {
        // The editor runs sync_in() at the top of EVERY loop iteration,
        // clamping the leaf's point AND MARK to the buffer length
        // (main.rs sync_in, writes both ways). After any op that shrinks
        // len (C-d/BS/C-k), a mark ahead of the new end is clamped down -
        // the P0/PBTF-006 finding: a model that skips this diverges in
        // forward_char's mark-ahead jump.
        let l = self.buf.len();
        if self.buf.point > l {
            self.buf.point = l;
        }
        if self.buf.mark > l {
            self.buf.mark = l;
        }
        // loop top (T-9): a command that pushes re-arms the change-run
        self.touched = false;
        self.dispatch(op);
        // loop bottom (T-9): any command that did NOT push breaks the run
        if !self.touched {
            self.group_alive = false;
        }
    }

    // mirrors Ed::push_entry (grouping: 1 = typing run, 2 = kill run)
    fn push_entry(&mut self, bi: usize, tag: u8, st: &buffer::UndoState) {
        let merge = tag != 0
            && self.group_alive
            && self.undo.last().is_some_and(|e| e.tag == tag && e.bi == bi);
        self.touched = true;
        self.group_alive = true;
        if merge {
            return;
        }
        self.undo.push(SimEntry { bi, tag, st: st.clone() });
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
    }

    // a change op: snapshot, apply, push ONLY when the text changed
    // (mirrors the production handlers' no-op guards)
    fn change(&mut self, tag: u8, f: impl FnOnce(&mut Sim) -> bool) {
        let pre = self.buf.snapshot();
        let applied = f(self);
        if applied && self.buf.text() != pre.text {
            self.push_entry(0, tag, &pre);
        }
    }

    fn dispatch(&mut self, op: &P2Op) {
        match op {
            P2Op::Char(c) => {
                let c = *c;
                self.change(1, |sim| {
                    sim.buf.insert_char(c);
                    true
                });
                self.prev = lead_byte(c);
            }
            P2Op::Enter => {
                self.change(1, |sim| {
                    sim.buf.insert_char('\n');
                    true // newline always inserts
                });
                self.prev = 0x0d;
            }
            P2Op::Bs => {
                self.change(0, |sim| sim.buf.delete_backward());
                self.prev = 0x7f;
            }
            P2Op::Cd => {
                self.change(0, |sim| sim.buf.delete_forward());
                self.prev = 0x04;
            }
            P2Op::Ck => {
                self.change(2, |sim| {
                    let cut = sim.buf.kill_to_end_of_line();
                    if sim.prev == 0x0b {
                        sim.killed.push_str(&cut);
                    } else {
                        sim.killed = cut;
                    }
                    true // the text guard in change() covers the no-op
                });
                self.prev = 0x0b;
            }
            P2Op::Cy => {
                self.change(0, |sim| {
                    let k = sim.killed.clone();
                    sim.buf.insert_string(&k);
                    !k.is_empty() // empty kill-ring: inserts nothing
                });
                self.prev = 0x19;
            }
            P2Op::Ca => {
                self.buf.beginning_of_line();
                self.prev = 0x01;
            }
            P2Op::Ce => {
                self.buf.end_of_line();
                self.prev = 0x05;
            }
            P2Op::Cb => {
                self.buf.backward_char();
                self.prev = 0x02;
            }
            P2Op::Cf => {
                self.buf.forward_char();
                self.prev = 0x06;
            }
            P2Op::Cn => {
                self.buf.next_line();
                self.prev = 0x0e;
            }
            P2Op::Cp => {
                self.buf.previous_line();
                self.prev = 0x10;
            }
            P2Op::Co => {
                self.change(0, |sim| {
                    sim.buf.insert_newline_after();
                    true // C-o always inserts
                });
                self.prev = 0x0f;
            }
            P2Op::Ct => {
                self.change(0, |sim| {
                    sim.buf.transpose_chars();
                    true
                });
                self.prev = 0x14;
            }
            P2Op::Cspc => {
                self.buf.mark = self.buf.point;
                self.prev = 0x00;
            }
            P2Op::Mb => {
                self.buf.backward_word();
                self.prev = 0x1b;
            }
            P2Op::Mf => {
                self.buf.forward_word();
                self.prev = 0x1b;
            }
            P2Op::Mlt => {
                self.buf.beginning_of_buffer();
                self.prev = 0x1b;
            }
            P2Op::Mgt => {
                self.buf.end_of_buffer();
                self.prev = 0x1b;
            }
            P2Op::Arr(t) => {
                match t {
                    0x41 => self.buf.previous_line(),
                    0x42 => self.buf.next_line(),
                    0x43 => self.buf.forward_char(),
                    _ => self.buf.backward_char(),
                }
                self.prev = 0x1b;
            }
            P2Op::Undo => {
                // mirrors Ed::undo: pop + restore (no msg in the model)
                self.group_alive = false;
                if let Some(e) = self.undo.pop() {
                    self.buf.restore(&e.st);
                }
                self.prev = 0x1f;
            }
        }
    }
}

// Line counts computed through the find API (its consumers here keep the
// pub include reachable - gate MA-2); mirrors screen.rs st_line/st_total.
fn line_of(buf: &Buffer, point: usize) -> usize {
    let mut n = 1usize;
    let mut i = 0usize;
    while let Some(end) = buf.find_forward("\n", i) {
        if end > point {
            break;
        }
        n += 1;
        i = end;
    }
    n
}

fn lines_total(buf: &Buffer) -> usize {
    let len = buf.len();
    if len == 0 {
        return 1;
    }
    let mut n = 1usize;
    let mut p = len;
    while let Some(start) = buf.find_backward("\n", p) {
        n += 1;
        p = start;
    }
    n
}

fn seed_chars() -> impl Strategy<Value = char> {
    prop_oneof![
        8 => prop::sample::select(ascii_printable()),
        2 => Just(' '),
        2 => prop::sample::select(cjk_pool()),
    ]
}

fn p2_ops() -> impl Strategy<Value = Vec<P2Op>> {
    let one = prop_oneof![
        12 => seed_chars().prop_map(P2Op::Char),
        4 => Just(P2Op::Enter),
        4 => Just(P2Op::Bs),
        2 => Just(P2Op::Cd),
        3 => Just(P2Op::Ck),
        2 => Just(P2Op::Cy),
        2 => Just(P2Op::Ca),
        2 => Just(P2Op::Ce),
        2 => Just(P2Op::Cb),
        2 => Just(P2Op::Cf),
        2 => Just(P2Op::Cn),
        2 => Just(P2Op::Cp),
        1 => Just(P2Op::Co),
        1 => Just(P2Op::Ct),
        1 => Just(P2Op::Cspc),
        2 => Just(P2Op::Mb),
        2 => Just(P2Op::Mf),
        1 => Just(P2Op::Mlt),
        1 => Just(P2Op::Mgt),
        2 => prop::sample::select(vec![0x41u8, 0x42, 0x43, 0x44]).prop_map(P2Op::Arr),
        3 => Just(P2Op::Undo),
    ];
    prop::collection::vec(one, 1..30usize)
}

fn seed_text() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::collection::vec(seed_chars(), 1..20usize), 1..4usize)
        .prop_map(|lines| {
            lines
                .into_iter()
                .map(|l| l.into_iter().collect::<String>())
                .collect::<Vec<String>>()
                .join("\n")
        })
}

proptest! {
    #![proptest_config(cfg(24))]

    #[test]
    fn p2_edit_roundtrip(seed in seed_text(), ops in p2_ops()) {
        let (seed_path, out_path) = tmp_paths("p2");
        std::fs::write(&seed_path, &seed).expect("seed write");
        let s = Session::start(&[seed_path.as_str()]);
        let mut sim = Sim {
            buf: Buffer::new(seed_path.clone(), seed.clone()),
            killed: String::new(),
            prev: 0,
            undo: vec![],
            group_alive: false,
            touched: false,
        };
        prop_assert_eq!(&sim.buf.name, &seed_path);
        // Model self-check (gate MA-2 field reachability). NOTE: emur
        // itself has filename=Some(seed_path) from startup (main.rs:657);
        // the model tracks the WRITE binding only (C-x C-w sets
        // filename=Some(out_path) on success), so the two filename
        // fields agree only for out_path, never for seed_path.
        prop_assert!(sim.buf.filename.is_none());
        let mut log = String::new();
        for op in ops.iter() {
            send_p2(&s, op);
            sim.op(op);
            log.push_str(&format!("{:?} ", op));
        }
        // final write phase (the ONLY prompt command, 03 §P2)
        s.send_hex(&[0x18, 0x17]); // C-x C-w
        settle(SETTLE_PHASE_MS);
        s.send_text(&out_path);
        settle(200);
        s.send_hex(&[0x0d]); // Enter
        settle(600);
        sim.buf.filename = Some(out_path.clone()); // M-8 binds on success
        sim.buf.modified = false;

        let want = sim.buf.text();
        let got = std::fs::read_to_string(&out_path).unwrap_or_default();
        prop_assert_eq!(got, want, "ops[{}] saved file mismatch; seed leaked for triage: {}", log, seed_path);

        let n = line_of(&sim.buf, sim.buf.point);
        let m = lines_total(&sim.buf);
        let len = sim.buf.len();
        let p = (sim.buf.point * 100).checked_div(len).unwrap_or(0); // screen.rs formula
        let want_line = format!("Wrote {} {}/{} ({}%)", out_path, n, m, p);
        let mut last = Session::last_line(&s.capture());
        if last != want_line {
            let pane2 = s.capture_poll(|c| Session::last_line(c) == want_line);
            last = Session::last_line(&pane2);
        }
        prop_assert_eq!(last, want_line, "ops[{}] tail[{}]", log, pane_tail(&s.capture()));

        let sane = pane_sane(&s, PANE_W);
        prop_assert!(sane.is_none(), "ops[{}] {:?} tail[{}]", log, sane.unwrap_or_default(), pane_tail(&s.capture()));
        // seed file untouched (C-x C-w binds the NEW path only)
        let seed_now = std::fs::read_to_string(&seed_path).unwrap_or_default();
        prop_assert_eq!(seed_now, seed, "seed file mutated");
        // model self-checks reading the remaining pub fields (gate MA-2)
        prop_assert_eq!(sim.buf.filename.as_deref(), Some(out_path.as_str()));
        prop_assert!(!sim.buf.modified);

        s.kill();
        let _ = std::fs::remove_file(&seed_path);
        let _ = std::fs::remove_file(&out_path);
    }
}
// P3-P4-MARKER

// =====================================================================
// P3 - window-structure walk (03 §P3). Seed text has NO '-' and '|',
// typing adds 'a'/' ' only, so boundary marks are unambiguous.
// =====================================================================

#[derive(Debug, Clone)]
enum P3Op {
    Split2,
    Split3,
    Other,
    DelOther,
    Type(char),
    Enter,
    Undo,
}

fn send_p3(s: &Session, op: &P3Op) {
    match op {
        P3Op::Split2 => s.send_hex(&[0x18, 0x32]),
        P3Op::Split3 => s.send_hex(&[0x18, 0x33]),
        P3Op::Other => s.send_hex(&[0x18, 0x6f]),
        P3Op::DelOther => s.send_hex(&[0x18, 0x31]),
        P3Op::Type(c) => s.send_text(&c.to_string()),
        P3Op::Enter => s.send_hex(&[0x0d]),
        P3Op::Undo => s.send_hex(&[0x1f]),
    }
}

fn p3_ops() -> impl Strategy<Value = Vec<P3Op>> {
    let one = prop_oneof![
        3 => Just(P3Op::Split2),
        3 => Just(P3Op::Split3),
        3 => Just(P3Op::Other),
        3 => Just(P3Op::DelOther),
        4 => prop::sample::select(vec!['a', ' ']).prop_map(P3Op::Type),
        1 => Just(P3Op::Enter),
        2 => Just(P3Op::Undo),
    ];
    prop::collection::vec(one, 1..16usize)
}

fn has_dash_row(pane: &str) -> bool {
    // 8 consecutive '-' anywhere in a line. A horizontal-split boundary
    // is 38-40 dashes wide; in a NESTED split the run sits at the line
    // start (left column, "----...----|@"), after the separator (right
    // column, "@...|----...----"), or spans the full width. Seed text
    // and P3 typed chars contain no '-', so the run is structural.
    pane.lines().any(|l| l.contains("--------"))
}

proptest! {
    #![proptest_config(cfg(16))]

    #[test]
    fn p3_window_walk(ops in p3_ops()) {
        let (seed_path, _out) = tmp_paths("p3");
        std::fs::write(&seed_path, "alpha beta\ngamma delta\n").expect("seed write");
        let s = Session::start(&[seed_path.as_str()]);
        let mut log = String::new();
        let mut checks = 0usize;
        for op in ops.iter() {
            send_p3(&s, op);
            log.push_str(&format!("{:?} ", op));
            let split2 = matches!(op, P3Op::Split2);
            let split3 = matches!(op, P3Op::Split3);
            if (!split2 && !split3) || checks >= 3 {
                continue;
            }
            settle(SETTLE_CHECK_MS);
            let pane = s.capture();
            if split2 {
                prop_assert!(has_dash_row(&pane), "ops[{}] no dash row tail[{}]", log, pane_tail(&pane));
            } else {
                prop_assert!(pane.lines().any(|l| l.contains('|')), "ops[{}] no bar col tail[{}]", log, pane_tail(&pane));
            }
            checks += 1;
        }
        // collapse, then the structure marks must be gone and text visible
        s.send_hex(&[0x18, 0x31]);
        settle(SETTLE_BURST_MS);
        let pane = s.capture();
        let tail = pane_tail(&pane);
        prop_assert!(!has_dash_row(&pane), "ops[{}] dash row after C-x 1 tail[{}]", log, tail);
        prop_assert!(!pane.lines().any(|l| l.contains('|')), "ops[{}] bar after C-x 1 tail[{}]", log, tail);
        prop_assert!(pane.contains("alpha"), "ops[{}] text gone tail[{}]", log, tail);
        let sane = pane_sane(&s, PANE_W);
        prop_assert!(sane.is_none(), "ops[{}] {:?} tail[{}]", log, sane.unwrap_or_default(), tail);
        s.kill();
        let _ = std::fs::remove_file(&seed_path);
    }
}

// =====================================================================
// P4 - resize fuzz (03 §P4 + design-deviation note: the status line
// wraps over 3+ rows at -x 10/20, so P4 uses pane_sane_lax: liveness,
// panic, escape litter, width bound).
// =====================================================================

#[derive(Debug, Clone)]
enum P4Op {
    Type(char),
    Enter,
    Cn,
    Cp,
    Ca,
    Ce,
    Split2,
    Split3,
    Other,
    DelOther,
    Undo,
}

fn send_p4(s: &Session, op: &P4Op) {
    match op {
        P4Op::Type(c) => s.send_text(&c.to_string()),
        P4Op::Enter => s.send_hex(&[0x0d]),
        P4Op::Cn => s.send_hex(&[0x0e]),
        P4Op::Cp => s.send_hex(&[0x10]),
        P4Op::Ca => s.send_hex(&[0x01]),
        P4Op::Ce => s.send_hex(&[0x05]),
        P4Op::Split2 => s.send_hex(&[0x18, 0x32]),
        P4Op::Split3 => s.send_hex(&[0x18, 0x33]),
        P4Op::Other => s.send_hex(&[0x18, 0x6f]),
        P4Op::DelOther => s.send_hex(&[0x18, 0x31]),
        P4Op::Undo => s.send_hex(&[0x1f]),
    }
}

#[derive(Debug, Clone)]
struct P4Step {
    ops: Vec<P4Op>,
    w: usize,
    h: usize,
}

fn p4_steps() -> impl Strategy<Value = Vec<P4Step>> {
    let op = prop_oneof![
        6 => prop::sample::select(vec!['a', 'b', 'g', ' ']).prop_map(P4Op::Type),
        1 => Just(P4Op::Enter),
        2 => Just(P4Op::Cn),
        2 => Just(P4Op::Cp),
        1 => Just(P4Op::Ca),
        1 => Just(P4Op::Ce),
        2 => Just(P4Op::Split2),
        2 => Just(P4Op::Split3),
        2 => Just(P4Op::Other),
        2 => Just(P4Op::DelOther),
        2 => Just(P4Op::Undo),
    ];
    let step = (
        prop::collection::vec(op, 1..6usize),
        prop::sample::select(vec![10usize, 20, 40, 80, 120, 200]),
        prop::sample::select(vec![4usize, 10, 24, 50]),
    )
        .prop_map(|(ops, w, h)| P4Step { ops, w, h });
    prop::collection::vec(step, 1..4usize)
}

proptest! {
    #![proptest_config(cfg(16))]

    #[test]
    fn p4_resize_fuzz(steps in p4_steps()) {
        let s = Session::start(&[]);
        let mut log = String::new();
        let mut lastw = PANE_W;
        for st in steps.iter() {
            for op in st.ops.iter() {
                send_p4(&s, op);
                log.push_str(&format!("{:?} ", op));
            }
            s.resize(st.w, st.h);
            lastw = st.w;
            log.push_str(&format!("R{}x{} ", st.w, st.h));
            settle(SETTLE_PHASE_MS);
        }
        s.send_hex(&[0x18, 0x31, 0x07]); // C-x 1, C-g
        settle(SETTLE_BURST_MS);
        let sane = pane_sane_lax(&s, lastw);
        prop_assert!(sane.is_none(), "ops[{}] {:?}", log, sane.unwrap_or_default());
    }
}

