// Pure harness layer: no tmux, no I/O beyond env vars.
// Design: ../../test-design/pbt-design/04-tmux-harness.md +
// pbt-design-combination/04 §1 (MA-2 three-layer split: core/sess/narrow).
// Compiled by tmux_fuzz (via support/mod.rs) AND by combination
// (direct #[path] include) - EVERY pub item must be used in BOTH targets.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

pub const PANE_W: usize = 80;
pub const SETTLE_START_MS: u64 = 600;
pub const SETTLE_BURST_MS: u64 = 800;
pub const SETTLE_PHASE_MS: u64 = 400;
pub const SETTLE_CHECK_MS: u64 = 500;

// Session-name sequence counter (lives here so sess.rs takes it via
// super::core; the tmux command plumbing itself is sess-only).
pub static SEQ: AtomicUsize = AtomicUsize::new(0);

pub fn seq_next() -> usize {
    SEQ.fetch_add(1, Ordering::SeqCst)
}

pub fn settle(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

// Case-count policy: PBT_CASES env overrides the per-property default
// (design 03 Config plumbing; CI sets 8).
pub fn cases(default: u32) -> u32 {
    std::env::var("PBT_CASES")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

pub fn cfg(default_cases: u32) -> proptest::test_runner::Config {
    let mut c = proptest::test_runner::Config::with_cases(cases(default_cases));
    // timeout STAYS 0: proptest FORK mode is DISABLED (design deviation,
    // recorded 03/04 + found-bugs PBTF-002). Rationale (observed): fork
    // children killed on timeout skip the Session Drop guard and leak
    // pbt-* sessions; fork children inherit the parent's completed
    // sweep Once and never sweep themselves; and a child failure is
    // reported to the parent only as "failed in other process" (the
    // assertion detail is lost). In-process cases keep Drop guards
    // active, full failure messages, and serial execution via
    // --test-threads=1; a hung case is bounded by the CI job timeout.
    c.max_shrink_time = 15_000; // shrinks re-run the tmux round-trip (MA-3)
    c.failure_persistence = None;
    c
}

// Status-line shape: "<label> N/M (P%)" (P2's exact assert builds the
// full string; this checks the structural tail - 03 P1 invariant 4).
pub fn parse_status(line: &str) -> bool {
    let t = line.trim_end();
    let Some(body) = t.strip_suffix(')') else { return false };
    let Some(open) = body.rfind('(') else { return false };
    let pct = &body[open + 1..];
    let Some(pn) = pct.strip_suffix('%') else { return false };
    if pn.is_empty() || !pn.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    // label part: the "N/M" token is the last word before " (P%)"
    let Some(nm) = body[..open].trim_end().split(' ').next_back() else { return false };
    let Some((n, m)) = nm.split_once('/') else { return false };
    !n.is_empty()
        && n.chars().all(|c| c.is_ascii_digit())
        && !m.is_empty()
        && m.chars().all(|c| c.is_ascii_digit())
}

// Last non-blank line (free function: pane_sane_core and the harness
// consumers share it; the Session::last_line assoc delegates here).
pub fn last_line(pane: &str) -> String {
    pane.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .to_string()
}

// Failure-message tail (04 §7): last 6 pane lines, |-joined.
pub fn pane_tail(pane: &str) -> String {
    pane.lines()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
}
