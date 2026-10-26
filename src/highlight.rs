// Syntax highlighting: per-character style codes for a buffer's text,
// selected by the buffer/file name (language detected from the extension).
// Styles: 0 plain, 1 keyword, 2 string, 3 comment, 4 number, 5 heading/marker.
// Languages (codes): 0 plain, 1 c-family (rs c cpp h js ts go css),
// 2 hash-family (py sh rb yaml toml), 3 json, 4 markdown, 5 html/xml.

// H-7: style codes as an enum (was magic usize 0..5 shared with
// screen.rs's style_esc). Exhaustive matches in style_esc make adding a
// style a compile-checked change. The numbers in comments preserve the
// historical wire order (0..5).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Plain,   // 0: no SGR (reset)
    Keyword, // 1: cyan
    Str,     // 2: magenta (string/char literals)
    Comment, // 3: green
    Number,  // 4: blue
    Marker,  // 5: bold cyan (markdown headings/list markers, HTML tags)
    Isearch, // 6: yellow bg (the live isearch match; C-s/C-r)
    Region,  // 7: cyan bg (the active region between mark and point)
}

pub fn hchars_of(s: &str) -> Vec<char> {
    let mut cs: Vec<char> = vec![];
    for c in s.chars() {
        cs.push(c);
    }
    cs
}

pub fn heq_at(cs: &[char], i: usize, w: &str) -> bool {
    for (k, wc) in w.chars().enumerate() {
        if i + k >= cs.len() || cs[i + k] != wc {
            return false;
        }
    }
    true
}

// word slice == literal (avoids per-word String allocation; allocation-free:
// chars() is exact-size, zip walks cs without copying)
fn word_eq(cs: &[char], w: &str) -> bool {
    w.chars().count() == cs.len() && w.chars().zip(cs.iter()).all(|(a, b)| a == *b)
}

pub fn hwordc(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()
}

pub fn hletter(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_uppercase() || c == '_'
}

pub fn hdigit(c: char) -> bool {
    c.is_ascii_digit()
}

pub fn detect(name: &str) -> usize {
    let cs = hchars_of(name);
    let n = cs.len();
    let mut start = 0;
    let mut i = 0;
    while i < n {
        if cs[i] == '/' {
            start = i + 1;
        }
        i += 1;
    }
    let mut dot = 0;
    i = start;
    while i < n {
        if cs[i] == '.' {
            dot = i;
        }
        i += 1;
    }
    if dot <= start {
        return 0;
    }
    // H-3: extension matching is ASCII-case-insensitive (FILE.RS is Rust).
    let ext: String = cs[dot + 1..].iter().collect();
    let ext = ext.to_ascii_lowercase();
    let exts1 = vec!["rs", "c", "h", "cpp", "cc", "hpp", "js", "ts", "go", "css"];
    let exts2 = ["py", "sh", "rb", "yaml", "toml"];
    let exts4 = ["md", "markdown"];
    let exts5 = ["html", "xml"];
    for e in exts1.iter() {
        if ext == *e {
            return 1;
        }
    }
    for e in exts2.iter() {
        if ext == *e {
            return 2;
        }
    }
    if ext == "json" {
        return 3;
    }
    for e in exts4.iter() {
        if ext == *e {
            return 4;
        }
    }
    for e in exts5.iter() {
        if ext == *e {
            return 5;
        }
    }
    0
}

pub fn kw(lang: usize, w: &[char]) -> bool {
    if lang == 1 {
        return word_eq(w, "fn") || word_eq(w, "let") || word_eq(w, "const") || word_eq(w, "if") ||
            word_eq(w, "else") ||
            word_eq(w, "for") || word_eq(w, "while") || word_eq(w, "return") || word_eq(w, "struct") ||
            word_eq(w, "enum") ||
            word_eq(w, "impl") || word_eq(w, "pub") || word_eq(w, "use") || word_eq(w, "match") ||
            word_eq(w, "case") ||
            word_eq(w, "switch") || word_eq(w, "break") || word_eq(w, "continue") || word_eq(w, "true") ||
            word_eq(w, "false") || word_eq(w, "new") || word_eq(w, "class") || word_eq(w, "void") ||
            word_eq(w, "int") ||
            word_eq(w, "char") || word_eq(w, "float") || word_eq(w, "double") || word_eq(w, "bool") ||
            word_eq(w, "auto") ||
            word_eq(w, "package") || word_eq(w, "import") || word_eq(w, "func") || word_eq(w, "var") ||
            word_eq(w, "type");
    }
    if lang == 2 {
        return word_eq(w, "def") || word_eq(w, "if") || word_eq(w, "elif") || word_eq(w, "else") ||
            word_eq(w, "for") ||
            word_eq(w, "while") || word_eq(w, "return") || word_eq(w, "import") || word_eq(w, "from") ||
            word_eq(w, "class") ||
            word_eq(w, "None") || word_eq(w, "True") || word_eq(w, "False") || word_eq(w, "lambda") ||
            word_eq(w, "in") ||
            word_eq(w, "not") || word_eq(w, "and") || word_eq(w, "or") || word_eq(w, "then") ||
            word_eq(w, "fi") ||
            word_eq(w, "done") || word_eq(w, "case") || word_eq(w, "esac") || word_eq(w, "echo") ||
            word_eq(w, "export") ||
            word_eq(w, "local") || word_eq(w, "function");
    }
    if lang == 3 {
        return word_eq(w, "true") || word_eq(w, "false") || word_eq(w, "null");
    }
    false
}

pub fn md_styles(cs: &[char], st: &mut [Style]) {
    let n = cs.len();
    let mut i = 0;
    while i < n {
        if cs[i] == '\n' {
            i += 1;
            continue;
        }
        while i < n && cs[i] == ' ' {
            i += 1;
        }
        if i >= n {
            return;
        }
        if cs[i] == '#' {
            while i < n && cs[i] != '\n' {
                st[i] = Style::Marker;
                i += 1;
            }
            continue;
        }
        if i < n && (cs[i] == '-' || cs[i] == '*' || cs[i] == '+') && i + 1 < n && cs[i + 1] == ' ' {
            st[i] = Style::Marker;
            i += 1;
        }
        if i < n && cs[i] == '`' {
            let mut j = i + 1;
            while j < n && cs[j] != '`' && cs[j] != '\n' {
                j += 1;
            }
            if j < n && cs[j] == '`' {
                j += 1;
            }
            let mut k = i;
            while k < j {
                st[k] = Style::Str;
                k += 1;
            }
            i = j;
            continue;
        }
        i += 1;
    }
}

// H-2 (Design Doc B): in the c-family (lang 1), a tick opens a string only
// when (a) its closing tick is on the SAME line, (b) at least 1 char sits
// between (j >= i+2), and (c) the content looks string-like (no space,
// '<', '(', ';', '&'). Lifetimes like 'de or &'de fail (a) or (c) and stay
// unstyled; char literals like 'a' get string style (accepted).
fn lang1_tick_opens_string(cs: &[char], i: usize) -> bool {
    let n = cs.len();
    let mut j = i + 1;
    while j < n && cs[j] != '\'' && cs[j] != '\n' {
        if cs[j] == '\\' {
            j += 1; // skip the escaped char (e.g. '\'')
        }
        j += 1;
    }
    if j >= n || cs[j] != '\'' {
        return false; // (a) no closing tick on this line
    }
    if j < i + 2 {
        return false; // (b) empty '' is not a string body
    }
    let mut k = i + 1;
    while k < j {
        let d = cs[k];
        if d == ' ' || d == '<' || d == '(' || d == ';' || d == '&' {
            return false; // (c) not string-like
        }
        k += 1;
    }
    true
}

pub fn styles(cs: &[char], lang: usize) -> Vec<Style> {
    let n = cs.len();
    let mut st: Vec<Style> = vec![];
    let mut i = 0;
    while i < n {
        st.push(Style::Plain);
        i += 1;
    }
    if lang == 0 {
        return st;
    }
    if lang == 4 {
        md_styles(cs, &mut st);
        return st;
    }
    i = 0;
    while i < n {
        let c = cs[i];
        let is_line_comment = (lang == 1 && heq_at(cs, i, "//")) || (lang == 2 && c == '#');
        if is_line_comment {
            let mut j = i;
            while j < n && cs[j] != '\n' {
                st[j] = Style::Comment;
                j += 1;
            }
            i = j;
            continue;
        }
        if (lang == 1 && heq_at(cs, i, "/*")) || (lang == 5 && heq_at(cs, i, "<!--")) {
            let close: &str = if lang == 5 { "-->" } else { "*/" };
            let clen = close.chars().count();
            let mut j = i + clen;
            while j < n {
                if heq_at(cs, j, close) {
                    j += clen;
                    break;
                }
                j += 1;
            }
            let mut k = i;
            while k < j {
                st[k] = Style::Comment;
                k += 1;
            }
            i = j;
            continue;
        }
        if lang == 2 && (heq_at(cs, i, "\"\"\"") || heq_at(cs, i, "'''")) {
            let open: &str = if cs[i] == '"' { "\"\"\"" } else { "'''" };
            let mut j = i + 3;
            while j < n && !heq_at(cs, j, open) {
                j += 1;
            }
            if j < n {
                j += 3;
            }
            let mut k = i;
            while k < j {
                st[k] = Style::Str;
                k += 1;
            }
            i = j;
            continue;
        }
        let mut strq = c == '"' || (lang != 3 && c == '\'') || (lang == 1 && c == '`');
        if strq && lang == 1 && c == '\'' && !lang1_tick_opens_string(cs, i) {
            strq = false; // lifetime tick: fall through unstyled
        }
        if strq {
            let mut j = i + 1;
            while j < n && cs[j] != c {
                if cs[j] == '\\' {
                    j += 1;
                }
                if j < n && cs[j] == '\n' {
                    break;
                }
                j += 1;
            }
            if j < n && cs[j] == c {
                j += 1;
            }
            let mut k = i;
            while k < j {
                st[k] = Style::Str;
                k += 1;
            }
            i = j;
            continue;
        }
        if lang == 5 && c == '<' && i + 1 < n && hletter(cs[i + 1]) {
            // H-6: the WHOLE tag span paints as tag style (5), so
            // attribute strings inside it (`<a href="x">`) never become
            // string style (2) - the scan consumes to '>' before the
            // string branch can see the quote. Cosmetics only, by design.
            let mut j = i + 1;
            while j < n && cs[j] != '>' {
                j += 1;
            }
            if j < n {
                j += 1;
            }
            let mut k = i;
            while k < j {
                st[k] = Style::Marker;
                k += 1;
            }
            i = j;
            continue;
        }
        if hdigit(c) {
            // H-5: letters join a number token ONLY inside a 0x.. hex
            // literal (the 'x' must sit right after a leading '0').
            // The old scan consumed any a-f/A-F anywhere, painting
            // digit+word tokens like `2face` fully as a number.
            // Kept/covered: 0x1F, 0xDEADbeef, 0b101, 1.2, 1..2, and
            // exponents 1e6 / 12.3e4 / 1e-3 (an e/E joins only when a
            // DIGIT follows, directly or after one sign, so `2e` stays
            // '2' + plain). Rust float/int SUFFIXES (1.0f32, 2u8) are
            // NOT re-admitted: they are indistinguishable from the
            // 2face over-decoration pattern, so they stay under-painted
            // (cosmetic; the old scan "painted" them by the same bug).
            let mut j = i;
            let mut has_x = false;
            while j < n {
                let d = cs[j];
                let hexc = ('a'..='f').contains(&d) || ('A'..='F').contains(&d);
                let expdigit = j + 1 < n && hdigit(cs[j + 1]);
                let expsign = j + 2 < n
                    && (cs[j + 1] == '+' || cs[j + 1] == '-')
                    && hdigit(cs[j + 2]);
                if (d == 'x' || d == 'X' || d == 'b' || d == 'B') && j == i + 1 && cs[i] == '0' {
                    has_x = d == 'x' || d == 'X';
                    j += 1;
                } else if hdigit(d) || d == '.'
                    || (hexc && has_x)
                    || ((d == 'e' || d == 'E') && !has_x && (expdigit || expsign))
                {
                    j += 1;
                } else {
                    break;
                }
            }
            let mut k = i;
            while k < j {
                st[k] = Style::Number;
                k += 1;
            }
            i = j;
            continue;
        }
        if hletter(c) {
            let mut j = i;
            while j < n && hwordc(cs[j]) {
                j += 1;
            }
            if kw(lang, &cs[i..j]) {
                let mut m = i;
                while m < j {
                    st[m] = Style::Keyword;
                    m += 1;
                }
            }
            i = j;
            continue;
        }
        i += 1;
    }
    st
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detect_extensions() {
        // H-3: extension matching is case-insensitive.
        assert!(detect("FILE.RS") == 1);
        assert!(detect("a.b.rs") == 1);
        assert!(detect("x.md") == 4);
        assert!(detect("*demo*") == 0);
        assert!(detect("/tmp/x.PY") == 2);
        assert!(detect("a.JSON") == 3);
        assert!(detect("z.HTML") == 5);
        assert!(detect("plain") == 0);
    }
    #[test]
    fn rust_lifetimes_not_strings() {
        // H-2 pinned fixture: no tick in a lifetime pair becomes string style.
        let src = "fn f<'de>(x: &'de str) {}";
        let cs = hchars_of(src);
        let st = styles(&cs, 1);
        assert!(st.len() == cs.len());
        for k in 0..cs.len() {
            if cs[k] == '\'' {
                assert!(st[k] != Style::Str); // 'de>(x: & fails (c), 'de str) {} fails (a)
            }
        }
        assert!(st[0] == Style::Keyword); // fn is a keyword
    }
    #[test]
    fn rust_strings_and_char_literal() {
        let cs = hchars_of("let s = \"hi\"; let c = 'a';");
        let st = styles(&cs, 1);
        let dq = cs.iter().position(|c| *c == '"').unwrap();
        assert!(st[dq] == Style::Str); // "hi" string
        let sq = cs.iter().position(|c| *c == '\'').unwrap();
        assert!(st[sq] == Style::Str); // 'a' char literal: styled as string (accepted)
        assert!(st[0] == Style::Keyword); // let keyword
    }
    #[test]
    fn json_single_quote_untouched() {
        let cs = hchars_of("{'a': 1}");
        let st = styles(&cs, 3);
        for k in 0..cs.len() {
            if cs[k] == '\'' {
                assert!(st[k] == Style::Plain);
            }
        }
    }
    #[test]
    fn md_heading_and_marker() {
        let cs = hchars_of("# Title\n- item");
        let st = styles(&cs, 4);
        assert!(st[0] == Style::Marker);
        assert!(st[8] == Style::Marker); // the '-' marker of "- item"
    }
    #[test]
    fn md_unclosed_code_span_stops_at_line_end() {
        // H-4: an UNCLOSED ` span paints to end-of-line only: the next
        // line must not be swallowed.
        let cs: Vec<char> = "a `bc\nd\n".chars().collect();
        let mut st = vec![Style::Plain; cs.len()];
        md_styles(&cs, &mut st);
        assert!(st[2] == Style::Str && st[3] == Style::Str && st[4] == Style::Str); // `bc to EOL
        assert!(st[5] == Style::Plain); // the newline stays plain
        assert!(st[6] == Style::Plain); // NEXT line untouched
    }
    #[test]
    fn number_scan_letters_only_after_0x() {
        // H-5: letters join a number ONLY inside 0x.. hex; exponents
        // join when a digit follows e/E (directly or after one sign).
        let s1 = styles(&"2face".chars().collect::<Vec<char>>(), 1);
        assert!(s1[0] == Style::Number && s1[1] == Style::Plain); // '2' is the number, face plain
        let s2 = styles(&"0x1F".chars().collect::<Vec<char>>(), 1);
        assert!(s2.iter().all(|&x| x == Style::Number));
        let s3 = styles(&"1.2".chars().collect::<Vec<char>>(), 1);
        assert!(s3.iter().all(|&x| x == Style::Number));
        let s4 = styles(&"1e6".chars().collect::<Vec<char>>(), 1);
        assert!(s4.iter().all(|&x| x == Style::Number)); // exponent restored
        let s5 = styles(&"12.3e4".chars().collect::<Vec<char>>(), 1);
        assert!(s5.iter().all(|&x| x == Style::Number));
        let s6 = styles(&"1e-3 x".chars().collect::<Vec<char>>(), 1);
        assert!(s6[0] == Style::Number && s6[1] == Style::Number); // "1e" painted (e joins via sign+digit)
        assert!(s6[2] == Style::Plain); // the sign itself never paints (same as the old scan)
        assert!(s6[3] == Style::Number); // the exponent's digits paint as their own number
        assert!(s6[4] == Style::Plain && s6[5] == Style::Plain); // space and x are plain
        let s7 = styles(&"0b101".chars().collect::<Vec<char>>(), 1);
        assert!(s7.iter().all(|&x| x == Style::Number)); // binary literal restored
        let s8 = styles(&"2e".chars().collect::<Vec<char>>(), 1);
        assert!(s8[0] == Style::Number && s8[1] == Style::Plain); // dangling e: '2' only
    }
}
