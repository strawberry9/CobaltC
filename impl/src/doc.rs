// `--doc NAME` (CHG-0446): an item of `std`, a type with its functions, or a
// module, from the documentation in `std`'s own source (D-0229) and
// `impl/std/builtins.md`, both compiled into the tools.
//
// `std_records` reads them as `impl/tools/cobdoc.py` does (the test
// `doc_matches_cobdoc` holds the two together); each item becomes a record
// whose text is already laid out, and `lookup` answers a name from the
// records alone. `cbc` has no `std` source of its own: `coby
// --prelude-summary` writes the records into impl/selfhost/front/prelude.cb
// (`PRELUDE_DOCS`, `encode`), and impl/selfhost/cbc/doc.cb is `lookup`'s
// twin over them, so the three tools print the same text.

/// One exported item, as `--doc` shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// `std::collections`.
    pub module: String,
    /// `Vec::push`, `printf`.
    pub item: String,
    /// `fn`, `extern fn`, `struct`, `enum`, `bitstruct`, `const`, `built in fn`.
    pub kind: String,
    /// The declaration up to its body.
    pub signature: String,
    /// The documentation's first sentence; empty for none.
    pub summary: String,
    /// `deprecated` or `removed` (D-0303), or empty.
    pub retired: String,
    /// The whole entry as `--doc` prints it for this item.
    pub text: String,
}

/// An item as `cobdoc.py` reads it, before it is laid out.
#[derive(Clone, Debug)]
pub struct Item {
    pub module: String,
    pub item: String,
    pub kind: String,
    pub signature: String,
    pub requires: Vec<String>,
    pub doc: Option<String>,
    pub file: String,
    pub line: usize,
    pub examples: Vec<(Option<String>, String)>,
    pub retired: Option<(String, String)>,
}

const BUILTINS: &str = include_str!("../std/builtins.md");

/// The documentation's first sentence: up to the first `. ` outside
/// backticks (or the end of the first paragraph).
pub fn summary(doc: &str) -> String {
    let para = doc.split("\n\n").next().unwrap_or("").replace('\n', " ");
    let para = para.trim();
    let b = para.as_bytes();
    let mut code = false;
    for (i, &c) in b.iter().enumerate() {
        if c == b'`' {
            code = !code;
        } else if c == b'.' && !code && (i + 1 == b.len() || (b[i + 1] as char).is_whitespace()) {
            return para[..i + 1].to_string();
        }
    }
    para.to_string()
}

fn squeeze(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// An item's declaration from line `i`, up to its body or its `;`.
fn signature(lines: &[&str], i: usize) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in lines.iter().skip(i).take(40) {
        let mut cut = line.len();
        if let Some(k) = line.find('{') {
            cut = cut.min(k);
        }
        if let Some(k) = line.find(';') {
            cut = cut.min(k + 1);
        }
        out.push(line[..cut].trim().to_string());
        let r = line.trim_end();
        if cut < line.len() || r.ends_with(';') || r.ends_with('{') {
            break;
        }
    }
    let mut sig = out.iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join(" ");
    // D-0317: a `requires` block is not part of the signature.
    let t = sig.trim_end();
    if let Some(head) = t.strip_suffix("requires") {
        if head.is_empty() || head.ends_with(|c: char| !(c.is_alphanumeric() || c == '_')) {
            sig = head.trim_end().to_string();
        }
    }
    // `(\s+` → `(`, `\s+)` → `)`, `,\s*)` → `)`.
    let mut s = String::new();
    let chars: Vec<char> = sig.chars().collect();
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        if c == '(' {
            s.push(c);
            k += 1;
            while k < chars.len() && chars[k].is_whitespace() {
                k += 1;
            }
            continue;
        }
        if c.is_whitespace() {
            let mut j = k;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && chars[j] == ')' {
                k = j;
                continue;
            }
        }
        if c == ',' {
            let mut j = k + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && chars[j] == ')' {
                k = j;
                continue;
            }
        }
        s.push(c);
        k += 1;
    }
    let s = squeeze(&s);
    s.trim_end_matches(';').trim().to_string()
}

// D-0317: the requirements of the `fn` declared from line `i`, each as
// written (`cond, diag, "message", args`), in order; none without a
// `requires` block.
fn requires_of(lines: &[&str], i: usize) -> Vec<String> {
    let text = lines.iter().skip(i).take(200).cloned().collect::<Vec<_>>().join("\n");
    let b = text.as_bytes();
    // `\brequires\s*\{`
    let mut at = None;
    let mut from = 0;
    while let Some(k) = text[from..].find("requires") {
        let s = from + k;
        let e = s + "requires".len();
        let word_start = s == 0 || !(b[s - 1].is_ascii_alphanumeric() || b[s - 1] == b'_');
        let mut j = e;
        while j < b.len() && (b[j] as char).is_whitespace() {
            j += 1;
        }
        if word_start && j < b.len() && b[j] == b'{' {
            at = Some(j + 1);
            break;
        }
        from = e;
    }
    let Some(open) = at else { return Vec::new() };
    if let Some(h) = text.find(|c| c == '{' || c == ';') {
        if h < open - 1 {
            return Vec::new();
        }
    }
    let mut parts: Vec<String> = Vec::new();
    let mut part = String::new();
    let (mut depth, mut quote) = (0i32, false);
    let chars: Vec<char> = text[open..].chars().collect();
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        if quote {
            part.push(c);
            if c == '\\' && k + 1 < chars.len() {
                part.push(chars[k + 1]);
                k += 1;
            } else if c == '"' {
                quote = false;
            }
        } else if c == '"' {
            quote = true;
            part.push(c);
        } else if "([{".contains(c) {
            depth += 1;
            part.push(c);
        } else if ")]}".contains(c) {
            if depth == 0 {
                break;
            }
            depth -= 1;
            part.push(c);
        } else if c == ';' && depth == 0 {
            parts.push(std::mem::take(&mut part));
        } else {
            part.push(c);
        }
        k += 1;
    }
    parts.iter().map(|p| squeeze(p)).filter(|p| !p.is_empty()).collect()
}

fn attr(line: &str) -> Option<(String, String)> {
    let s = line.trim();
    let rest = s.strip_prefix("[[")?.strip_suffix("]]")?;
    let (kind, rest) = if let Some(r) = rest.strip_prefix("deprecated") {
        ("deprecated", r)
    } else if let Some(r) = rest.strip_prefix("removed") {
        ("removed", r)
    } else {
        return None;
    };
    let msg = rest.strip_prefix("(\"")?.strip_suffix("\")")?;
    Some((kind.to_string(), msg.to_string()))
}

fn ident_end(s: &str) -> usize {
    let b = s.as_bytes();
    if b.is_empty() || !(b[0].is_ascii_alphabetic() || b[0] == b'_') {
        return 0;
    }
    let mut k = 1;
    while k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'_') {
        k += 1;
    }
    k
}

fn eat_word<'a>(s: &'a str, w: &str) -> Option<&'a str> {
    let r = s.strip_prefix(w)?;
    let t = r.trim_start();
    if t.len() == r.len() {
        return None;
    }
    Some(t)
}

// `export [unsafe] [extern] fn NAME`: the kind and the name.
fn fn_head(line: &str) -> Option<(String, String)> {
    let mut r = eat_word(line, "export")?;
    if let Some(t) = eat_word(r, "unsafe") {
        r = t;
    }
    let mut kind = "fn";
    if let Some(t) = eat_word(r, "extern") {
        r = t;
        kind = "extern fn";
    }
    let r = eat_word(r, "fn")?;
    let e = ident_end(r);
    if e == 0 {
        return None;
    }
    let mut name = r[..e].to_string();
    if let Some(t) = r[e..].strip_prefix("::") {
        let e2 = ident_end(t);
        if e2 > 0 {
            name = format!("{}::{}", name, &t[..e2]);
        }
    }
    Some((kind.to_string(), name))
}

// `export [resource|quiet]* (struct|enum|bitstruct) NAME`.
fn type_head(line: &str) -> Option<(String, String)> {
    let mut r = eat_word(line, "export")?;
    loop {
        if let Some(t) = eat_word(r, "resource").or_else(|| eat_word(r, "quiet")) {
            r = t;
        } else {
            break;
        }
    }
    for k in ["struct", "enum", "bitstruct"] {
        if let Some(t) = eat_word(r, k) {
            let e = ident_end(t);
            if e > 0 {
                return Some((k.to_string(), t[..e].to_string()));
            }
        }
    }
    None
}

// `export const … NAME =`.
fn const_head(line: &str) -> Option<String> {
    let r = eat_word(line, "export")?;
    let r = eat_word(r, "const")?;
    let b = r.as_bytes();
    for (k, &c) in b.iter().enumerate() {
        if c != b'=' {
            continue;
        }
        let mut e = k;
        while e > 0 && (b[e - 1] as char).is_whitespace() {
            e -= 1;
        }
        let mut s = e;
        while s > 0 && (b[s - 1].is_ascii_alphanumeric() || b[s - 1] == b'_') {
            s -= 1;
        }
        while s < e && b[s].is_ascii_digit() {
            s += 1;
        }
        if s < e {
            return Some(r[s..e].to_string());
        }
    }
    None
}

// `[export] module NAME "PATH";`.
fn module_line(line: &str) -> Option<(String, String)> {
    let r = eat_word(line, "export").unwrap_or(line);
    let r = eat_word(r, "module")?;
    let e = ident_end(r);
    if e == 0 {
        return None;
    }
    let rest = r[e..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let q = rest.find('"')?;
    let after = rest[q + 1..].trim();
    if after != ";" {
        return None;
    }
    Some((r[..e].to_string(), rest[..q].to_string()))
}

// `unittest ["NAME"] [{]`: the indent and the name.
fn test_line(line: &str) -> Option<(String, Option<String>)> {
    let indent_len = line.len() - line.trim_start().len();
    let r = line[indent_len..].strip_prefix("unittest")?;
    let r = r.trim_start();
    let (name, rest) = if let Some(q) = r.strip_prefix('"') {
        let mut end = None;
        let b = q.as_bytes();
        let mut k = 0;
        while k < b.len() {
            if b[k] == b'\\' {
                k += 2;
                continue;
            }
            if b[k] == b'"' {
                end = Some(k);
                break;
            }
            k += 1;
        }
        let e = end?;
        (Some(q[..e].to_string()), &q[e + 1..])
    } else {
        (None, r)
    };
    let rest = rest.trim();
    if !(rest.is_empty() || rest == "{") {
        return None;
    }
    Some((line[..indent_len].to_string(), name))
}

fn items_of(files: &dyn Fn(&str) -> Option<String>, path: &str, module: &str, out: &mut Vec<Item>, seen: &mut Vec<String>) {
    if seen.iter().any(|p| p == path) {
        return;
    }
    seen.push(path.to_string());
    let Some(text) = files(path) else { return };
    let lines: Vec<&str> = text.split('\n').collect();
    let dir = match path.rfind('/') {
        Some(k) => &path[..k + 1],
        None => "",
    };
    let mut doc: Vec<String> = Vec::new();
    let mut retired: Option<(String, String)> = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(a) = attr(line) {
            retired = Some(a);
            continue;
        }
        if let Some(t) = line.strip_prefix("///") {
            doc.push(t.strip_prefix(' ').unwrap_or(t).to_string());
            continue;
        }
        if let Some((name, sub)) = module_line(line) {
            let p = format!("{}{}", dir, sub);
            items_of(files, &p, &format!("{}::{}", module, name), out, seen);
            doc.clear();
            continue;
        }
        if let Some((indent, name)) = test_line(line) {
            if !doc.is_empty() && out.last().map_or(false, |x| x.file == path) {
                let mut body: Vec<&str> = Vec::new();
                for later in &lines[i + 1..] {
                    let lead = later.len() - later.trim_start().len();
                    if later.trim() == "}" && later[..lead] == *indent {
                        break;
                    }
                    body.push(later);
                }
                if body.first().map_or(false, |b| b.trim() == "{") {
                    body.remove(0);
                }
                let width = body.iter().filter(|b| !b.trim().is_empty()).map(|b| b.len() - b.trim_start().len()).min().unwrap_or(0);
                let code = body.iter().map(|b| if b.len() >= width { &b[width..] } else { "" }).collect::<Vec<_>>().join("\n");
                out.last_mut().unwrap().examples.push((name, code.trim_matches('\n').to_string()));
            }
            doc.clear();
            continue;
        }
        let head = fn_head(line).or_else(|| type_head(line)).or_else(|| const_head(line).map(|n| ("const".to_string(), n)));
        if let Some((kind, name)) = head {
            let text = if doc.is_empty() { None } else { Some(doc.join("\n").trim().to_string()) };
            out.push(Item {
                module: module.to_string(),
                item: name,
                requires: if kind.ends_with("fn") { requires_of(&lines, i) } else { Vec::new() },
                kind,
                signature: signature(&lines, i),
                doc: text,
                file: path.to_string(),
                line: i + 1,
                examples: Vec::new(),
                retired: retired.take(),
            });
        }
        doc.clear();
        retired = None;
    }
}

/// The entries of `impl/std/builtins.md`.
fn builtins() -> Vec<Item> {
    let lines: Vec<&str> = BUILTINS.split('\n').collect();
    let head = |l: &str| -> Option<(String, String)> {
        let r = l.strip_prefix("## `")?;
        let (item, rest) = r.split_once("` in `")?;
        let module = rest.trim_end().strip_suffix('`')?;
        Some((item.to_string(), module.to_string()))
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((item, module)) = head(lines[i]) else {
            i += 1;
            continue;
        };
        let start = i + 1;
        let mut sig = String::new();
        let mut body: Vec<&str> = Vec::new();
        i += 1;
        while i < lines.len() && head(lines[i]).is_none() {
            match lines[i].strip_prefix("Signature:") {
                Some(s) if sig.is_empty() && s.trim().starts_with('`') && s.trim().ends_with('`') => sig = s.trim().replace('`', "").trim().to_string(),
                _ => body.push(lines[i]),
            }
            i += 1;
        }
        let text = body.join("\n").trim().to_string();
        out.push(Item {
            module,
            item,
            kind: "built in fn".to_string(),
            signature: sig,
            requires: Vec::new(),
            doc: if text.is_empty() { None } else { Some(text) },
            file: "builtins.md".to_string(),
            line: start,
            examples: Vec::new(),
            retired: None,
        });
    }
    out
}

/// Every exported item of `std`, in the order `cobdoc.py` lists them.
pub fn std_items() -> Vec<Item> {
    let files = |p: &str| -> Option<String> {
        if p == "std.cb" {
            return Some(crate::prelude::std_root().to_string());
        }
        crate::prelude::std_file(p).map(|s| s.to_string())
    };
    let mut out = Vec::new();
    items_of(&files, "std.cb", "std", &mut out, &mut Vec::new());
    out.extend(builtins());
    out
}

fn indent(text: &str, by: &str) -> String {
    text.split('\n').map(|l| if l.is_empty() { String::new() } else { format!("{}{}", by, l) }).collect::<Vec<_>>().join("\n")
}

/// An item's record: its entry laid out.
pub fn record(x: &Item) -> Record {
    let mut text = format!("{}\n    in `{}`", x.signature, x.module);
    if let Some((kind, msg)) = &x.retired {
        text.push_str(&format!("\n    {}: {}", kind, msg));
    }
    if !x.requires.is_empty() {
        text.push_str("\n    requires");
        for r in &x.requires {
            text.push_str(&format!("\n        {}", r));
        }
    }
    if let Some(d) = &x.doc {
        text.push_str("\n\n");
        text.push_str(&indent(d, "    "));
    }
    for (name, code) in &x.examples {
        match name {
            Some(n) => text.push_str(&format!("\n\n    Example, \"{}\":\n", n)),
            None => text.push_str("\n\n    Example:\n"),
        }
        text.push_str(&indent(code, "        "));
    }
    text.push('\n');
    Record {
        module: x.module.clone(),
        item: x.item.clone(),
        kind: x.kind.clone(),
        signature: x.signature.clone(),
        summary: x.doc.as_deref().map(summary).unwrap_or_default(),
        retired: x.retired.as_ref().map(|r| r.0.clone()).unwrap_or_default(),
        text,
    }
}

/// `std`'s records, made once.
pub fn std_records() -> &'static [Record] {
    static R: std::sync::OnceLock<Vec<Record>> = std::sync::OnceLock::new();
    R.get_or_init(|| std_items().iter().map(record).collect())
}

// The records as one text (`PRELUDE_DOCS`): fields separated by U+001F,
// records by U+001E.
pub fn encode(records: &[Record]) -> String {
    let mut s = String::new();
    for r in records {
        for f in [&r.module, &r.item, &r.kind, &r.signature, &r.summary, &r.retired, &r.text] {
            s.push_str(f);
            s.push('\u{1f}');
        }
        s.push('\u{1e}');
    }
    s
}

// An item's line in a listing: its signature, and its summary under it.
fn line_of(r: &Record) -> String {
    let mut s = format!("  {}\n", r.signature);
    if r.retired == "deprecated" {
        s.push_str("      (deprecated)\n");
    }
    if !r.summary.is_empty() {
        s.push_str(&format!("      {}\n", r.summary));
    }
    s
}

fn module_of(name: &str) -> String {
    if name == "std" || name.starts_with("std::") {
        name.to_string()
    } else {
        format!("std::{}", name)
    }
}

/// What `--doc NAME` prints (`Ok`), or what it says when nothing has that
/// name (`Err`, exit status 1). With no name: the modules.
pub fn lookup(records: &[Record], name: Option<&str>) -> Result<String, String> {
    let Some(name) = name else {
        let mut mods: Vec<(String, usize)> = Vec::new();
        for r in records {
            match mods.iter_mut().find(|(m, _)| *m == r.module) {
                Some(e) => e.1 += 1,
                None => mods.push((r.module.clone(), 1)),
            }
        }
        mods.sort();
        let mut s = String::from("The modules of `std`, and how many items each exports (`--doc MODULE` lists them):\n");
        for (m, n) in &mods {
            s.push_str(&format!("  {} ({})\n", m, n));
        }
        s.push_str("`--doc NAME` shows an item (`Vec::push`, `printf`), a type and its functions (`Vec`), or a module (`std::text`).\n");
        return Ok(s);
    };
    let bare = name.strip_prefix("std::").unwrap_or(name);
    // A module.
    let m = module_of(name);
    let in_module: Vec<&Record> = records.iter().filter(|r| r.module == m && r.retired != "removed").collect();
    if !in_module.is_empty() {
        let mut s = format!("module `{}`: {} items\n", m, in_module.len());
        for r in in_module {
            s.push_str(&line_of(r));
        }
        return Ok(s);
    }
    // An item: `Vec::push`, `std::collections::Vec::push`, `printf`.
    let hits: Vec<&Record> = records.iter().filter(|r| r.item == bare || format!("{}::{}", r.module, r.item) == module_of(bare)).collect();
    if !hits.is_empty() {
        let mut s = String::new();
        for (k, r) in hits.iter().enumerate() {
            if k > 0 {
                s.push('\n');
            }
            s.push_str(&r.text);
            if matches!(r.kind.as_str(), "struct" | "enum" | "bitstruct") {
                let prefix = format!("{}::", r.item);
                let fns: Vec<&Record> = records.iter().filter(|f| f.module == r.module && f.item.starts_with(&prefix) && f.retired != "removed").collect();
                if !fns.is_empty() {
                    s.push_str(&format!("\nThe functions of `{}`:\n", r.item));
                    for f in fns {
                        s.push_str(&line_of(f));
                    }
                }
            }
        }
        return Ok(s);
    }
    let mut names: Vec<&str> = records.iter().filter(|r| r.retired != "removed").map(|r| r.item.as_str()).collect();
    names.sort();
    names.dedup();
    Err(match crate::hints::nearest(bare, names.iter().copied()) {
        Some(n) => format!("nothing named `{}` in `std`; did you mean `{}`?", name, n),
        None => format!("nothing named `{}` in `std`; `--doc` alone lists its modules", name),
    })
}

/// `coby --doc [NAME]`, `cobc --doc [NAME]`.
pub fn run(args: &[String], tool: &str) -> u8 {
    if args.len() > 1 {
        eprintln!("usage: {} --doc [NAME]", tool);
        return 2;
    }
    match lookup(std_records(), args.first().map(|s| s.as_str())) {
        Ok(s) => {
            print!("{}", s);
            0
        }
        Err(e) => {
            eprintln!("{}: {}", tool, e);
            1
        }
    }
}
