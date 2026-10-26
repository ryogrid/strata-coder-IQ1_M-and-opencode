// M-11 (Design Doc D): the ONE binding table. Design Doc D
// (docs/design/keymap-dispatch-table.md) pins every row; the main loop
// reads the next byte in this exact order and dispatches to one method
// per command. The old flat if-else chain (review §M-11) is gone;
// behavior is byte-for-byte the old dispatch (all 89 e2e scenarios).
//
// Read-order invariants (Design Doc D §3.3): PrefixCtrlX/PrefixMeta make
// main read exactly ONE next byte; PrefixArrow makes main read the arrow
// tail byte t (ESC -> 0x5b -> t, M-3). Prefix EOF reads byte 0
// (read_byte): C-x + EOF -> Unbound(0x18, 0) -> the "Undefined command"
// message (M-7); Meta + EOF -> MetaUnbound = silent (as today).

#[derive(PartialEq, Eq, Debug)]
pub enum Cmd {
    PrefixCtrlX,  // main reads n -> ctrl_x_key(n)
    PrefixMeta,   // main reads n -> meta_key(n)
    PrefixArrow,  // Meta 0x5b: main reads t -> arrow_key(t)
    Quit,         // confirm_exit; break on yes (C-c AND C-x C-c)
    SelfInsert,   // main passes the lead byte to read_char_eof
    QuoteInsert,  // C-q: main reads the next byte (EOF -> NUL quirk, Doc A)
    Newline,      // C-j / C-m
    Cancel,       // C-g: msg = Cancelled
    Noop,         // unbound plain byte: SILENT (0x03 lands here after Quit-cancel)
    SetMark,      // C-SPC
    BeginningOfLine, EndOfLine, BackwardChar, ForwardChar,
    NextLine, PreviousLine, BeginningOfBuffer, EndOfBuffer,
    ScrollPageDown, ScrollPageUp, // C-v / M-v: one screenful (point moves; the scroll follows)
    BackwardWord, ForwardWord,
    DeleteBackward, DeleteForward, OpenLine, TransposeChars,
    KillLine, KillRegion, Yank,  // C-k / C-w / C-y
    Undo,         // C-/ (arrives as 0x1f = US, the C-_ code; T-9)
    GotoLine,     // C-l (minibuffer line number)
    Isearch,      // C-s: the incremental-search loop (one method)
    FindBackward, // C-r: THE SAME loop, backward direction (incremental)
    FindFile, Save, WriteFile, InsertFile, KillBuffer,
    SwitchBuffer(bool), // true = Ctrl-x Ctrl-b (list prompt + Enter cycles)
    DeleteOtherWindows, SplitBelow, SplitRight, OtherWindow,
    Arrow(usize), // 0=Up 1=Down 2=Right 3=Left (arrow_key maps t)
    Unbound(u8, u8), // (prefix, byte): C-x unbound msg ONLY (M-7)
    MetaUnbound,  // silent (as today)
}

// Plain (unprefixed) keys. The printable self-insert window is
// 0x20..0x7f plus every b >= 0x80 (UTF-8 lead bytes); DEL (0x7f) is
// delete-backward, NOT self-insert.
pub fn plain_key(b: u8) -> Cmd {
    if b >= 0x80 {
        return Cmd::SelfInsert;
    }
    match b {
        0x03 => Cmd::Quit,
        0x18 => Cmd::PrefixCtrlX,
        0x1b => Cmd::PrefixMeta,
        0x13 => Cmd::Isearch,
        0x12 => Cmd::FindBackward,
        0x16 => Cmd::ScrollPageDown, // C-v (Emacs: scroll-page)
        0x17 => Cmd::KillRegion,     // C-w (Emacs: kill-region; C-x C-w is a DIFFERENT byte pair)
        0x07 => Cmd::Cancel,
        0x00 => Cmd::SetMark,
        0x01 => Cmd::BeginningOfLine,
        0x02 => Cmd::BackwardChar,
        0x06 => Cmd::ForwardChar,
        0x0e => Cmd::NextLine,
        0x10 => Cmd::PreviousLine,
        0x7f => Cmd::DeleteBackward,
        0x04 => Cmd::DeleteForward,
        0x0f => Cmd::OpenLine,
        0x14 => Cmd::TransposeChars,
        0x0b => Cmd::KillLine,
        0x19 => Cmd::Yank,
        0x05 => Cmd::EndOfLine,
        0x0c => Cmd::GotoLine,
        0x11 => Cmd::QuoteInsert,
        0x0a | 0x0d => Cmd::Newline,
        0x1f => Cmd::Undo, // C-/ (xterm-family: Ctrl-/ sends 0x1f, same as C-_)
        0x20..0x7f => Cmd::SelfInsert,
        _ => Cmd::Noop, // unbound control byte: SILENT (no msg, old chain)
    }
}

// Ctrl-x prefix. Unbound -> the M-7 "Undefined command: C-x <byte>" msg.
pub fn ctrl_x_key(n: u8) -> Cmd {
    match n {
        0x03 => Cmd::Quit,
        0x31 => Cmd::DeleteOtherWindows,
        0x32 => Cmd::SplitBelow,
        0x33 => Cmd::SplitRight,
        0x6f => Cmd::OtherWindow,
        0x06 => Cmd::FindFile,
        0x13 => Cmd::Save,
        0x17 => Cmd::WriteFile,
        0x09 => Cmd::InsertFile,
        0x6b => Cmd::KillBuffer,
        0x62 => Cmd::SwitchBuffer(false),
        0x02 => Cmd::SwitchBuffer(true),
        _ => Cmd::Unbound(0x18, n),
    }
}

// Meta (ESC) prefix. Unbound is SILENT (the old chain emitted nothing).
pub fn meta_key(n: u8) -> Cmd {
    match n {
        0x5b => Cmd::PrefixArrow, // ANSI arrow: ESC [ t - main reads t
        0x62 => Cmd::BackwardWord,
        0x66 => Cmd::ForwardWord,
        0x3c => Cmd::BeginningOfBuffer, // C-</C-> arrive as Meta forms
        0x3e => Cmd::EndOfBuffer,
        0x76 => Cmd::ScrollPageUp, // M-v (Emacs: scroll-page-up)
        _ => Cmd::MetaUnbound,
    }
}

// Arrow tail byte t (after ESC [). Unbound t is silent (MetaUnbound).
pub fn arrow_key(t: u8) -> Cmd {
    match t {
        0x41 => Cmd::Arrow(0), // Up
        0x42 => Cmd::Arrow(1), // Down
        0x43 => Cmd::Arrow(2), // Right
        0x44 => Cmd::Arrow(3), // Left
        _ => Cmd::MetaUnbound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plain_table_pins_control_keys() {
        // M-11: spot-pin the plain map (e2e pins the full behavior).
        assert!(plain_key(0x03) == Cmd::Quit);
        assert!(plain_key(0x18) == Cmd::PrefixCtrlX);
        assert!(plain_key(0x1b) == Cmd::PrefixMeta);
        assert!(plain_key(0x13) == Cmd::Isearch);
        assert!(plain_key(0x02) == Cmd::BackwardChar);
        assert!(plain_key(0x7f) == Cmd::DeleteBackward); // DEL, NOT insert
        assert!(plain_key(0x0b) == Cmd::KillLine);
        assert!(plain_key(0x16) == Cmd::ScrollPageDown); // C-v
        assert!(plain_key(0x17) == Cmd::KillRegion); // C-w (plain; C-x C-w is ctrl_x 0x17)
        assert!(plain_key(0x0a) == Cmd::Newline && plain_key(0x0d) == Cmd::Newline);
        assert!(plain_key(0x1f) == Cmd::Undo); // T-9: C-/ (US byte, the C-_ code)
        assert!(plain_key(0x08) == Cmd::Noop); // unbound control: silent
    }
    #[test]
    fn plain_table_pins_self_insert_window() {
        assert!(plain_key(0x20) == Cmd::SelfInsert); // space
        assert!(plain_key(0x7e) == Cmd::SelfInsert); // '~'
        assert!(plain_key(0x7f) != Cmd::SelfInsert); // DEL excluded
        assert!(plain_key(0x80) == Cmd::SelfInsert); // UTF-8 lead bytes
        assert!(plain_key(0xff) == Cmd::SelfInsert);
    }
    #[test]
    fn ctrlx_table_pins_prefix_and_unbound() {
        assert!(ctrl_x_key(0x03) == Cmd::Quit);
        assert!(ctrl_x_key(0x62) == Cmd::SwitchBuffer(false));
        assert!(ctrl_x_key(0x02) == Cmd::SwitchBuffer(true));
        assert!(ctrl_x_key(0x6f) == Cmd::OtherWindow);
        assert!(ctrl_x_key(0x7a) == Cmd::Unbound(0x18, 0x7a)); // M-7 msg
        assert!(ctrl_x_key(0x00) == Cmd::Unbound(0x18, 0x00)); // prefix EOF byte 0
    }
    #[test]
    fn meta_table_pins_arrows_and_silence() {
        assert!(meta_key(0x5b) == Cmd::PrefixArrow); // 3-byte read via main
        assert!(meta_key(0x62) == Cmd::BackwardWord);
        assert!(meta_key(0x3c) == Cmd::BeginningOfBuffer);
        assert!(meta_key(0x76) == Cmd::ScrollPageUp); // M-v
        assert!(meta_key(0x7a) == Cmd::MetaUnbound); // SILENT
        assert!(arrow_key(0x41) == Cmd::Arrow(0));
        assert!(arrow_key(0x44) == Cmd::Arrow(3));
        assert!(arrow_key(0x45) == Cmd::MetaUnbound); // unbound t: silent
    }
}
