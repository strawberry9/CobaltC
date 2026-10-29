// `cobc --report-tracking FILE.cb`: where the compiled program still pays
// for run-time tracking. The generated C is read back: every call of the
// runtime's tracking interface -- paths, objects, checked reads and
// writes -- is attributed to the source line named by the nearest
// `cb_at`/`cb_stmt_push_at`/`cb_frame_push_at` before it, and the
// program's own lines (not `std`'s) that carry any are listed, those
// inside a loop of the generated C first. Bounds and overflow checks are
// not tracking: they are cheap, and they are the language's semantics.
// The counts are static (calls written in the C, not calls made).
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;
use std::path::Path;

const TRACKING: &[&str] = &[
    "cb_borrow_range_unminted", "cb_borrow_unminted", "cb_borrow_unstamped", "cb_borrow_range", "cb_borrow_check", "cb_borrow",
    "cb_elem_borrow_here", "cb_elem_borrow_datum", "cb_elem_borrow", "cb_elem_access", "cb_read", "cb_write",
    "cb_recv_datum_tok", "cb_recv_datum", "cb_recv", "cb_send_datum", "cb_send_ref", "cb_send", "cb_copy_datum", "cb_new", "cb_bind",
    "cb_move_to", "cb_store_ref", "cb_load_ref", "cb_frame_hold", "cb_valid", "cb_absorb",
];

// The tracking calls on one line of generated C.
fn calls(row: &str) -> Vec<&'static str> {
    let b = row.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(k) = row[i..].find("cb_") {
        let at = i + k;
        let word_start = at == 0 || !(b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_');
        let name_end = row[at..].find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map_or(row.len(), |e| at + e);
        if word_start && row[name_end..].trim_start().starts_with('(') {
            if let Some(t) = TRACKING.iter().find(|t| **t == &row[at..name_end]) {
                out.push(*t);
            }
        }
        i = name_end.max(at + 3);
    }
    out
}

// `cb_at(F, L)` and its relatives: the last source position on the line.
fn position(row: &str) -> Option<(usize, usize)> {
    let mut last = None;
    for marker in ["cb_at(", "cb_stmt_push_at(", "cb_frame_push_at("] {
        let mut from = 0;
        while let Some(k) = row[from..].find(marker) {
            let args = &row[from + k + marker.len()..];
            let end = args.find(')').unwrap_or(args.len());
            let mut parts = args[..end].split(',').map(|p| p.trim().trim_end_matches('u'));
            if let (Some(f), Some(l)) = (parts.next(), parts.next()) {
                if let (Ok(f), Ok(l)) = (f.parse::<usize>(), l.parse::<usize>()) {
                    if last.map_or(true, |(o, _)| o < from + k) {
                        last = Some((from + k, (f, l)));
                    }
                }
            }
            from += k + marker.len();
        }
    }
    last.map(|(_, p)| p)
}

// The names in `static const char *const cb_files[] = {"a.cb", …, 0};`.
fn file_names(c: &str) -> Vec<String> {
    let Some(row) = c.lines().find(|l| l.starts_with("static const char *const cb_files[]")) else { return Vec::new() };
    row.split('"').skip(1).step_by(2).map(str::to_string).collect()
}

pub fn report(c: &str, entry: &Path) -> String {
    let names = file_names(c);
    let mut per_line: BTreeMap<(usize, usize), BTreeMap<&'static str, usize>> = BTreeMap::new();
    let mut in_loop: HashSet<(usize, usize)> = HashSet::new();
    let mut per_fn: HashMap<String, usize> = HashMap::new();
    let mut func = String::new();
    let mut cur: Option<(usize, usize)> = None;
    let mut stack: Vec<bool> = Vec::new();
    for row in c.lines() {
        // A function's definition begins at column 0 with its signature.
        if !row.starts_with(' ') && row.contains("f_") && row.ends_with(')') && !row.ends_with(';') {
            func = row.split('(').next().unwrap_or("").rsplit(' ').next().unwrap_or("").trim_start_matches('*').to_string();
            cur = None;
            stack.clear();
            continue;
        }
        if let Some(p) = position(row) {
            cur = Some(p);
        }
        let looping = stack.iter().any(|&l| l);
        for k in calls(row) {
            // File 0 onward are the program's files; `std`'s calls carry no
            // position of the program's (`cb_at` is not written in `std`).
            if let Some(p) = cur.filter(|p| p.0 < names.len()) {
                *per_line.entry(p).or_default().entry(k).or_default() += 1;
                *per_fn.entry(func.clone()).or_default() += 1;
                if looping {
                    in_loop.insert(p);
                }
            }
        }
        let code = row.split("//").next().unwrap_or("");
        for ch in code.chars() {
            match ch {
                '{' => stack.push(code.contains("for (;;)") || code.contains("while (")),
                '}' => {
                    stack.pop();
                }
                _ => {}
            }
        }
    }
    let dir = entry.parent().unwrap_or(Path::new("."));
    let mut texts: HashMap<usize, Vec<String>> = HashMap::new();
    let mut out = String::new();
    if per_line.is_empty() {
        writeln!(out, "no run-time tracking left in the program's own code").unwrap();
        return out;
    }
    writeln!(out, "Run-time tracking left in the program's own code (calls written in the C, per source line;").unwrap();
    writeln!(out, "`loop` marks lines inside a loop, listed first):").unwrap();
    let mut lines: Vec<&(usize, usize)> = per_line.keys().collect();
    lines.sort_by_key(|p| (!in_loop.contains(p), **p));
    for p in lines {
        let name = &names[p.0];
        let text = texts
            .entry(p.0)
            .or_insert_with(|| {
                // As `cb_files` names it (the path cobc was given), else
                // beside the entry file.
                let base = Path::new(name).file_name().map(Path::new).unwrap_or(Path::new(name));
                std::fs::read_to_string(name).or_else(|_| std::fs::read_to_string(dir.join(base))).map(|s| s.lines().map(str::to_string).collect()).unwrap_or_default()
            })
            .get(p.1.wrapping_sub(1))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let mut kinds: Vec<(&&str, &usize)> = per_line[p].iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let kinds: Vec<String> = kinds.iter().map(|(k, n)| format!("{} x{}", &k[3..], n)).collect();
        let short: String = text.chars().take(56).collect();
        let shown = Path::new(name).file_name().and_then(|n| n.to_str()).unwrap_or(name);
        writeln!(out, "  {}:{:<5} {:<4} {:<56}  {}", shown, p.1, if in_loop.contains(p) { "loop" } else { "" }, short, kinds.join(", ")).unwrap();
    }
    let mut fns: Vec<(&String, &usize)> = per_fn.iter().filter(|(f, _)| !f.is_empty()).collect();
    fns.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let fns: Vec<String> = fns.iter().take(10).map(|(f, n)| format!("{} {}", f.trim_start_matches("f_"), n)).collect();
    writeln!(out, "By function: {}", fns.join(", ")).unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_are_found_by_whole_name() {
        assert_eq!(calls("    uint64_t t = cb_borrow_check(root, NULL, 0, CB_SHARED, 0u, 3u); cb_read(x);"), vec!["cb_borrow_check", "cb_read"]);
        assert!(calls("    cb_index_check(i, n, CB_NOLOC, 0); my_cb_read(1);").is_empty());
    }

    #[test]
    fn positions_are_the_last_marker() {
        assert_eq!(position("cb_at(0u, 12u); cb_stmt_push_at(1u, 13u);"), Some((1, 13)));
        assert_eq!(position("cb_stmt_push_at(CB_NOLOC, 0);"), None);
    }
}
