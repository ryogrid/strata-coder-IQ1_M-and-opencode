// Shared tmux harness for the emur PBT properties.
// Design: ../../test-design/pbt-design/04-tmux-harness.md
// Used by tests/tmux_fuzz.rs ONLY (buffer_model is tmux-free), so every
// pub item must be used by tmux_fuzz - the zero-warning gate enforces it
// (no #[allow]).

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Once;
use std::thread;
use std::time::Duration;

pub const PANE_W: usize = 80;
pub const SETTLE_START_MS: u64 = 600;
pub const SETTLE_BURST_MS: u64 = 800;
pub const SETTLE_PHASE_MS: u64 = 400;
pub const SETTLE_CHECK_MS: u64 = 500;

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

fn emur_bin() -> PathBuf {
    let pb = match std::env::var("EMUR_BIN") {
        Ok(p) => PathBuf::from(p),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/debug/emur"),
    };
    assert!(
        pb.exists(),
        "emur binary not found at {} - run: cargo build --bin emur",
        pb.display()
    );
    pb
}

fn tmux(args: &[&str]) -> std::process::Output {
    Command::new("tmux")
        .args(args)
        .output()
        .expect("tmux binary not found (required by the tmux PBT properties)")
}

fn tmux_ok(args: &[&str]) -> bool {
    tmux(args).status.success()
}

static SWEEP: Once = Once::new();
static SEQ: AtomicUsize = AtomicUsize::new(0);

// Kill pbt-* sessions leaked by killed runners / fork-mode timeouts.
pub fn sweep_leftovers() {
    SWEEP.call_once(|| {
        let out = tmux(&["list-sessions", "-F", "#{session_name}"]);
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if line.starts_with("pbt-") {
                    tmux(&["kill-session", "-t", line]);
                }
            }
            // Killing the LAST session shuts the tmux server down; a
            // new-session issued immediately can connect to the dying
            // server and fail with "server exited unexpectedly" (observed
            // 2026-10-25, PBTF-002). Let the shutdown finish, and let
            // new-session below re-create a clean server.
            settle(SETTLE_START_MS);
        }
    });
}

pub struct Session {
    pub name: String,
}

impl Session {
    // Fixed-pattern argv (no shell metacharacters, no apostrophes - 04 §1).
    pub fn start(args: &[&str]) -> Session {
        sweep_leftovers();
        let name = format!(
            "pbt-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        );
        let mut cmd = emur_bin().display().to_string();
        for a in args {
            cmd.push(' ');
            cmd.push_str(a);
        }
        let sh = format!("{}; echo EMU_EXITED $?; sleep 30", cmd);
        let cmd_args: Vec<&str> =
            vec!["new-session", "-d", "-s", &name, "-x", "80", "-y", "24", "sh", "-c", &sh];
        let mut out = tmux(&cmd_args);
        // If the sweep above just killed the server's LAST session, the
        // server shutdown races this first client ("server exited
        // unexpectedly", PBTF-002): retry once after a settle.
        if !out.status.success() {
            settle(SETTLE_START_MS);
            out = tmux(&cmd_args);
        }
        assert!(
            out.status.success(),
            "tmux new-session failed for {}: {}",
            name,
            String::from_utf8_lossy(&out.stderr).trim()
        );
        settle(SETTLE_START_MS);
        Session { name }
    }

    pub fn send_hex(&self, bytes: &[u8]) {
        let hexs: Vec<String> = bytes.iter().map(|b| format!("{:02x}", b)).collect();
        let mut args: Vec<&str> = vec!["send-keys", "-t", &self.name, "-H"];
        args.extend(hexs.iter().map(|s| s.as_str()));
        assert!(tmux_ok(&args), "send-keys -H failed on {}", self.name);
    }

    // Text goes through send-keys -H (raw hex bytes), NOT -l: a lone ';'
    // argument survives tmux's argv quoting inside a longer string ("X;Y"
    // arrives intact) but is eaten as a command separator when it is the
    // ONLY argument (`send-keys -l -- ';'` sends NOTHING - verified live
    // 2026-10-25, PBTF-003). Hex tokens carry no tmux command syntax;
    // UTF-8 code points arrive as their bytes and emur's read_char_eof
    // reassembles them.
    pub fn send_text(&self, text: &str) {
        self.send_hex(text.as_bytes());
    }

    pub fn resize(&self, w: usize, h: usize) {
        let (ws, hs) = (w.to_string(), h.to_string());
        assert!(
            tmux_ok(&["resize-window", "-t", &self.name, "-x", &ws, "-y", &hs]),
            "resize-window failed on {}",
            self.name
        );
    }

    pub fn capture_bytes(&self) -> Vec<u8> {
        tmux(&["capture-pane", "-p", "-t", &self.name]).stdout
    }

    // Trailing-blank-stripped lines (the e2e `cap` shape); tmux 3.4 trims
    // the padding itself (gate probe), lines keep their internal spaces.
    pub fn capture(&self) -> String {
        let raw = String::from_utf8_lossy(&self.capture_bytes()).into_owned();
        raw.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
    }

    pub fn last_line(pane: &str) -> String {
        pane.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string()
    }

    // Poll-capture helper (04 §4): retry the full capture 3x300ms.
    pub fn capture_poll<F: Fn(&str) -> bool>(&self, pred: F) -> String {
        for _ in 0..3 {
            let c = self.capture();
            if pred(&c) {
                return c;
            }
            settle(300);
        }
        self.capture()
    }

    pub fn kill(&self) {
        tmux(&["kill-session", "-q", "-t", &self.name]);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.kill();
    }
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

// Pane sanity WITHOUT the status-tail check (P4: the status line wraps
// over 3+ rows at narrow widths; the tail parser cannot follow it - see
// the 03 §P4 design-deviation note).
pub fn pane_sane_lax(s: &Session, width: usize) -> Option<String> {
    pane_sane_core(s, width, false)
}

pub fn pane_sane(s: &Session, width: usize) -> Option<String> {
    pane_sane_core(s, width, true)
}

fn pane_sane_core(s: &Session, width: usize, check_status: bool) -> Option<String> {
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
        if line.chars().count() > width {
            return Some(format!(
                "line over width {}: {:?}",
                width,
                line.chars().take(20).collect::<String>()
            ));
        }
    }
    if check_status {
        let last = Session::last_line(&pane);
        if !parse_status(&last) {
            return Some(format!("malformed status line: {:?}", last));
        }
    }
    None
}

// Failure-message tail (04 §7): last 6 pane lines, |-joined.
pub fn pane_tail(pane: &str) -> String {
    pane.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ")
}
