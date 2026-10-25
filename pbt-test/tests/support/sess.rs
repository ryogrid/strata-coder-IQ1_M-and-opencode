// Tmux session layer. Compiled by tmux_fuzz (via support/mod.rs) AND by
// combination (direct #[path] include; combination.rs resolves `super::core`
// to its own root `mod core`) - EVERY pub item here is used in BOTH targets.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;

use super::core::{settle, seq_next, SETTLE_START_MS};

pub fn emur_bin() -> PathBuf {
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

pub fn tmux(args: &[&str]) -> std::process::Output {
    Command::new("tmux")
        .args(args)
        .output()
        .expect("tmux binary not found (required by the tmux PBT properties)")
}

pub fn tmux_ok(args: &[&str]) -> bool {
    tmux(args).status.success()
}

static SWEEP: Once = Once::new();

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
        Self::start_size(args, 80, 24)
    }

    // The 80x24 default above routes through here; P7's non-default cells
    // (pbt-design-combination 04 §1) are the natural users of the size
    // parameters, and `resize` keeps the explicit mid-session path.
    pub fn start_size(args: &[&str], w: usize, h: usize) -> Session {
        sweep_leftovers();
        let name = format!("pbt-{}-{}", std::process::id(), seq_next());
        let mut cmd = emur_bin().display().to_string();
        for a in args {
            cmd.push(' ');
            cmd.push_str(a);
        }
        let sh = format!("{}; echo EMU_EXITED $?; sleep 30", cmd);
        let (ws, hs) = (w.to_string(), h.to_string());
        let mut out = tmux(&[
            "new-session", "-d", "-s", &name, "-x", &ws, "-y", &hs, "sh", "-c", &sh,
        ]);
        // If the sweep above just killed the server's LAST session, the
        // server shutdown races this first client ("server exited
        // unexpectedly", PBTF-002): retry once after a settle.
        if !out.status.success() {
            settle(SETTLE_START_MS);
            out = tmux(&[
                "new-session", "-d", "-s", &name, "-x", &ws, "-y", &hs, "sh", "-c", &sh,
            ]);
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
        // tmux `send-keys -H` with NO key args sends ENTER (05 §13):
        // an empty byte list is a driver bug, never a no-op
        assert!(!bytes.is_empty(), "send_hex called with 0 bytes on {}", self.name);
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

    // Poll-capture helper (04 §4 / 05 §8): 4 pred-check captures with
    // 300 ms sleeps (final plain capture on miss = 5 captures total).
    pub fn capture_poll<F: Fn(&str) -> bool>(&self, pred: F) -> String {
        for _ in 0..4 {
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

// The sanity layer (support/sanity.rs) hosts pane_sane/pane_sane_lax.
