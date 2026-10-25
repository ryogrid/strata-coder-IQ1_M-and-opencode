// Shared tmux harness for the emur PBT properties.
// Design: ../../test-design/pbt-design/04-tmux-harness.md +
// pbt-design-combination/04 §1 (three-layer MA-2 split).
//
// tmux_fuzz compiles ALL THREE layers (core + sess + sanity), so every
// pub item must be used by tmux_fuzz (zero-warning gate, no #[allow]).
// The combination target includes core + sess ONLY (direct #[path]
// includes), where the same rule applies per its 04 §1 use map.
// buffer_model stays tmux-free and compiles none of this.

mod core;
mod sanity;
mod sess;

pub use core::{
    cfg, last_line, pane_tail, parse_status, settle, PANE_W, SETTLE_BURST_MS,
    SETTLE_CHECK_MS, SETTLE_PHASE_MS,
};
pub use sanity::{pane_sane, pane_sane_lax};
pub use sess::Session;
