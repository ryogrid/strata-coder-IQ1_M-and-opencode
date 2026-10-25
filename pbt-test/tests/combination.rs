// P5-P7: combination (full-pane) properties.
// Design: ../../test-design/pbt-design-combination/ (README/01-05).
//
// The model = state machine (bufs/tree/active/msg/killed/undo, mirroring
// main.rs) + PURE renderer mirroring screen.rs (02 §3) + minibuffer
// machine mirroring terminal.rs read_line_c (02 §4). buffer.rs and
// window.rs are compiled VERBATIM via #[path] (pure modules). Every op
// is ONE FULL KEYBINDING (01 §3): prompt commands carry their scripted
// tail, so no checkpoint ever sees a dangling prefix/prompt.
//
// Serial law (04): run with --test-threads=1 inside pbt-run.sh; no other
// tmux suite and no emur build while running. Support layers: core+sess
// are included (04 §1 use map); sanity (pane_sane) is NOT compiled -
// combination runs its own comb_sane (05 §12).

#[path = "support/core.rs"]
mod core;
#[path = "support/sess.rs"]
mod sess;

#[path = "../../src/buffer.rs"]
mod buffer;
#[path = "../../src/window.rs"]
mod window;

use buffer::Buffer;
use core::{
    cfg, last_line, pane_tail, parse_status, settle, PANE_W, SETTLE_BURST_MS,
    SETTLE_CHECK_MS, SETTLE_PHASE_MS,
};
use proptest::prelude::*;
use sess::Session;
use window::{new_tree, Frame, Tree};

// The combination sanity (04 §1 v2): liveness + no escape litter +
// width bound. The P1 status-TAIL parser is NOT used here: long
// messages TRUNCATE the status row at the pane width, leaving an
// incomplete tail (05 §12 - documented behavior, not a bug). The
// full grid diff of every checkpoint is the surface check.
fn comb_sane(s: &Session, w: usize) -> Option<String> {
    let raw = s.capture_bytes();
    let pane = s.capture();
    if let Some(pos) = pane.find("EMU_EXITED") {
        let line = pane[pos..].lines().next().unwrap_or("");
        return Some(format!("session exited: {}", line));
    }
    if pane.contains("panicked") {
        return Some("panic text in pane".to_string());
    }
    if raw.contains(&0x1b) {
        return Some("escape litter: raw ESC byte in capture".to_string());
    }
    for line in pane.lines() {
        if line.chars().count() > w {
            return Some(format!(
                "line over width {}: {:?}",
                w,
                line.chars().take(20).collect::<String>()
            ));
        }
    }
    None
}

static SEQN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn paths(tag: &str) -> (String, String, String) {
    let id = SEQN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let stem = format!("/tmp/emur-pbt-{}-{}-{}", std::process::id(), tag, id);
    (
        format!("{}-seed.txt", stem),
        format!("{}-out.txt", stem),
        format!("{}-new.txt", stem),
    )
}

// =====================================================================
// The minibuffer reader mirror (terminal.rs read_line_c, ASCII pool:
// cells == chars). 02 §4.
// =====================================================================

fn completions(input: &str, cands: &[String]) -> Vec<String> {
    cands.iter().filter(|c| c.starts_with(input)).cloned().collect()
}

fn common_prefix(hits: &[String]) -> String {
    let mut cp = String::new();
    for (i, c) in hits[0].chars().enumerate() {
        if hits[1..].iter().all(|h| h.chars().nth(i) == Some(c)) {
            cp.push(c);
        } else {
            break;
        }
    }
    cp
}

fn list_width(list: &[String]) -> usize {
    if list.is_empty() {
        0
    } else {
        2 + list.iter().map(|c| c.chars().count() + 1).sum::<usize>()
    }
}

// cand_at mirror (terminal.rs:380-392, cells == chars on the pool).
fn cand_at(list: &[String], inp: usize, cur: usize) -> String {
    let mut s = inp + 2;
    for cand in list {
        let l = cand.chars().count();
        if cur >= s && cur <= s + l {
            return cand.clone();
        }
        s += l + 1;
    }
    "".to_string()
}

// file_candidates mirror (terminal.rs:261-344): entries after the last
// '/', dirs get '/', prefix filter on the tail, sorted. NO '~' paths in
// the pool (expand_tilde is a pass-through recorded in 02 §4).
fn file_candidates(input: &str) -> Vec<String> {
    // mirror of terminal.rs:261-344: candidates = head + name (+ "/" for
    // dirs) = FULL paths. Production DEFAULTS the listing dir to "."
    // (Tab on a slash-free input lists the CWD, terminal.rs:285-299);
    // the model returns empty (bell) instead - the P5/P6 pools always
    // type ABSOLUTE paths, so the CWD-listing arm is unreachable
    // (recorded, 05 §14)
    let Some(k) = input.rfind('/') else { return vec![] };
    let dir = &input[..k + 1];
    let tail = &input[k + 1..];
    let mut hits: Vec<String> = vec![];
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(tail) {
                if e.path().is_dir() {
                    hits.push(format!("{}{}/", dir, name));
                } else {
                    hits.push(format!("{}{}", dir, name));
                }
            }
        }
    }
    hits.sort();
    hits
}

// The reader mirror, split-driveable (terminal.rs read_line_c, ASCII
// pool: cells == chars). P5 feeds the whole tail at once; P6 feeds a
// prefix, captures the mid-prompt echo, then commits (02 §4).
struct Rd {
    prompt: String,
    cands: Vec<String>,
    file_comp: bool,
    out: Vec<char>,
    list: Vec<String>,
    cur: usize, // dual-unit caret (units == chars, ASCII)
    sel: String,
    cancelled: bool,
    done: bool,
    after: Vec<u8>, // bytes fed after the reader returned (main-loop keys)
    esc: u8, // ESC byte-sequence state: 1 = after ESC, 2 = after "ESC ["
}

impl Rd {
    fn new(prompt: &str, cands: &[String], file_comp: bool) -> Rd {
        Rd {
            prompt: prompt.to_string(),
            cands: cands.to_vec(),
            file_comp,
            out: vec![],
            list: vec![],
            cur: 0,
            sel: String::new(),
            cancelled: false,
            done: false,
            after: vec![],
            esc: 0,
        }
    }

    // The full physical echo row (prompt + input + list), plus the
    // C-g suffix. Echo budget law (05 §10): callers assert
    // echo().chars().count() <= grid width before comparing.
    fn echo(&self) -> String {
        let mut e = self.prompt.clone();
        e.extend(self.out.iter());
        if !self.list.is_empty() {
            e.push_str("  ");
            for c in &self.list {
                e.push_str(c);
                e.push(' ');
            }
        }
        if self.cancelled {
            e.push_str(" [Cancelled]");
        }
        e
    }

    fn feed(&mut self, bytes: &[u8]) {
        for b in bytes {
            if self.done {
                // the reader RETURNED: production's main loop eats the
                // remaining bytes as plain keys (e.g. C-g cancels, the
                // scripted Enter that follows inserts a newline)
                self.after.push(*b);
                continue;
            }
            self.step(*b);
        }
    }

    fn step(&mut self, b: u8) {
        let tl = self.out.len() + list_width(&self.list);
        if b == 0x0D || b == 0x0A {
            // Enter: the candidate under the caret wins (464-474)
            let mut ret: String = self.out.iter().collect();
            if !self.list.is_empty() {
                let sel = cand_at(&self.list, self.out.len(), self.cur);
                if !sel.is_empty() {
                    ret = sel;
                }
            }
            self.sel = ret;
            self.done = true;
            return;
        }
        if b == 0x07 {
            self.sel = String::new();
            self.cancelled = true;
            self.done = true;
            return;
        }
        if b == 0x09 {
            let inp: String = self.out.iter().collect();
            let hits = if self.file_comp {
                completions(&inp, &file_candidates(&inp))
            } else {
                completions(&inp, &self.cands)
            };
            if hits.is_empty() {
                // bell: echo unchanged (481-539)
            } else if hits.len() == 1 {
                self.list = vec![];
                let full: Vec<char> = hits[0].chars().collect();
                let mut k = inp.chars().count();
                while k < full.len() {
                    self.out.push(full[k]);
                    k += 1;
                }
                self.cur = self.out.len();
            } else {
                let cp: Vec<char> = common_prefix(&hits).chars().collect();
                let mut k = self.out.len();
                while k < cp.len() {
                    self.out.push(cp[k]);
                    k += 1;
                }
                self.list = hits;
                self.cur = self.out.len() + list_width(&self.list);
            }
            return;
        }
        if b == 0x02 {
            if self.cur > 0 {
                self.cur -= 1;
            }
            return;
        }
        if b == 0x06 {
            if self.cur < tl {
                self.cur += 1;
            }
            return;
        }
        if b == 0x01 {
            self.cur = 0;
            return;
        }
        if b == 0x05 {
            self.cur = tl;
            return;
        }
        if b == 0x1b {
            self.esc = 1; // production consumes 2 more bytes after ESC
            return;
        }
        if self.esc == 1 {
            self.esc = if b == 0x5b { 2 } else { 0 };
            return;
        }
        if self.esc == 2 {
            // arrow byte: D = caret left, C = caret right, else swallow
            self.esc = 0;
            if b == 0x44 && self.cur > 0 {
                self.cur -= 1;
            } else if b == 0x43 && self.cur < tl {
                self.cur += 1;
            }
            return;
        }
        if b == 0x7f {
            if self.cur > self.out.len() {
                self.cur = self.out.len();
            }
            if self.cur > 0 {
                self.out.remove(self.cur - 1);
                self.cur -= 1;
                self.list = vec![]; // stale list cleared on deletion (590, 606)
            }
            return;
        }
        // printable insert at caret clamped to the input end
        if self.cur > self.out.len() {
            self.cur = self.out.len();
        }
        if (0x20..0x7f).contains(&b) {
            self.out.insert(self.cur, b as char);
            self.cur += 1;
            self.list = vec![];
        }
        // control bytes outside the arms above: silent continue
    }
}

// =====================================================================
// The renderer mirror (screen.rs pipeline, ASCII pool: widths == 1).
// 02 §3. Grid = (w x h); rows 0..h-2 = tree frames, row h-1 = status.
// =====================================================================

// split_lines mirror (screen.rs:97-110): ALWAYS a trailing final line.
fn split_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    let mut cur = String::new();
    for c in text.chars() {
        if c == '\n' {
            lines.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    lines.push(cur);
    lines
}

fn line_of(text: &str, point: usize) -> usize {
    text.chars().take(point).filter(|c| *c == '\n').count()
}

// draw_cell mirror (screen.rs:311-364, ASCII): control chars -> SPACE,
// truncate to width, pad with spaces.
fn draw_row(s: &str, width: usize) -> Vec<char> {
    let mut out: Vec<char> = vec![];
    for c in s.chars() {
        if out.len() >= width {
            break;
        }
        out.push(if (c as u32) < 0x20 { ' ' } else { c });
    }
    while out.len() < width {
        out.push(' ');
    }
    out
}

#[derive(Clone, Copy)]
struct LeafPos {
    pline: usize,
}

// The full draw event: scroll prepass (screen.rs:182-189, MUTATES leaf
// scroll), frame paint in `frames` order (S-12 overlaps: later frames
// overwrite, exactly the production paint order), status line
// (screen.rs:193-278: msg > Buf name * > Buf name, ACTIVE leaf only).
fn render(
    bufs: &[Buffer],
    tree: &mut Tree,
    active: usize,
    msg: &str,
    w: usize,
    h: usize,
) -> Vec<String> {
    let height = if h > 1 { h - 1 } else { 1 };
    let frames = tree.layout(tree.root, 0, height, 0, w);
    let mut grid: Vec<Vec<char>> = vec![vec![' '; w]; h];

    // pass 1: scroll prepass per leaf (point_pos ONCE per leaf, S-10)
    let mut pos: Vec<LeafPos> = vec![LeafPos { pline: 0 }; tree.leaves.len()];
    for f in frames.iter() {
        if let Frame::Window { leaf, height: fh, .. } = f {
            let l = &tree.leaves[*leaf];
            let text = bufs[l.buffer].text();
            let pline = line_of(&text, l.point);
            pos[*leaf] = LeafPos { pline };
            let mut scroll = l.scroll;
            if pline < scroll {
                scroll = pline;
            }
            if pline >= scroll + *fh {
                scroll = pline - fh + 1;
            }
            tree.leaves[*leaf].scroll = scroll;
        }
    }

    // pass 2: paint frames in order
    for f in frames.iter() {
        match f {
            Frame::BoundaryRow { top, col, width } => {
                for c in grid[*top][*col..*col + *width].iter_mut() {
                    *c = '-';
                }
            }
            Frame::BoundaryCol { top, height: bh, col } => {
                for rr in 0..*bh {
                    grid[*top + rr][*col] = '|';
                }
            }
            Frame::Window { leaf, top, height: wh, col, width } => {
                let l = &tree.leaves[*leaf];
                let lines = split_lines(&bufs[l.buffer].text());
                for r in 0..*wh {
                    let li = l.scroll + r;
                    // '@' filler = a single '@' + spaces (screen.rs:246,
                    // gate J1), NOT a full '@' row
                    let s = if li < lines.len() { lines[li].clone() } else { "@".to_string() };
                    let cells = draw_row(&s, *width);
                    grid[*top + r][*col..*col + *width].copy_from_slice(&cells);
                }
            }
        }
    }

    // status/echo row (h-1): ACTIVE leaf only
    let l = &tree.leaves[active];
    let text = bufs[l.buffer].text();
    let lines = split_lines(&text);
    let pline = pos[active].pline;
    let st_total = lines.len();
    let st_pct = (l.point * 100).checked_div(text.chars().count()).unwrap_or(0);
    let label = if !msg.is_empty() {
        msg.to_string()
    } else if bufs[l.buffer].modified {
        format!("Buf {} *", bufs[l.buffer].name)
    } else {
        format!("Buf {}", bufs[l.buffer].name)
    };
    let status = format!("{} {}/{} ({}%)", label, pline + 1, st_total, st_pct);
    grid[h - 1] = draw_row(&status, w);

    grid.iter().map(|r| r.iter().collect::<String>().trim_end().to_string()).collect()
}

// Pane rows vs model rows (both per-line trim_end; tmux capture drops
// trailing blank lines - the model pads with "" and the loop tolerates).
fn grid_diff(model: &[String], pane: &str) -> Option<(usize, String, String)> {
    let prow: Vec<&str> = pane.lines().collect();
    let n = model.len().max(prow.len());
    for i in 0..n {
        let m = model.get(i).map(|s| s.as_str()).unwrap_or("");
        let p = prow.get(i).copied().unwrap_or("");
        if m != p {
            return Some((i, m.to_string(), p.to_string()));
        }
    }
    None
}

// Report block (03 §P5.6): op log + first differing row + 6-row windows.
fn diff_report(log: &str, i: usize, m: &str, p: &str, grid: &[String], pane: &str) -> String {
    let lo = i.saturating_sub(3);
    let gm: Vec<String> = grid[lo..grid.len().min(i + 4)].to_vec();
    let pl: Vec<&str> = pane.lines().collect();
    let gp: Vec<&str> = pl[lo..pl.len().min(i + 4)].to_vec();
    format!(
        "ops[{}] row {} differs\n  MODEL: {:?}\n  TMUX : {:?}\n  MODEL ctx: {:?}\n  TMUX  ctx: {:?}",
        log, i, m, p, gm, gp
    )
}

// =====================================================================
// The Ed mirror (main.rs): full loop state + per-op iteration (02 §1).
// =====================================================================

struct EdEntry {
    bi: usize,
    tag: u8,
    st: buffer::UndoState,
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Ty(char), // pool incl. '\n' (0x0a/0x0d = Newline) and ' '
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
    Cg,
    Quote(char),
    Split2,
    Split3,
    Other,
    DelOther,
    PrefixG,
    MetaG,
    SwbName(usize), // 0=*demo*, 1=novel, 2=seed (materialized in body)
    SwbTabEnter,
    SwbEmpty,
    SwbCancel,
    SwbListEnter,
    SwbListCancel,
    Wf,
    Sf,
    FiSeed,
    FiMiss,
    FiNew,
    IfSeed,
    KillYes,
    KillNo,
    Gl(usize), // 0="1" 1="0" 2="999" 3="xy" 4="" (empty)
    RfFound,
    RfMiss,
    IsEnter,
    IsCg,
}

struct M {
    bufs: Vec<Buffer>,
    tree: Tree,
    active: usize,
    msg: String,
    killed: String,
    prev: u8,
    moved: bool,
    undo: Vec<EdEntry>,
    group_alive: bool,
    touched: bool,
    seed_path: String,
    out_path: String,
    new_path: String,
    miss_path: String,
    // write events: (path, text AT the write) - P5 invariant 5
    writes: Vec<(String, String)>,
    // last DRAW event's grid (production frame() runs at the end of
    // every loop iteration + at startup; mid-prompt frames compare
    // rows 0..h-2 of this cache with the echo row, 02 §3)
    grid: Vec<String>,
    w: usize,
    h: usize,
}

fn attach_leaf(tree: &mut Tree, active: usize, nb: usize) {
    // main.rs:60-65
    let l = &mut tree.leaves[active];
    l.buffer = nb;
    l.point = 0;
    l.mark = 0;
    l.scroll = 0;
}

fn show_buffer(tree: &mut Tree, active: &mut usize, nb: usize) {
    // main.rs:38-46: lone window splits below, buffer shows in the NEW
    // leaf (old keeps its buffer); split tree replaces in place.
    if tree.leaves.len() == 1 {
        tree.split(*active, true);
        *active = tree.leaves.len() - 1;
        attach_leaf(tree, *active, nb);
    } else {
        attach_leaf(tree, *active, nb);
    }
}

impl M {
    fn new(
        seed_path: &str,
        seed_text: &str,
        out_path: &str,
        new_path: &str,
        miss_path: &str,
        w: usize,
        h: usize,
    ) -> M {
        // startup (main.rs:739-752): with a FILE ARG the arg buffer
        // REPLACES bufs[0] (no *demo*, no split): one buffer, one leaf.
        let mut b0 = Buffer::new(seed_path, seed_text);
        b0.filename = Some(seed_path.to_string());
        let bufs = vec![b0];
        let tree = new_tree(0);
        let active = 0usize;
        let mut m = M {
            bufs,
            tree,
            active,
            msg: String::new(),
            killed: String::new(),
            prev: 0,
            moved: false,
            undo: vec![],
            group_alive: false,
            touched: false,
            seed_path: seed_path.to_string(),
            out_path: out_path.to_string(),
            new_path: new_path.to_string(),
            miss_path: miss_path.to_string(),
            writes: vec![],
            grid: vec![],
            w,
            h,
        };
        m.draw();
        m
    }

    fn bi(&self) -> usize {
        self.tree.leaves[self.active].buffer
    }

    fn draw(&mut self) {
        self.grid = render(&self.bufs, &mut self.tree, self.active, &self.msg, self.w, self.h);
    }

    fn push_entry(&mut self, bi: usize, tag: u8, st: buffer::UndoState) {
        // Ed::push_entry verbatim (merge = run's FIRST pre-state stays)
        let merge = tag != 0
            && self.group_alive
            && self.undo.last().is_some_and(|e| e.tag == tag && e.bi == bi);
        self.touched = true;
        self.group_alive = true;
        if merge {
            return;
        }
        self.undo.push(EdEntry { bi, tag, st });
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
    }

    fn change(&mut self, tag: u8, f: impl FnOnce(&mut M) -> bool) {
        let bi = self.bi();
        let pre = self.bufs[bi].snapshot();
        let applied = f(self);
        if applied && self.bufs[bi].text() != pre.text {
            self.push_entry(bi, tag, pre);
        }
    }

    fn sync_in(&mut self) {
        // main.rs:171-184 (M-15): clamp leaf AND buffer point/mark, both ways
        let bi = self.bi();
        let l = self.bufs[bi].len();
        let lp = if self.tree.leaves[self.active].point > l {
            l
        } else {
            self.tree.leaves[self.active].point
        };
        let lm = if self.tree.leaves[self.active].mark > l {
            l
        } else {
            self.tree.leaves[self.active].mark
        };
        self.tree.leaves[self.active].point = lp;
        self.tree.leaves[self.active].mark = lm;
        self.bufs[bi].point = lp;
        self.bufs[bi].mark = lm;
    }

    fn iter(&mut self, op: &Op) {
        self.sync_in();
        self.moved = false;
        self.touched = false;
        self.dispatch(op);
        if !self.moved {
            let bi = self.bi();
            let (p, m) = (self.bufs[bi].point, self.bufs[bi].mark);
            let leaf = &mut self.tree.leaves[self.active];
            leaf.point = p;
            leaf.mark = m;
        }
        if !self.touched {
            self.group_alive = false;
        }
        self.prev = self.lead(op);
        self.draw(); // production frame() at the end of every iteration
    }

    fn lead(&self, op: &Op) -> u8 {
        // loop-bottom prev_cmd = b (the top-level lead byte; P2 table)
        match op {
            Op::Ty(c) => {
                if c.is_ascii() {
                    *c as u8
                } else {
                    0xc3
                }
            }
            Op::Bs => 0x7f,
            Op::Cd => 0x04,
            Op::Ck => 0x0b,
            Op::Cy => 0x19,
            Op::Ca => 0x01,
            Op::Ce => 0x05,
            Op::Cb => 0x02,
            Op::Cf => 0x06,
            Op::Cn => 0x0e,
            Op::Cp => 0x10,
            Op::Co => 0x0f,
            Op::Ct => 0x14,
            Op::Cspc => 0x00,
            Op::Mb | Op::Mf | Op::Mlt | Op::Mgt | Op::Arr(_) | Op::MetaG => 0x1b,
            Op::Undo => 0x1f,
            Op::Cg => 0x07,
            Op::Quote(_) => 0x11,
            Op::Split2
            | Op::Split3
            | Op::Other
            | Op::DelOther
            | Op::PrefixG
            | Op::SwbName(_)
            | Op::SwbTabEnter
            | Op::SwbEmpty
            | Op::SwbCancel
            | Op::SwbListEnter
            | Op::SwbListCancel
            | Op::Wf
            | Op::Sf
            | Op::FiSeed
            | Op::FiMiss
            | Op::FiNew
            | Op::IfSeed
            | Op::KillYes
            | Op::KillNo => 0x18,
            Op::Gl(_) => 0x0c,
            Op::RfFound | Op::RfMiss => 0x12,
            Op::IsEnter | Op::IsCg => 0x13,
        }
    }

    fn bnames(&self) -> Vec<String> {
        self.bufs.iter().map(|b| b.name.clone()).collect()
    }

    fn dispatch(&mut self, op: &Op) {
        let bi = self.bi();
        match op {
            Op::Ty(c) => {
                let c = *c;
                self.change(1, |m| {
                    let b = m.bi();
                    m.bufs[b].insert_char(c);
                    true
                });
            }
            Op::Quote(c) => {
                let c = *c;
                self.change(1, |m| {
                    let b = m.bi();
                    m.bufs[b].insert_char(c);
                    true
                });
            }
            Op::Bs => self.change(0, |m| {
                let b = m.bi();
                m.bufs[b].delete_backward()
            }),
            Op::Cd => self.change(0, |m| {
                let b = m.bi();
                m.bufs[b].delete_forward()
            }),
            Op::Ck => {
                self.change(2, |m| {
                    let b = m.bi();
                    let cut = m.bufs[b].kill_to_end_of_line();
                    if m.prev == 0x0b {
                        m.killed.push_str(&cut);
                    } else {
                        m.killed = cut;
                    }
                    true
                });
            }
            Op::Cy => self.change(0, |m| {
                let b = m.bi();
                let k = m.killed.clone();
                m.bufs[b].insert_string(&k);
                !k.is_empty()
            }),
            Op::Co => self.change(0, |m| {
                let b = m.bi();
                m.bufs[b].insert_newline_after();
                true
            }),
            Op::Ct => self.change(0, |m| {
                let b = m.bi();
                m.bufs[b].transpose_chars();
                true
            }),
            Op::Cspc => {
                let p = self.bufs[bi].point;
                self.bufs[bi].mark = p;
                self.msg = "".to_string();
            }
            Op::Ca => self.bufs[bi].beginning_of_line(),
            Op::Ce => self.bufs[bi].end_of_line(),
            Op::Cb => self.bufs[bi].backward_char(),
            Op::Cf => self.bufs[bi].forward_char(),
            Op::Cn => self.bufs[bi].next_line(),
            Op::Cp => self.bufs[bi].previous_line(),
            Op::Mb => self.bufs[bi].backward_word(),
            Op::Mf => self.bufs[bi].forward_word(),
            Op::Mlt => self.bufs[bi].beginning_of_buffer(),
            Op::Mgt => self.bufs[bi].end_of_buffer(),
            Op::Arr(t) => match t {
                0x41 => self.bufs[bi].previous_line(),
                0x42 => self.bufs[bi].next_line(),
                0x43 => self.bufs[bi].forward_char(),
                _ => self.bufs[bi].backward_char(),
            },
            Op::Undo => {
                // Ed::undo verbatim
                self.group_alive = false;
                match self.undo.pop() {
                    Some(e) => {
                        self.bufs[e.bi].restore(&e.st);
                        self.msg = "Undo".to_string();
                    }
                    None => self.msg = "No change to undo".to_string(),
                }
            }
            Op::Cg => self.msg = "Cancelled".to_string(),
            Op::Split2 => {
                self.tree.split(self.active, true);
                self.msg = "".to_string();
            }
            Op::Split3 => {
                self.tree.split(self.active, false);
                self.msg = "".to_string();
            }
            Op::Other => {
                // other_window (main.rs:235-254): sync_leaf then NEXT
                // flatten leaf, wrapping; moved
                let bi = self.bi();
                let (p, m) = (self.bufs[bi].point, self.bufs[bi].mark);
                let leaf = &mut self.tree.leaves[self.active];
                leaf.point = p;
                leaf.mark = m;
                let order = self.tree.flatten();
                let mut j = 0;
                while j < order.len() {
                    if order[j] == self.active {
                        self.active = if j + 1 < order.len() { order[j + 1] } else { order[0] };
                        break;
                    }
                    j += 1;
                }
                self.moved = true;
                self.msg = "".to_string();
            }
            Op::DelOther => {
                self.active = self.tree.collapse(self.active);
                self.msg = "".to_string();
            }
            Op::PrefixG => {
                // Unbound(0x18, 0x07): BEL renders as SPACE via draw_cell
                self.msg = format!("Undefined command: C-x {}", 0x07u8 as char);
            }
            Op::MetaG => {} // MetaUnbound: silent
            // prompt-driven ops run through prompt_run() (P5) and the
            // P6 split-drive; dispatch never receives them
            Op::SwbName(_)
            | Op::SwbTabEnter
            | Op::SwbEmpty
            | Op::SwbCancel
            | Op::SwbListEnter
            | Op::SwbListCancel
            | Op::Wf
            | Op::Sf
            | Op::FiSeed
            | Op::FiMiss
            | Op::FiNew
            | Op::IfSeed
            | Op::KillYes
            | Op::KillNo
            | Op::Gl(_)
            | Op::RfFound
            | Op::RfMiss
            | Op::IsEnter
            | Op::IsCg => {}
        }
    }

    fn apply_swb(&mut self, sel: &str, listform: bool) {
        // main.rs:407-425 verbatim (ti logic; cycle arm; create arm)
        let bi = self.bi();
        let mut ti = usize::MAX;
        if !sel.is_empty() {
            if let Some(h) = self.bufs.iter().position(|b| b.name == sel) {
                ti = h;
            }
        } else if listform {
            ti = if bi + 1 < self.bufs.len() { bi + 1 } else { 0 };
        }
        if ti == usize::MAX && sel.is_empty() {
            self.msg = "Cancelled".to_string();
        } else if ti < self.bufs.len() {
            attach_leaf(&mut self.tree, self.active, ti);
            self.moved = true;
            self.msg = "".to_string();
        } else {
            self.bufs.push(Buffer::new(sel, ""));
            let idx = self.bufs.len() - 1;
            attach_leaf(&mut self.tree, self.active, idx);
            self.moved = true;
            self.msg = "".to_string();
        }
    }

    // ---- the main-loop wrapper around ONE command (main.rs:772-869) ----
    fn begin_cmd(&mut self) {
        self.sync_in();
        self.moved = false;
        self.touched = false;
    }
    // end_cmd with a RETURNING body (the prompt-apply tail-bytes)
    fn end_cmd_ret(&mut self, lead: u8, f: impl FnOnce(&mut M) -> Vec<u8>) -> Vec<u8> {
        let out = f(self);
        self.finish_cmd(lead);
        out
    }
    fn end_cmd(&mut self, lead: u8, f: impl FnOnce(&mut M)) {
        f(self);
        self.finish_cmd(lead);
    }
    fn finish_cmd(&mut self, lead: u8) {
        if !self.moved {
            let bi = self.bi();
            let (p, mk) = (self.bufs[bi].point, self.bufs[bi].mark);
            let leaf = &mut self.tree.leaves[self.active];
            leaf.point = p;
            leaf.mark = mk;
        }
        if !self.touched {
            self.group_alive = false;
        }
        self.prev = lead;
        self.draw();
    }
    fn run_cmd(&mut self, lead: u8, f: impl FnOnce(&mut M)) {
        self.begin_cmd();
        self.end_cmd(lead, f);
    }

    fn plain_byte(&mut self, b: u8) {
        // kill on an UNMODIFIED buffer shows no prompt: the scripted
        // answer bytes are eaten by the MAIN LOOP as plain keys
        // (self-insert / newline). Mirror byte by byte.
        match b {
            0x0A | 0x0D => self.change(1, |m| {
                let bi = m.bi();
                m.bufs[bi].insert_char('\n');
                true
            }),
            0x20..=0x7E => self.change(1, |m| {
                let bi = m.bi();
                m.bufs[bi].insert_char(b as char);
                true
            }),
            _ => {}
        }
    }

    // ---- per-command effects (reader output precomputed) ----
    fn apply_find(&mut self, p: &str) {
        // find_file (main.rs:256-297)
        if p.is_empty() {
            return; // empty path: silent
        }
        let hit = self.bufs.iter().position(|b| b.name == p);
        if let Some(nb) = hit {
            if self.tree.leaves[self.active].buffer == nb {
                self.msg = "".to_string(); // the ACTIVE leaf shows it: no-op
            } else {
                show_buffer(&mut self.tree, &mut self.active, nb);
                self.moved = true;
                self.msg = "".to_string();
            }
        } else {
            match std::fs::read_to_string(p) {
                Ok(text) => {
                    let mut nb = Buffer::new(p, text);
                    nb.filename = Some(p.to_string());
                    nb.modified = false;
                    self.bufs.push(nb);
                    let idx = self.bufs.len() - 1;
                    show_buffer(&mut self.tree, &mut self.active, idx);
                    self.moved = true;
                    self.msg = "".to_string();
                }
                Err(e) => {
                    // io::Error Display == production's (same fs call)
                    self.msg = format!("Failed to open {}: {}", p, e);
                }
            }
        }
    }

    fn apply_write(&mut self, p: &str) {
        // write_file (main.rs:315-331): binding ONLY on success (M-8)
        if p.is_empty() {
            return; // silent, msg retained
        }
        let bi = self.bi();
        let text = self.bufs[bi].text();
        if std::fs::write(p, text.as_bytes()).is_ok() {
            self.bufs[bi].filename = Some(p.to_string());
            self.bufs[bi].modified = false;
            self.msg = format!("Wrote {}", p);
            self.writes.push((p.to_string(), text));
        } else {
            self.msg = format!("Failed to write {}", p);
        }
    }

    fn apply_insert(&mut self, p: &str) {
        // insert_file (main.rs:333-354)
        if p.is_empty() {
            return;
        }
        match std::fs::read_to_string(p) {
            Ok(text) => {
                self.change(0, |m| {
                    let bi = m.bi();
                    m.bufs[bi].insert_string(&text);
                    true
                });
                self.msg = format!("Inserted file {}", p);
            }
            Err(e) => self.msg = format!("Failed to open {}: {}", p, e),
        }
    }

    fn kill_fresh(&mut self, bi: usize, name: &str) {
        // main.rs:372-390 (fresh buffer, undo retain, ALL alias reset)
        self.bufs[bi] = Buffer::new(name, "");
        self.bufs[bi].filename = None;
        self.bufs[bi].modified = false;
        self.undo.retain(|e| e.bi != bi);
        let mut lk = 0;
        while lk < self.tree.leaves.len() {
            if self.tree.leaves[lk].buffer == bi {
                self.tree.leaves[lk].point = 0;
                self.tree.leaves[lk].mark = 0;
                self.tree.leaves[lk].scroll = 0;
            }
            lk += 1;
        }
        self.msg = format!("Killed buffer {}", name);
    }

    fn apply_kill(&mut self, sel: &str, echo_tail: &[u8]) -> Vec<u8> {
        let bi = self.bi();
        let name = self.bufs[bi].name.clone();
        if !self.bufs[bi].modified {
            // NO prompt: proceed; the scripted answer is NOT consumed
            // by a reader - it flows to the MAIN LOOP in WIRE order
            // (y, e, s, then Enter - the tail is built ans+0x0D),
            // typing into the FRESH buffer, one command per byte
            self.kill_fresh(bi, &name);
            return echo_tail.to_vec();
        }
        if sel == "yes" || sel == "y" {
            self.kill_fresh(bi, &name);
        } else {
            self.msg = "Cancelled".to_string();
        }
        vec![]
    }

    fn apply_gl(&mut self, sel: &str) {
        // goto_line (main.rs:650-692), scan verbatim
        if sel.is_empty() {
            self.msg = "Cancelled".to_string();
            return;
        }
        let digits_ok = sel.bytes().all(|c| c.is_ascii_digit());
        let want = if digits_ok { sel.parse::<u64>().ok() } else { None };
        match want {
            None | Some(0) => self.msg = format!("Invalid line number: {}", sel),
            Some(want_ln) => {
                let bi = self.bi();
                let mut tpos = 0usize; // start of line 1
                let mut line = 1u64;
                let mut i = 0usize;
                while i < self.bufs[bi].len() {
                    if self.bufs[bi].char_at(i) == '\n' {
                        line += 1;
                        if line <= want_ln {
                            tpos = i + 1;
                        } else {
                            break;
                        }
                    }
                    i += 1;
                }
                self.bufs[bi].point = tpos;
                self.msg = "".to_string();
            }
        }
    }

    fn apply_rf(&mut self, sel: &str) {
        // find_backward (main.rs:521-535)
        if sel.is_empty() {
            self.msg = "Cancelled".to_string();
            return;
        }
        let bi = self.bi();
        match self.bufs[bi].find_backward(sel, self.bufs[bi].point) {
            Some(start) => {
                self.bufs[bi].point = start;
                self.msg = "".to_string();
            }
            None => self.msg = format!("Search string not found: {}", sel),
        }
    }

    fn sf_step(&mut self) {
        // save (main.rs:299-313): NO prompt, writes the bound filename
        let bi = self.bi();
        let path = self.bufs[bi].filename.clone().unwrap_or_default();
        if path.is_empty() {
            self.msg = "No file name for this buffer".to_string();
        } else {
            let text = self.bufs[bi].text();
            if std::fs::write(&path, text.as_bytes()).is_ok() {
                self.bufs[bi].modified = false;
                self.msg = format!("Wrote {}", path);
                self.writes.push((path, text));
            } else {
                self.msg = format!("Failed to write {}", path);
            }
        }
    }

    fn run_prompt(&mut self, r: PromptRun) {
        // P5 shape: the full byte sequence is already sent; the READER
        // only ever sees the bytes AFTER the command prefix
        self.begin_cmd();
        let mut rd = Rd::new(&r.prompt, &r.cands, r.file_comp);
        rd.feed(&r.bytes[r.prefix_len.min(r.bytes.len())..]);
        let extra = self.end_cmd_ret(r.lead, move |m| (r.apply)(m, rd.sel.clone(), rd.cancelled));
        let mut after = rd.after.clone();
        after.extend(extra);
        run_tail(self, &after);
    }

    // isearch machine: the FULL loop arms (main.rs:428-519; gate J4).
    fn isearch(&mut self, tail: &[u8]) {
        let bi = self.bi();
        let entry = self.bufs[bi].point;
        let mut qs: Vec<char> = vec![];
        // production `last = orig` (main.rs:437): the FIRST char searches
        // from last+1 = orig+1, NOT from 0
        let mut last = entry;
        let mut i = 0usize;
        while i < tail.len() {
            let b = tail[i];
            i += 1;
            if b == 0x07 {
                self.bufs[bi].point = entry;
                self.msg = "Cancelled".to_string();
                return;
            }
            if b == 0x0d {
                self.msg = "".to_string();
                return;
            }
            if b == 0x13 {
                // C-s mid-search: repeat the query from last+1 (484-486)
                if !qs.is_empty() {
                    let q: String = qs.iter().collect();
                    let hit = self
                        .bufs[bi]
                        .find_forward(&q, last + 1)
                        .or_else(|| self.bufs[bi].find_forward(&q, 0));
                    if let Some(end) = hit {
                        self.bufs[bi].point = end;
                        last = end - qs.len(); // main.rs:503 (shared hit arm)
                    }
                }
                continue;
            }
            if b == 0x7f {
                // DEL: pop + RE-SEARCH from 0, last = end - qs.len()
                if !qs.is_empty() {
                    qs.pop();
                }
                if !qs.is_empty() {
                    let q: String = qs.iter().collect();
                    if let Some(end) = self.bufs[bi].find_forward(&q, 0) {
                        self.bufs[bi].point = end;
                        last = end - qs.len();
                    }
                }
                continue;
            }
            if b == 0x1b {
                // ESC accepts. Production consumes ONE byte
                // unconditionally (main.rs:452), and a THIRD only when
                // the second is '[' (453-465: [A-D = arrows). The pools
                // never put ESC in an isearch tail (05 §2), and ESC as
                // the LAST tail byte would eat the next stdin byte in
                // production - unrepresentable here, recorded.
                if i < tail.len() {
                    let n2 = tail[i];
                    i += 1;
                    if n2 == 0x5b && i < tail.len() {
                        let t = tail[i]; // consumed here; the loop returns
                        match t {
                            0x41 => self.bufs[bi].previous_line(),
                            0x42 => self.bufs[bi].next_line(),
                            0x43 => self.bufs[bi].forward_char(),
                            0x44 => self.bufs[bi].backward_char(),
                            _ => {}
                        }
                    }
                }
                self.msg = "".to_string();
                return;
            }
            {
                // Any other byte extends the query: read_char_eof
                // returns None ONLY at EOF (terminal.rs:135-160), so a
                // CONTROL byte is a query CHAR - find_forward against
                // the printable pool misses -> bell, char DROPPED, loop
                // CONTINUES (main.rs:493-513; the EOF break is 488-490,
                // and this harness has no mid-tail EOF). UTF-8 lead
                // byte continuation consumption (PBTF-007) is n/a: the
                // isearch tails are ASCII (05 §2).
                let mut attempt: Vec<char> = qs.clone();
                attempt.push(b as char);
                let q: String = attempt.iter().collect();
                let hit = self
                    .bufs[bi]
                    .find_forward(&q, last + 1)
                    .or_else(|| self.bufs[bi].find_forward(&q, 0));
                if let Some(end) = hit {
                    last = end - attempt.len(); // main.rs:503 (match START)
                    qs = attempt;
                    self.bufs[bi].point = end;
                }
                // MISS: bell - the char is DROPPED from qs (hit-arm
                // append at main.rs:501), point untouched (509-510)
            }
        }
        self.msg = "".to_string();
    }
}

// =====================================================================
// Byte materialization (op = ONE full keybinding, 01 §3)
// =====================================================================

fn op_bytes(op: &Op) -> Vec<u8> {
    // ONE full keybinding for the PLAIN ops (prompt ops: prompt_run)
    let mut v: Vec<u8> = vec![];
    match op {
        Op::Ty(c) | Op::Quote(c) => {
            if matches!(op, Op::Quote(_)) {
                v.push(0x11);
            }
            v.extend(c.to_string().as_bytes());
        }
        Op::Bs => v.push(0x7F),
        Op::Cd => v.push(0x04),
        Op::Ck => v.push(0x0B),
        Op::Cy => v.push(0x19),
        Op::Ca => v.push(0x01),
        Op::Ce => v.push(0x05),
        Op::Cb => v.push(0x02),
        Op::Cf => v.push(0x06),
        Op::Cn => v.push(0x0E),
        Op::Cp => v.push(0x10),
        Op::Co => v.push(0x0F),
        Op::Ct => v.push(0x14),
        Op::Cspc => v.push(0x00),
        Op::Mb => v.extend([0x1B, 0x62]),
        Op::Mf => v.extend([0x1B, 0x66]),
        Op::Mlt => v.extend([0x1B, 0x3C]),
        Op::Mgt => v.extend([0x1B, 0x3E]),
        Op::Arr(t) => v.extend([0x1B, 0x5B, *t]),
        Op::Undo => v.push(0x1F),
        Op::Cg => v.push(0x07),
        Op::Split2 => v.extend([0x18, 0x32]),
        Op::Split3 => v.extend([0x18, 0x33]),
        Op::Other => v.extend([0x18, 0x6F]),
        Op::DelOther => v.extend([0x18, 0x31]),
        Op::PrefixG => v.extend([0x18, 0x07]),
        Op::MetaG => v.extend([0x1B, 0x07]),
        // prompt-driven ops: bytes come from prompt_run(), never here
        Op::SwbName(_)
        | Op::SwbTabEnter
        | Op::SwbEmpty
        | Op::SwbCancel
        | Op::SwbListEnter
        | Op::SwbListCancel
        | Op::Wf
        | Op::Sf
        | Op::FiSeed
        | Op::FiMiss
        | Op::FiNew
        | Op::IfSeed
        | Op::KillYes
        | Op::KillNo
        | Op::Gl(_)
        | Op::RfFound
        | Op::RfMiss
        | Op::IsEnter
        | Op::IsCg => {}
    }
    v
}

fn swb_name(m: &M, k: usize) -> String {
    match k {
        0 => "*demo*".to_string(),
        1 => "nb".to_string(),
        _ => m.seed_path.clone(),
    }
}

// =====================================================================
// PromptRun: one prompt command = full bytes (prefix + scripted tail +
// commit) + the reader spec + the model effect closure (02 §4). P5
// feeds the whole sequence; P6 splits at mid_bytes and captures the
// echo row mid-prompt.
// =====================================================================

// model effect of one prompt command: (reader selection, C-g flag) ->
// bytes left to the MAIN LOOP (the kill command on an UNMODIFIED
// buffer shows no prompt: the answer bytes echo-type into the FRESH
// buffer in WIRE order - y, e, s, Enter LAST (tails are built
// ans+0x0D); production's byte order, main.rs kill_buffer)
type ApplyFn = Box<dyn FnOnce(&mut M, String, bool) -> Vec<u8>>;

pub struct PromptRun {
    bytes: Vec<u8>,
    prefix_len: usize, // command prefix bytes: sent, NOT reader input
    lead: u8,
    prompt: String,
    cands: Vec<String>,
    file_comp: bool,
    mid_bytes: usize,
    apply: ApplyFn,
}

fn pr(
    bytes: Vec<u8>,
    prompt: &str,
    cands: Vec<String>,
    file_comp: bool,
    apply: impl FnOnce(&mut M, String, bool) -> Vec<u8> + 'static,
) -> PromptRun {
    PromptRun {
        bytes,
        prefix_len: 2,
        lead: 0x18,
        prompt: prompt.to_string(),
        cands,
        file_comp,
        mid_bytes: 0,
        apply: Box::new(apply),
    }
}

fn prompt_run(m: &M, op: &Op) -> Option<PromptRun> {
    let names = m.bnames();
    match op {
        Op::SwbName(k) => {
            let mut bytes = vec![0x18, 0x62];
            bytes.extend(swb_name(m, *k).as_bytes());
            bytes.push(0x0D);
            Some(pr(bytes, "Switch to buffer: ", names, false, |m, sel, _c| {
                m.apply_swb(&sel, false);
                vec![]
            }))
        }
        Op::SwbTabEnter => Some(pr(
            vec![0x18, 0x62, 0x09, 0x0D],
            "Switch to buffer: ",
            names,
            false,
            |m, sel, _c| {
                m.apply_swb(&sel, false);
                vec![]
            },
        )),
        Op::SwbEmpty => Some(pr(
            vec![0x18, 0x62, 0x0D],
            "Switch to buffer: ",
            names,
            false,
            |m, sel, _c| {
                m.apply_swb(&sel, false);
                vec![]
            },
        )),
        Op::SwbCancel => Some(pr(
            vec![0x18, 0x62, 0x07],
            "Switch to buffer: ",
            names,
            false,
            |m, sel, _c| {
                m.apply_swb(&sel, false);
                vec![]
            },
        )),
        Op::SwbListEnter => {
            let prompt = format!("Buffers: {} Enter=next, name: ", names.join(" "));
            Some(pr(
                vec![0x18, 0x02, 0x0D],
                &prompt,
                names,
                false,
                |m, sel, _c| {
                m.apply_swb(&sel, true);
                vec![]
            },
            ))
        }
        Op::SwbListCancel => {
            let prompt = format!("Buffers: {} Enter=next, name: ", names.join(" "));
            Some(pr(
                vec![0x18, 0x02, 0x07],
                &prompt,
                names,
                false,
                |m, sel, _c| {
                m.apply_swb(&sel, true);
                vec![]
            },
            ))
        }
        Op::Wf => {
            let mut bytes = vec![0x18, 0x17];
            bytes.extend(m.out_path.as_bytes());
            bytes.push(0x0D);
            Some(pr(bytes, "Write file: ", vec![], true, |m, sel, _c| {
                m.apply_write(&sel);
                vec![]
            }))
        }
        Op::FiSeed | Op::FiMiss | Op::FiNew => {
            let path = match op {
                Op::FiSeed => m.seed_path.clone(),
                Op::FiMiss => m.miss_path.clone(),
                _ => m.new_path.clone(),
            };
            let mut bytes = vec![0x18, 0x06];
            bytes.extend(path.as_bytes());
            bytes.push(0x0D);
            Some(pr(bytes, "Find file: ", vec![], true, move |m, sel, _c| {
                m.apply_find(&sel);
                vec![]
            }))
        }
        Op::IfSeed => {
            let mut bytes = vec![0x18, 0x09];
            bytes.extend(m.seed_path.as_bytes());
            bytes.push(0x0D);
            Some(pr(bytes, "Insert file: ", vec![], true, |m, sel, _c| {
                m.apply_insert(&sel);
                vec![]
            }))
        }
        Op::KillYes | Op::KillNo => {
            let ans: &[u8] = if matches!(op, Op::KillYes) {
                b"yes"
            } else {
                b"no"
            };
            let name = m.bufs[m.bi()].name.clone();
            let mut bytes = vec![0x18, 0x6B];
            bytes.extend(ans);
            bytes.push(0x0D);
            let prompt = format!("Kill buffer {}? (yes|no) ", name);
            let mut tb = ans.to_vec();
            tb.push(0x0D);
            Some(PromptRun {
                bytes,
                prefix_len: 2,
                lead: 0x18,
                prompt,
                cands: vec![],
                file_comp: false,
                mid_bytes: 0,
                apply: Box::new(move |m, sel, _c| m.apply_kill(&sel, &tb)),
            })
        }
        Op::Gl(k) => {
            let q = ["1", "0", "999", "xy", ""][*k];
            let mut bytes = vec![0x0C];
            bytes.extend(q.as_bytes());
            bytes.push(0x0D);
            let mut r = pr(bytes, "Go to line: ", vec![], false, |m, sel, _c| {
                m.apply_gl(&sel);
                vec![]
            });
            r.lead = 0x0C;
            r.prefix_len = 1;
            Some(r)
        }
        Op::RfFound | Op::RfMiss => {
            let q: &[u8] = if matches!(op, Op::RfFound) { b"a" } else { b"zzqz" };
            let mut bytes = vec![0x12];
            bytes.extend(q);
            bytes.push(0x0D);
            let mut r = pr(bytes, "Reverse find: ", vec![], false, |m, sel, _c| {
                m.apply_rf(&sel);
                vec![]
            });
            r.lead = 0x12;
            r.prefix_len = 1;
            Some(r)
        }
        _ => None,
    }
}

// The bytes a reader left AFTER returning are SEPARATE main-loop
// commands (production: each byte dispatches on its own; the loop-
// bottom sync_out and the next sync_in run between them, so a
// cancelled C-x C-b CYCLE followed by Enter inserts at the CYCLED
// leaf's point, one command later - observed via PBTF probe).
fn run_tail(m: &mut M, tail: &[u8]) {
    let mut i = 0;
    while i < tail.len() {
        let b = tail[i];
        let lead: u8 = if b == 0x0A || b == 0x0D { 0x0D } else { b };
        m.run_cmd(lead, |m| m.plain_byte(b));
        i += 1;
    }
}

// Send + model one op (the P5/P7 shared driver; the op is ONE full
// keybinding, 01 §3).
fn run_op(m: &mut M, s: &Session, op: &Op) {
    match op {
        Op::IsEnter | Op::IsCg => {
            let q: &[u8] = if matches!(op, Op::IsEnter) { b"a" } else { b"z" };
            let commit: u8 = if matches!(op, Op::IsEnter) { 0x0D } else { 0x07 };
            let mut tail = q.to_vec();
            tail.push(commit);
            let mut bytes = vec![0x13];
            bytes.extend(&tail);
            s.send_hex(&bytes);
            m.run_cmd(0x13, move |m| m.isearch(&tail));
        }
        Op::Sf => {
            // C-x C-s == 0x18 0x13 (keymap ctrl_x_key: 0x13 Save);
            // 0x18 0x06 is C-x C-f FindFile - sending it opened the
            // Find-file prompt and desynced the NEXT prompt reader
            s.send_hex(&[0x18, 0x13]);
            m.run_cmd(0x18, |m| m.sf_step());
        }
        Op::SwbName(_)
        | Op::SwbTabEnter
        | Op::SwbEmpty
        | Op::SwbCancel
        | Op::SwbListEnter
        | Op::SwbListCancel
        | Op::Wf
        | Op::FiSeed
        | Op::FiMiss
        | Op::FiNew
        | Op::IfSeed
        | Op::KillYes
        | Op::KillNo
        | Op::Gl(_)
        | Op::RfFound
        | Op::RfMiss => {
            let r = prompt_run(m, op).expect("prompt op");
            s.send_hex(&r.bytes);
            m.run_prompt(r);
        }
        _ => {
            let bytes = op_bytes(op);
            s.send_hex(&bytes);
            m.iter(op);
        }
    }
}

// =====================================================================
// P6 - minibuffer machine (03 §P6): mid-prompt echo-row comparisons
// (echo budget law asserted) + committed final grids. All prompts are
// CLOSED at case end.
// =====================================================================

fn p6_setup(tag: &str) -> (Session, M, String) {
    // SHORT seeded dir (echo budget, gate J3): /tmp/e<pid*100+id><tag>
    let id = SEQN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = format!(
        "/tmp/e{}{}",
        std::process::id() as u64 % 100_000 * 100 + id as u64,
        tag
    );
    std::fs::create_dir_all(&dir).expect("p6 dir");
    std::fs::create_dir(format!("{}/sub", dir)).expect("p6 sub");
    let seed = format!("{}/seed.txt", dir);
    std::fs::write(&seed, "abcdef\nxyz\n").expect("p6 seed");
    std::fs::write(format!("{}/a1.txt", dir), "one\n").expect("p6 a1");
    std::fs::write(format!("{}/a2.txt", dir), "two\n").expect("p6 a2");
    std::fs::write(format!("{}/zzq.txt", dir), "three\n").expect("p6 zzq");
    let s = Session::start(&[seed.as_str()]);
    let m = M::new(
        &seed,
        "abcdef\nxyz\n",
        &format!("{}/out.txt", dir),
        &format!("{}/fresh.txt", dir),
        &format!("{}/miss-x.txt", dir),
        PANE_W,
        24,
    );
    (s, m, dir)
}

// The MID-PROMPT surface (02 §3, 05 §11): THE ECHO ROW compared in
// FULL (prompt+input+list == a pane row, not just its tail - the
// echo budget law makes it one physical row). The tree rows are NOT
// compared mid-prompt (a full-screen CLEAR+repaint can land inside
// the capture window); the committed FINAL grid comparison covers
// the whole tree.
fn check_mid(s: &Session, rd: &Rd, tag: &str, w: usize) {
    // The echo is ONE logical row; over 80 cells it WRAPS onto the
    // pane's trailing rows (the tree shifts up - tree rows are NOT
    // compared mid-prompt, 05 §11). v1 bounds the echo to 2 rows
    // (the budget law, 05 §10); every physical chunk row is compared.
    let e = rd.echo();
    assert!(
        e.chars().count() <= 2 * w,
        "echo budget {} cells > 2 rows (tag {})",
        e.chars().count(),
        tag
    );
    let cs: Vec<char> = e.chars().collect();
    let mut chunks: Vec<String> = vec![];
    let mut i = 0;
    while i < cs.len() {
        let n = w.min(cs.len() - i);
        chunks.push(cs[i..i + n].iter().collect());
        i += n;
    }
    if chunks.is_empty() {
        chunks.push(String::new());
    }
    let lastc = chunks[chunks.len() - 1].trim_end().to_string();
    // POLL for the echo's LAST chunk (the echo appears once the keys
    // are consumed; polling for just any nonblank last row would race
    // the tree repaint - 05 §8), then compare EVERY chunk row.
    let pane = s.capture_poll(|p| {
        p.lines().last().map(|l| l.trim_end() == lastc).unwrap_or(false)
    });
    let rows: Vec<&str> = pane.lines().collect();
    // the poll TARGET is the echo itself: if it never appeared, the
    // mid state is unobservable (e.g. a closing C-g whose echo is
    // painted over by the main-loop repaint that follows) - fail
    // with the pane, never a silent pass
    if !rows.iter().any(|l| l.trim_end() == lastc) {
        panic!(
            "mid echo tag {} NEVER appeared\\n expected last chunk: {:?}\\n pane: {:#?}",
            tag, lastc, rows
        );
    }
    for (k, ch) in chunks.iter().rev().enumerate() {
        let want = rows[rows.len() - 1 - k].trim_end();
        assert_eq!(
            ch.trim_end(),
            want,
            "mid echo wrap row -{} tag {}\n pane: {:#?}",
            k + 1,
            tag,
            rows
        );
    }
}

// Drive a PromptRun with a mid-prompt echo capture at mid_bytes.
fn drive_mid(m: &mut M, s: &Session, r: PromptRun, mid: impl FnOnce(&Rd)) {
    m.begin_cmd();
    let mut rd = Rd::new(&r.prompt, &r.cands, r.file_comp);
    let pre = r.prefix_len.min(r.bytes.len());
    let cut = r.mid_bytes.max(pre).min(r.bytes.len());
    s.send_hex(&r.bytes[..cut]);
    rd.feed(&r.bytes[pre..cut]);
    settle(SETTLE_PHASE_MS);
    mid(&rd);
    s.send_hex(&r.bytes[cut..]);
    rd.feed(&r.bytes[cut..]);
    let extra = m.end_cmd_ret(r.lead, move |mm| (r.apply)(mm, rd.sel.clone(), rd.cancelled));
    let mut after = rd.after.clone();
    after.extend(extra);
    run_tail(m, &after);
    settle(SETTLE_CHECK_MS);
}

fn p6_plain(m: &mut M, s: &Session, op: &Op) {
    run_op(m, s, op);
    settle(SETTLE_CHECK_MS);
}

fn p6_isearch(m: &mut M, s: &Session, tail: &[u8]) {
    let mut bytes = vec![0x13];
    bytes.extend(tail);
    s.send_hex(&bytes);
    let t = tail.to_vec();
    m.run_cmd(0x13, move |m| m.isearch(&t));
    settle(SETTLE_CHECK_MS);
}

fn p6_final(m: &M, s: &Session, tag: &str) {
    let pane = s.capture();
    if let Some((r, mm, pp)) = grid_diff(&m.grid, &pane) {
        panic!(
            "p6[{}] final\n{}\n pane lines: {:#?}",
            tag,
            diff_report(tag, r, &mm, &pp, &m.grid, &pane),
            pane.lines().collect::<Vec<_>>()
        );
    }
    let sane = comb_sane(s, PANE_W);
    assert!(sane.is_none(), "p6[{}] sane: {:?}", tag, sane);
}

fn p6_ff(bytes: Vec<u8>, mid_bytes: usize) -> PromptRun {
    PromptRun {
        bytes,
        prefix_len: 2,
        lead: 0x18,
        prompt: "Find file: ".to_string(),
        cands: vec![],
        file_comp: true,
        mid_bytes,
        apply: Box::new(|m, sel, _c| {
                    m.apply_find(&sel);
                    vec![]
                }),
    }
}

fn p6_swb(cands: Vec<String>, tail: &[u8], mid_bytes: usize) -> PromptRun {
    let mut bytes = vec![0x18, 0x62];
    bytes.extend(tail);
    PromptRun {
        bytes,
        prefix_len: 2,
        lead: 0x18,
        prompt: "Switch to buffer: ".to_string(),
        cands,
        file_comp: false,
        mid_bytes,
        apply: Box::new(|m, sel, _c| {
                    m.apply_swb(&sel, false);
                    vec![]
                }),
    }
}

fn p6_kill(m: &M, ans: &str, mid_bytes: usize) -> PromptRun {
    let name = m.bufs[m.bi()].name.clone();
    let mut bytes = vec![0x18, 0x6B];
    bytes.extend(ans.as_bytes());
    bytes.push(0x0D);
    let mut tb = ans.as_bytes().to_vec();
    tb.push(0x0D);
    PromptRun {
        bytes,
        prefix_len: 2,
        lead: 0x18,
        prompt: format!("Kill buffer {}? (yes|no) ", name),
        cands: vec![],
        file_comp: false,
        mid_bytes,
        apply: Box::new(move |m, sel, _c| m.apply_kill(&sel, &tb)),
    }
}

#[test]
fn p6_minibuffer_machine() {
    // 1. file completion MANY: dir/ typed (mid: prompt+input echo),
    //    Tab (mid: prompt+input+LIST echo), Enter on the DIR path ->
    //    read fails: msg carries the errno map (02 §4, gate m8)
    {
        let (s, mut m, dir) = p6_setup("many");
        let mut bytes = vec![0x18, 0x06];
        bytes.extend(format!("{}/", dir).as_bytes());
        let cut = bytes.len();
        bytes.push(0x09);
        bytes.push(0x0D);
        // capture BEFORE the Tab: echo = prompt + input
        drive_mid(&mut m, &s, p6_ff(bytes.clone(), cut), |rd| {
            check_mid(&s, rd, "many-dir", PANE_W)
        });
        p6_final(&m, &s, "many-tab");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 2. the LIST echo itself: dir/ + Tab captured AFTER the Tab
    //    (full row = prompt + input + "  cand cand ...")
    {
        let (s, mut m, dir) = p6_setup("list");
        let mut bytes = vec![0x18, 0x06];
        bytes.extend(format!("{}/", dir).as_bytes());
        let mid = bytes.len() + 1; // after the Tab
        bytes.push(0x09);
        bytes.push(0x0D);
        drive_mid(&mut m, &s, p6_ff(bytes, mid), |rd| {
            check_mid(&s, rd, "many-list", PANE_W)
        });
        p6_final(&m, &s, "list-enter");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 3. common-prefix EXTEND: dir/a + Tab (mid: input extended to a1,
    //    list [a1.txt a2.txt]) + Enter OPENS a1.txt
    {
        let (s, mut m, dir) = p6_setup("cp");
        let mut bytes = vec![0x18, 0x06];
        bytes.extend(format!("{}/a", dir).as_bytes());
        let mid = bytes.len() + 1; // after the Tab
        bytes.push(0x09);
        bytes.push(0x0D);
        drive_mid(&mut m, &s, p6_ff(bytes, mid), |rd| {
            check_mid(&s, rd, "cp-tab", PANE_W)
        });
        p6_final(&m, &s, "cp-enter");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 4. SINGLE hit: dir/zzq + Tab (mid: ".txt" appended, NO list) +
    //    Enter opens zzq.txt
    {
        let (s, mut m, dir) = p6_setup("one");
        let mut bytes = vec![0x18, 0x06];
        bytes.extend(format!("{}/zzq", dir).as_bytes());
        let mid = bytes.len() + 1;
        bytes.push(0x09);
        bytes.push(0x0D);
        drive_mid(&mut m, &s, p6_ff(bytes, mid), |rd| {
            check_mid(&s, rd, "one-tab", PANE_W)
        });
        p6_final(&m, &s, "one-enter");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 5. buffer completion + CARET SPAN select: create `nb`, Tab
    //    (mid: list echo), C-b (caret onto `nb`'s span), Enter
    {
        let (s, mut m, dir) = p6_setup("span");
        p6_plain(&mut m, &s, &Op::SwbName(1)); // CREATE `nb` (two names)
        let names = m.bnames();
        drive_mid(&mut m, &s, p6_swb(names, &[0x09, 0x02, 0x0D], 3), |rd| {
            check_mid(&s, rd, "span-tab", PANE_W)
        });
        p6_final(&m, &s, "span-enter");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 6. buffer list + Enter at the caret END (past every span) = the
    //    typed input "" -> Cancelled (main.rs:414)
    {
        let (s, mut m, dir) = p6_setup("listcancel");
        p6_plain(&mut m, &s, &Op::SwbName(1));
        let names = m.bnames();
        drive_mid(&mut m, &s, p6_swb(names, &[0x09, 0x0D], 3), |rd| {
            check_mid(&s, rd, "lc-tab", PANE_W)
        });
        p6_final(&m, &s, "lc-enter");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 7. CYCLE-CANCEL (the designed quirk): listform C-g -> the empty
    //    return takes the CYCLE branch, NOT Cancelled (main.rs:410-413,
    //    gate-verified). Mid echo = prompt + " [Cancelled]"; the
    //    scripted Enter then flows through the main loop (newline
    //    insert in the cycled buffer).
    {
        let (s, mut m, dir) = p6_setup("cycle");
        p6_plain(&mut m, &s, &Op::SwbName(1)); // CREATE `nb`: cycle 1->0
        let names = m.bnames();
        let mut bytes = vec![0x18, 0x02];
        bytes.extend(&[0x07, 0x0D]);
        let r = PromptRun {
            bytes,
            prefix_len: 2,
            lead: 0x18,
            prompt: format!("Buffers: {} Enter=next, name: ", names.join(" ")),
            cands: names,
            file_comp: false,
            // OBSERVABLE mid state = the prompt (cut before the C-g).
            // The " [Cancelled]" echo IS modeled, but production's
            // cancel triggers a full-screen repaint (msg "Cancelled"
            // lands on the status row), so the suffixed echo is
            // ephemeral on the physical surface (05 §11 addendum).
            mid_bytes: 2,
            apply: Box::new(|m, sel, _c| {
                    m.apply_swb(&sel, true);
                    vec![]
                }),
        };
        drive_mid(&mut m, &s, r, |rd| check_mid(&s, rd, "cycle-cg", PANE_W));
        p6_final(&m, &s, "cycle-enter");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 8. cancel echo on Write: junk + C-g (mid echo [Cancelled]) +
    //    Enter (flows through the main loop: newline INSERT - the
    //    msg is RETAINED, write-cancel is silent, main.rs:320)
    {
        let (s, mut m, dir) = p6_setup("wcancel");
        let mut bytes = vec![0x18, 0x17];
        bytes.extend(b"nope");
        // OBSERVABLE mid = prompt + typed input (the C-g-committed
        // "[Cancelled]" echo is ephemeral: the full-screen repaint
        // follows at once, 05 §11 addendum)
        let mid = bytes.len();
        bytes.push(0x07);
        bytes.push(0x0D);
        let r = PromptRun {
            bytes,
            prefix_len: 2,
            lead: 0x18,
            prompt: "Write file: ".to_string(),
            cands: vec![],
            file_comp: true,
            mid_bytes: mid,
            apply: Box::new(|m, sel, _c| {
                    m.apply_write(&sel);
                    vec![]
                }),
        };
        drive_mid(&mut m, &s, r, |rd| check_mid(&s, rd, "w-cancel", PANE_W));
        p6_final(&m, &s, "w-enter");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 9. kill-confirm YES (MODIFIED buffer): mid echo = the kill
    //    prompt with "yes" typed
    {
        let (s, mut m, dir) = p6_setup("klivy");
        p6_plain(&mut m, &s, &Op::Ty('x'));
        let r = p6_kill(&m, "yes", 5); // after "yes" (2+3)
        drive_mid(&mut m, &s, r, |rd| check_mid(&s, rd, "kill-prompt", PANE_W));
        p6_final(&m, &s, "kill-yes");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 10. kill-confirm NO: Cancelled; the scripted "no" is consumed
    //     by the reader (prompt shown: buffer modified)
    {
        let (s, mut m, dir) = p6_setup("killno");
        p6_plain(&mut m, &s, &Op::Ty('x'));
        let r = p6_kill(&m, "no", 4);
        drive_mid(&mut m, &s, r, |rd| check_mid(&s, rd, "kill-no-prompt", PANE_W));
        p6_final(&m, &s, "kill-no");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 11. kill on an UNMODIFIED buffer: NO prompt - the scripted
    //     answer ECHOES into the fresh buffer via the main loop
    {
        let (s, mut m, dir) = p6_setup("killunmod");
        let r = p6_kill(&m, "yes", 0);
        s.send_hex(&r.bytes.clone()); // the model run_prompt does NOT
        m.run_prompt(r);              // send: the driver sends (01 §3)
        settle(SETTLE_CHECK_MS);
        p6_final(&m, &s, "kill-unmod");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 12. isearch with DEL pop + MISS drop: a b DEL c Enter
    {
        let (s, mut m, dir) = p6_setup("isdel");
        p6_isearch(&mut m, &s, b"ab\x7fc\r");
        p6_final(&m, &s, "is-del");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 13. isearch total MISS + C-g: point restored, Cancelled
    {
        let (s, mut m, dir) = p6_setup("iscg");
        p6_isearch(&mut m, &s, b"z\x07");
        p6_final(&m, &s, "is-cg");
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // 14. goto-line full matrix (mid echo: typed digits visible)
    {
        let (s, mut m, dir) = p6_setup("gl");
        for k in 0..5usize {
            let q = ["1", "0", "999", "xy", ""][k];
            let mut bytes = vec![0x0C];
            bytes.extend(q.as_bytes());
            let mid = bytes.len(); // after the digits (empty q: mid=1)
            bytes.push(0x0D);
            let r = PromptRun {
                bytes,
                prefix_len: 1,
                lead: 0x0C,
                prompt: "Go to line: ".to_string(),
                cands: vec![],
                file_comp: false,
                mid_bytes: mid,
                apply: Box::new(|m, sel, _c| {
                    m.apply_gl(&sel);
                    vec![]
                }),
            };
            drive_mid(&mut m, &s, r, |rd| check_mid(&s, rd, "gl-mid", PANE_W));
            p6_final(&m, &s, &format!("gl-{}", k));
        }
        s.kill();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn seed_chars() -> impl Strategy<Value = char> {
    prop::sample::select(
        ('a'..='z').chain('A'..='Z').chain([' ', '1', '.', '-', ';', ',']).collect::<Vec<char>>(),
    )
}

fn seed_text() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::collection::vec(seed_chars(), 1..20usize), 1..4usize)
        .prop_map(|ls| ls.into_iter().map(|l| l.into_iter().collect::<String>()).collect::<Vec<String>>().join("\n"))
}

fn p5_ops() -> impl Strategy<Value = Vec<Op>> {
    let tc = prop::sample::select(vec!['a', 'b', 'x', 'Q', '.', '-', ';', '\n', ' ']);
    let one = prop_oneof![
        8 => tc.prop_map(Op::Ty),
        4 => Just(Op::Bs),
        3 => Just(Op::Cd),
        3 => Just(Op::Ck),
        2 => Just(Op::Cy),
        2 => Just(Op::Ca),
        2 => Just(Op::Ce),
        2 => Just(Op::Cb),
        2 => Just(Op::Cf),
        2 => Just(Op::Cn),
        2 => Just(Op::Cp),
        1 => Just(Op::Co),
        1 => Just(Op::Ct),
        1 => Just(Op::Cspc),
        1 => Just(Op::Mb),
        1 => Just(Op::Mf),
        1 => Just(Op::Mlt),
        1 => Just(Op::Mgt),
        2 => prop::sample::select(vec![0x41u8, 0x42, 0x43, 0x44]).prop_map(Op::Arr),
        3 => Just(Op::Undo),
        2 => Just(Op::Cg),
        1 => prop::sample::select(vec!['a', 'x', '.', ';']).prop_map(Op::Quote),
        4 => Just(Op::Split2),
        3 => Just(Op::Split3),
        3 => Just(Op::Other),
        2 => Just(Op::DelOther),
        1 => Just(Op::PrefixG),
        1 => Just(Op::MetaG),
        3 => (0usize..3).prop_map(Op::SwbName),
        1 => Just(Op::SwbTabEnter),
        1 => Just(Op::SwbEmpty),
        1 => Just(Op::SwbCancel),
        2 => Just(Op::SwbListEnter),
        1 => Just(Op::SwbListCancel),
        2 => Just(Op::Wf),
        2 => Just(Op::Sf),
        1 => Just(Op::FiSeed),
        1 => Just(Op::FiMiss),
        1 => Just(Op::FiNew),
        1 => Just(Op::IfSeed),
        1 => Just(Op::KillYes),
        1 => Just(Op::KillNo),
        2 => (0usize..5).prop_map(Op::Gl),
        1 => Just(Op::RfFound),
        1 => Just(Op::RfMiss),
        1 => Just(Op::IsEnter),
        1 => Just(Op::IsCg),
    ];
    prop::collection::vec(one, 1..14usize)
}

proptest! {
    #![proptest_config(cfg(24))]

    #[test]
    fn p5_full_pane(seed in seed_text(), ops in p5_ops()) {
        let (seed_p, out_p, new_p) = paths("p5");
        let miss_p = "/tmp/emiss-x.txt".to_string(); // short: ENOENT label+tail fit 80 (P1 tail law)
        std::fs::write(&seed_p, &seed).expect("seed write");
        std::fs::write(&new_p, "fresh2 content\n").expect("new write");
        let s = Session::start(&[seed_p.as_str()]);
        let mut m = M::new(&seed_p, &seed, &out_p, &new_p, &miss_p, PANE_W, 24);
        let mut log = String::new();
        for (i, op) in ops.iter().enumerate() {
            if matches!(op, Op::Sf)
                && m.bufs[m.bi()].filename.as_deref() == Some(seed_p.as_str())
            {
                log.push_str("SfSkip "); // generator rule (02 §4, gate J2)
                continue;
            }
            run_op(&mut m, &s, op);
            log.push_str(&format!("{:?} ", op));
            if [2usize, 6, 10].contains(&(i + 1)) {
                settle(SETTLE_CHECK_MS);
                let pane = s.capture();
                if let Some((r, mm, pp)) = grid_diff(&m.grid, &pane) {
                    let rep = diff_report(&log, r, &mm, &pp, &m.grid, &pane);
                    s.kill();
                    prop_assert!(
                        false,
                        "checkpoint{} {}\nseed file: {:?}\nmodel: {:#?}\npane: {:#?}",
                        i + 1,
                        rep,
                        std::fs::read_to_string(&seed_p).unwrap_or_default(),
                        m.grid,
                        pane.lines().collect::<Vec<_>>()
                    );
                }
            }
        }
        settle(SETTLE_BURST_MS);
        let pane = s.capture();
        if let Some((r, mm, pp)) = grid_diff(&m.grid, &pane) {
            let rep = diff_report(&log, r, &mm, &pp, &m.grid, &pane);
            s.kill();
            prop_assert!(
                false,
                "final {}\nseed file: {:?}\nmodel: {:#?}\npane: {:#?}",
                rep,
                std::fs::read_to_string(&seed_p).unwrap_or_default(),
                m.grid,
                pane.lines().collect::<Vec<_>>()
            );
        }
        let sane = comb_sane(&s, PANE_W);
        prop_assert!(sane.is_none(), "ops[{}] sane: {:?}", log, sane.unwrap_or_default());
        // status-tail parse: valid ONLY on an untruncated row (long
        // messages truncate at 80: 05 §12)
        let last = last_line(&pane);
        if last.chars().count() < PANE_W {
            prop_assert!(
                parse_status(&last),
                "ops[{}] status tail tail[{}]",
                log,
                pane_tail(&pane)
            );
        }
        // invariant 4: seed untouched (Sf never bound to it)
        let seed_now = std::fs::read_to_string(&seed_p).unwrap_or_default();
        prop_assert_eq!(seed_now, seed, "seed file mutated; ops[{}]", log);
        // invariant 5: last write per path == file content
        let mut seen: Vec<(String, String)> = vec![];
        for (p, t) in m.writes.iter() {
            if let Some(e) = seen.iter_mut().find(|(q, _)| q == p) {
                e.1 = t.clone();
            } else {
                seen.push((p.clone(), t.clone()));
            }
        }
        for (p, t) in seen.iter() {
            let got = std::fs::read_to_string(p).unwrap_or_default();
            prop_assert_eq!(&got, t, "write-event content {} ops[{}]", p, log);
        }
        s.kill();
        let _ = std::fs::remove_file(&seed_p);
        let _ = std::fs::remove_file(&out_p);
        let _ = std::fs::remove_file(&new_p);
    }
}

// =====================================================================
// P7 - split x prefix matrix (03 §P7): enumerated preludes x commands,
// grid compared after EVERY op. Deterministic, single #[test].
// =====================================================================

fn p7_run(pn: &str, cn: &str, pre: &[Op], cmd: Op, w: usize, h: usize) {
    let (seed_p, out_p, new_p) = paths("p7");
    let miss_p = seed_p.replace("-seed.txt", "-miss.txt");
    let seed = "l1 alpha\nl2\n";
    std::fs::write(&seed_p, seed).expect("p7 seed");
    std::fs::write(&new_p, "n\n").expect("p7 new");
    let s = if (w, h) == (PANE_W, 24) {
        Session::start(&[seed_p.as_str()])
    } else {
        Session::start_size(&[seed_p.as_str()], w, h)
    };
    let mut m = M::new(&seed_p, seed, &out_p, &new_p, &miss_p, w, h);
    let mut all = vec![Op::Ty('x')]; // the fixed modify prelude (kill prompt)
    all.extend(pre.iter().copied());
    all.push(cmd);
    for (i, op) in all.iter().enumerate() {
        run_op(&mut m, &s, op);
        settle(SETTLE_CHECK_MS);
        let pane = s.capture();
        if let Some((r, mm, pp)) = grid_diff(&m.grid, &pane) {
            panic!(
                "p7[{}][{}] op {} {:?}\n{}",
                pn,
                cn,
                i,
                op,
                diff_report(&format!("prelude {} cmd {}", pn, cn), r, &mm, &pp, &m.grid, &pane)
            );
        }
    }
    let sane = comb_sane(&s, w);
    assert!(sane.is_none(), "p7[{}][{}] sane: {:?}", pn, cn, sane);
    s.kill();
    let _ = std::fs::remove_file(&seed_p);
    let _ = std::fs::remove_file(&out_p);
    let _ = std::fs::remove_file(&new_p);
}

#[test]
fn p7_split_prefix_matrix() {
    let preludes: Vec<(&str, Vec<Op>)> = vec![
        ("[]", vec![]),
        ("[C-x 2]", vec![Op::Split2]),
        ("[C-x 3]", vec![Op::Split3]),
        ("[C-x 2 C-x 2]", vec![Op::Split2, Op::Split2]),
        ("[C-x 3 C-x o]", vec![Op::Split3, Op::Other]),
    ];
    let cmds: Vec<(&str, Op)> = vec![
        ("C-x o", Op::Other),
        ("C-x 1", Op::DelOther),
        ("C-x 2", Op::Split2),
        ("C-x b *demo*", Op::SwbName(0)),
        ("C-x C-b Enter", Op::SwbListEnter),
        ("C-x C-g", Op::PrefixG),
        ("C-x k yes", Op::KillYes),
        ("C-l 1", Op::Gl(0)),
    ];
    for (pn, pre) in preludes {
        for (cn, cmd) in &cmds {
            p7_run(pn, cn, &pre, *cmd, PANE_W, 24);
        }
    }
    // 60x20 submatrix (04 §1): natural users of Session::start_size
    p7_run("[C-x 2]", "C-x 2 @60x20", &[Op::Split2], Op::Split2, 60, 20);
    p7_run("[C-x 2]", "C-x o @60x20", &[Op::Split2], Op::Other, 60, 20);
}

// Resize x splits mid-sequence (P7b): the natural user of
// Session::resize (04 §1); grid compared after every op, 80x24 -> 60x20.
#[test]
fn p7b_resize_splits() {
    let (seed_p, out_p, new_p) = paths("p7b");
    let miss_p = seed_p.replace("-seed.txt", "-miss.txt");
    let seed = "l1 alpha\nl2\nl3\n";
    std::fs::write(&seed_p, seed).expect("p7b seed");
    std::fs::write(&new_p, "n\n").expect("p7b new");
    let s = Session::start(&[seed_p.as_str()]);
    let mut m = M::new(&seed_p, seed, &out_p, &new_p, &miss_p, PANE_W, 24);
    let grid_check = |m: &M, s: &Session, tag: &str| {
        settle(SETTLE_CHECK_MS);
        let pane = s.capture();
        if let Some((r, mm, pp)) = grid_diff(&m.grid, &pane) {
            panic!("{}", diff_report(tag, r, &mm, &pp, &m.grid, &pane));
        }
    };
    // typed text through Session::send_text (the -H law, PBTF-003)
    s.send_text("x");
    m.iter(&Op::Ty('x'));
    grid_check(&m, &s, "p7b type");
    run_op(&mut m, &s, &Op::Split2);
    grid_check(&m, &s, "p7b split80");
    s.resize(60, 20);
    m.w = 60;
    m.h = 20;
    settle(SETTLE_BURST_MS);
    run_op(&mut m, &s, &Op::Other);
    grid_check(&m, &s, "p7b other60");
    run_op(&mut m, &s, &Op::Split2);
    grid_check(&m, &s, "p7b split60");
    run_op(&mut m, &s, &Op::DelOther);
    grid_check(&m, &s, "p7b collapse60");
    let sane = comb_sane(&s, 60);
    assert!(sane.is_none(), "p7b sane: {:?}", sane);
    s.kill();
    let _ = std::fs::remove_file(&seed_p);
    let _ = std::fs::remove_file(&out_p);
    let _ = std::fs::remove_file(&new_p);
}



