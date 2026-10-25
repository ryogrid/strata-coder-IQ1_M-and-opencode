// Pane-sanity layer (P1-style checks): compiled by tmux_fuzz ONLY
// (pbt-design-combination 04 §1 v2). The combination target's FULL
// grid diff is a strictly stronger surface check, and the P1 status
// TAIL assert is INVALID for long messages (production truncates the
// status row at 80 cols, leaving the tail parser an incomplete
// "(P%" - 05 §12), so combination runs its own sanity (comb_sane).

use super::core::{last_line, parse_status};
use super::sess::Session;

pub fn pane_sane(s: &Session, width: usize) -> Option<String> {
    pane_sane_core(s, width, true)
}

pub fn pane_sane_lax(s: &Session, width: usize) -> Option<String> {
    pane_sane_core(s, width, false)
}

pub fn pane_sane_core(s: &Session, width: usize, check_status: bool) -> Option<String> {
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
        let last = last_line(&pane);
        if !parse_status(&last) {
            return Some(format!("malformed status line: {:?}", last));
        }
    }
    None
}
