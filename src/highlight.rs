// Syntax highlighting: per-character style codes for a buffer's text,
// selected by the buffer/file name (language detected from the extension).
// Styles: 0 plain, 1 keyword, 2 string, 3 comment, 4 number, 5 heading/marker.
// Languages (codes): 0 plain, 1 c-family (rs c cpp h js ts go css),
// 2 hash-family (py sh rb yaml toml), 3 json, 4 markdown, 5 html/xml.

pub fn hchars_of(h: String) -> Vec<char> {
    let mut cs: Vec<char> = vec![];
    for c in h.chars() {
        cs.push(c);
    }
    cs
}

pub fn heq_at(cs: Vec<char>, i: usize, w: String) -> bool {
    let ws = hchars_of(w);
    if i + ws.len() > cs.len() {
        return false;
    }
    let mut k = 0;
    while k < ws.len() {
        if cs[i + k] != ws[k] {
            return false;
        }
        k += 1;
    }
    true
}

pub fn hwordc(c: char) -> bool {
    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_' || (c >= '0' && c <= '9')
}

pub fn hletter(c: char) -> bool {
    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_'
}

pub fn hdigit(c: char) -> bool {
    c >= '0' && c <= '9'
}

pub fn detect(name: String) -> usize {
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
    let at = dot + 1;
    let exts1 = vec!["rs", "c", "h", "cpp", "cc", "hpp", "js", "ts", "go", "css"];
    let exts2 = vec!["py", "sh", "rb", "yaml", "toml"];
    let exts4 = vec!["md", "markdown"];
    let exts5 = vec!["html", "xml"];
    for e in exts1.iter() {
        if heq_at(cs.clone(), at, e.to_string()) && at + e.len() == n {
            return 1;
        }
    }
    for e in exts2.iter() {
        if heq_at(cs.clone(), at, e.to_string()) && at + e.len() == n {
            return 2;
        }
    }
    if heq_at(cs.clone(), at, "json".to_string()) && at + 4 == n {
        return 3;
    }
    for e in exts4.iter() {
        if heq_at(cs.clone(), at, e.to_string()) && at + e.len() == n {
            return 4;
        }
    }
    for e in exts5.iter() {
        if heq_at(cs.clone(), at, e.to_string()) && at + e.len() == n {
            return 5;
        }
    }
    0
}

pub fn kw(lang: usize, w: String) -> bool {
    if lang == 1 {
        return w == "fn" || w == "let" || w == "const" || w == "if" || w == "else" ||
            w == "for" || w == "while" || w == "return" || w == "struct" || w == "enum" ||
            w == "impl" || w == "pub" || w == "use" || w == "match" || w == "case" ||
            w == "switch" || w == "break" || w == "continue" || w == "true" ||
            w == "false" || w == "new" || w == "class" || w == "void" || w == "int" ||
            w == "char" || w == "float" || w == "double" || w == "bool" || w == "auto" ||
            w == "package" || w == "import" || w == "func" || w == "var" || w == "type";
    }
    if lang == 2 {
        return w == "def" || w == "if" || w == "elif" || w == "else" || w == "for" ||
            w == "while" || w == "return" || w == "import" || w == "from" || w == "class" ||
            w == "None" || w == "True" || w == "False" || w == "lambda" || w == "in" ||
            w == "not" || w == "and" || w == "or" || w == "then" || w == "fi" ||
            w == "done" || w == "case" || w == "esac" || w == "echo" || w == "export" ||
            w == "local" || w == "function";
    }
    if lang == 3 {
        return w == "true" || w == "false" || w == "null";
    }
    false
}

pub fn md_styles(cs: Vec<char>, mut st: Vec<usize>) -> Vec<usize> {
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
            return st;
        }
        if cs[i] == '#' {
            while i < n && cs[i] != '\n' {
                st[i] = 5;
                i += 1;
            }
            continue;
        }
        if i < n && (cs[i] == '-' || cs[i] == '*' || cs[i] == '+') && i + 1 < n && cs[i + 1] == ' ' {
            st[i] = 5;
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
                st[k] = 2;
                k += 1;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    st
}

pub fn styles(text: String, lang: usize) -> Vec<usize> {
    let cs = hchars_of(text);
    let n = cs.len();
    let mut st: Vec<usize> = vec![];
    let mut i = 0;
    while i < n {
        st.push(0);
        i += 1;
    }
    if lang == 0 {
        return st;
    }
    if lang == 4 {
        return md_styles(cs, st);
    }
    i = 0;
    while i < n {
        let c = cs[i];
        let is_line_comment = (lang == 1 && heq_at(cs.clone(), i, "//".to_string())) || (lang == 2 && c == '#');
        if is_line_comment {
            let mut j = i;
            while j < n && cs[j] != '\n' {
                st[j] = 3;
                j += 1;
            }
            i = j;
            continue;
        }
        if (lang == 1 && heq_at(cs.clone(), i, "/*".to_string())) || (lang == 5 && heq_at(cs.clone(), i, "<!--".to_string())) {
            let close = if lang == 5 { "-->".to_string() } else { "*/".to_string() };
            let cl = hchars_of(close.clone());
            let mut j = i + cl.len();
            while j < n {
                if heq_at(cs.clone(), j, close.clone()) {
                    j += cl.len();
                    break;
                }
                j += 1;
            }
            let mut k = i;
            while k < j {
                st[k] = 3;
                k += 1;
            }
            i = j;
            continue;
        }
        if lang == 2 && (heq_at(cs.clone(), i, "\"\"\"".to_string()) || heq_at(cs.clone(), i, "'''".to_string())) {
            let open = if cs[i] == '"' { "\"\"\"".to_string() } else { "'''".to_string() };
            let mut j = i + 3;
            while j < n && !heq_at(cs.clone(), j, open.clone()) {
                j += 1;
            }
            if j < n {
                j += 3;
            }
            let mut k = i;
            while k < j {
                st[k] = 2;
                k += 1;
            }
            i = j;
            continue;
        }
        let strq = c == '"' || (lang != 3 && c == '\'') || (lang == 1 && c == '`');
        if strq {
            let mut j = i + 1;
            while j < n && cs[j] != c {
                if cs[j] == '\\' {
                    j += 1;
                }
                if j + 1 <= n && cs[j] == '\n' {
                    break;
                }
                j += 1;
            }
            if j < n && cs[j] == c {
                j += 1;
            }
            let mut k = i;
            while k < j {
                st[k] = 2;
                k += 1;
            }
            i = j;
            continue;
        }
        if lang == 5 && c == '<' && i + 1 < n && hletter(cs[i + 1]) {
            let mut j = i + 1;
            while j < n && cs[j] != '>' {
                j += 1;
            }
            if j < n {
                j += 1;
            }
            let mut k = i;
            while k < j {
                st[k] = 5;
                k += 1;
            }
            i = j;
            continue;
        }
        if hdigit(c) {
            let mut j = i;
            while j < n {
                let d = cs[j];
                let hexc = (d >= 'a' && d <= 'f') || (d >= 'A' && d <= 'F');
                if hdigit(d) || d == '.' || d == 'x' || d == 'X' || hexc {
                    j += 1;
                } else {
                    break;
                }
            }
            let mut k = i;
            while k < j {
                st[k] = 4;
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
            let mut w: Vec<char> = vec![];
            let mut k = i;
            while k < j {
                w.push(cs[k]);
                k += 1;
            }
            if kw(lang, w.iter().collect::<String>()) {
                let mut m = i;
                while m < j {
                    st[m] = 1;
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
