// Text passes over the generated C.

use super::*;

// The form of a native `Vec` body whose checks through the vector's own
// path (`p_v`) the call site has made (`lower_vec_checked_once`): those
// lines go; the test for an element with an object stays.
pub(super) fn once_variant(body: &str) -> String {
    body.lines()
        .filter(|l| !(l.contains("p_v.tok") && (l.trim_start().starts_with("cb_read(") || l.trim_start().starts_with("cb_write(") || l.trim_start().starts_with("cb_borrow_check("))))
        .map(|l| format!("{}\n", l))
        .collect()
}

// The unchecked form of a native `Vec` body, for a confined vector
// (`CONFINING_CALLS`): its checks through the vector's own path (`p_v`)
// cannot fail, since nothing but the root reaches a confined vector, and
// no element of one ever has an object, so the test for one (`cb_live_in`,
// and the fallback it guards) is always false. Each such check or test is
// a line of its own in those bodies; checks through other parameters
// (`p_key`, `p_x`) stay.
pub(super) fn nc_variant(body: &str) -> String {
    body.lines()
        .filter(|l| !(l.contains("p_v.tok") && (l.trim_start().starts_with("cb_read(") || l.trim_start().starts_with("cb_write(") || l.trim_start().starts_with("cb_borrow_check(")) || l.contains("cb_live_in")))
        .map(|l| format!("{}\n", l))
        .collect()
}

// Stage 3 (`COBC-PLAN.md` §10, T1): which statement scopes and frames
// a function body needs. Each one was emitted as a marker line: `F`/`S`
// its push, `f`/`s` its pop, `g`/`t` a pop made by a jump's unwind, `e`
// the end of its extent after a jump already left it. Walking the text
// keeps the stack of those open at each line, which is the runtime's
// own stack there (the code is structured). A scope or frame is kept
// only if some runtime call made while it is open could record
// something in it:
//
// - into the innermost statement scope (`top_scope`, which looks past
//   frames): temporaries (`cb_new`, `cb_recv`, `cb_take`) and derived
//   paths (`cb_borrow`, `cb_recv_ref`, and the `…_pre` element access,
//   which stamps its path into its caller's scope);
// - into the innermost frame (`top_frame`): `cb_bind`;
// - nothing: checks, slot bookkeeping and sends (`cb_read`, `cb_write`,
//   `cb_load_ref`, `cb_store_ref`, `cb_copy_datum`, `cb_move_to`,
//   `cb_send…`, `cb_borrow_check`, …) and calls of generated functions,
//   whose own scopes hold what they form — a result they hand back
//   arrives through a `cb_recv…` here;
// - any other runtime call keeps every scope and frame open around it.
//
// One that nothing records into would end nothing at `[Stmt-Exit]` or
// `[Block-Exit]`, so its push and every pop of it are dropped.
// The executing statement's location (`cb_at`) is read only by a fault
// raised without a location of its own (`CB_NOLOC`), and inside the
// runtime (pops, destructors, the checks it faults from). A `cb_at` is
// dropped when, in straight-line code, the next line to set it comes
// before any line that could read it: a call of anything but the pure
// helpers below, or a `cb_fault`/`cb_index_check` without a location of
// its own. Any brace or jump ends the straight line, and keeps it.
// Checks implied by an earlier one (a pass over a function's C). Within a
// run of lines with no other runtime call and no join (a label, a `}`,
// an `else`), the runtime's state cannot change, and a check is a pure
// function of it, so:
// - after an exclusive check through `base` with projection `P`
//   (`cb_borrow_unminted`/`cb_borrow_check` with `CB_EXCLUSIVE`), a read
//   or exclusive check through `base` under `P` cannot fail on a clash;
// - after a read or shared check under `P`, a read under `P` cannot;
// - after any check that tests initialization (`cb_read`,
//   `cb_borrow_check`) through `base`, none can fail on it (it is the
//   object's, and only a write changes it).
// Such a later `cb_read` / `cb_borrow_unminted` / `cb_borrow_check` is
// dropped. A projection with a range (a slice), and any base not written
// identically, are left alone.
// D-0200: the shared checks on a quiet local's root (`Gen::quiet_locals`):
// `cb_read(R, …)` and `cb_borrow_check(R, …, CB_SHARED, …)`, each a
// statement of its own, removed. No held exclusive path to the local
// exists in the function's own code, it is initialized where declared and
// never moved, so neither can fault.
pub(super) fn drop_quiet_checks(out: &str, roots: &HashSet<String>) -> String {
    if roots.is_empty() {
        return out.to_string();
    }
    let mut keep = String::with_capacity(out.len());
    for l in out.lines() {
        let t = l.trim();
        let drop = (t.starts_with("cb_read(") || (t.starts_with("cb_borrow_check(") && t.contains(", CB_SHARED, "))) && t.ends_with(");") && {
            let open = t.find('(').unwrap() + 1;
            let first = t[open..].split(',').next().unwrap_or("").trim();
            roots.contains(first)
        };
        if !drop {
            keep.push_str(l);
            keep.push('\n');
        }
    }
    if !out.ends_with('\n') {
        keep.pop();
    }
    keep
}

// D-0200: the checks through a token known to be 0 (the no-path token,
// `spec/14` §6), removed. A token variable is known 0 when its one
// assignment, its declaration, is `0`, `cb_borrow` of a token known 0 (which
// returns 0), or the `.tok` of a reference built with one, and its address
// is never taken. Then `cb_read`, `cb_write`, `cb_borrow_unminted`,
// `cb_borrow_check`, `cb_frame_hold` and `cb_frame_unhold` through it, each
// a statement of its own whose other arguments are constants and
// projection literals, return at once (`cbrt`): the statements go; and
// `cb_borrow` of it is 0. Repeated until nothing changes, as one fold
// makes the next token known.
pub(super) fn fold_zero_tokens(out: &str) -> String {
    fn ident_char(c: u8) -> bool {
        c.is_ascii_alphanumeric() || c == b'_'
    }
    // Each identifier assigned (`X = `, not `==`), and how often; each one
    // whose address is taken.
    fn scan(lines: &[String]) -> (HashMap<String, usize>, HashSet<String>) {
        let mut assigned: HashMap<String, usize> = HashMap::new();
        let mut addressed: HashSet<String> = HashSet::new();
        for l in lines {
            let b = l.as_bytes();
            let mut i = 0;
            while i < b.len() {
                if ident_char(b[i]) && (i == 0 || !ident_char(b[i - 1])) {
                    let st = i;
                    while i < b.len() && ident_char(b[i]) {
                        i += 1;
                    }
                    let id = &l[st..i];
                    if st > 0 && b[st - 1] == b'&' && (st < 2 || b[st - 2] != b'&') {
                        addressed.insert(id.to_string());
                    }
                    let rest = &l[i..];
                    if rest.starts_with(" = ") || (rest.starts_with("=") && !rest.starts_with("==")) {
                        *assigned.entry(id.to_string()).or_insert(0) += 1;
                    }
                } else {
                    i += 1;
                }
            }
        }
        (assigned, addressed)
    }
    // `name(args…)` exactly (with `;` after when `stmt`): the arguments.
    fn call_args<'l>(t: &'l str, name: &str, stmt: bool) -> Option<&'l str> {
        let t = if stmt { t.strip_suffix(';')? } else { t };
        let inner = t.strip_prefix(name)?.strip_prefix('(')?.strip_suffix(')')?;
        let mut depth = 0i32;
        for c in inner.chars() {
            match c {
                '(' | '{' | '[' => depth += 1,
                ')' | '}' | ']' => {
                    depth -= 1;
                    if depth < 0 {
                        return None;
                    }
                }
                _ => {}
            }
        }
        (depth == 0).then_some(inner)
    }
    fn first_arg(args: &str) -> &str {
        let mut depth = 0i32;
        for (i, c) in args.char_indices() {
            match c {
                '(' | '{' | '[' => depth += 1,
                ')' | '}' | ']' => depth -= 1,
                ',' if depth == 0 => return args[..i].trim(),
                _ => {}
            }
        }
        args.trim()
    }
    fn last_arg(args: &str) -> &str {
        let mut depth = 0i32;
        let mut at = 0;
        for (i, c) in args.char_indices() {
            match c {
                '(' | '{' | '[' => depth += 1,
                ')' | '}' | ']' => depth -= 1,
                ',' if depth == 0 => at = i + 1,
                _ => {}
            }
        }
        args[at..].trim()
    }
    // A constant, a projection literal, or reads of memory with no call and
    // no assignment (`((*(T *)r.p)).bytes.len`): nothing evaluated has an
    // effect.
    fn inert(a: &str) -> bool {
        if a.starts_with("(cb_proj[])") || a == "NULL" {
            return true;
        }
        let b = a.as_bytes();
        !(0..b.len()).any(|i| b[i] == b'(' && i > 0 && ident_char(b[i - 1]))
            && !a.contains('=')
            && !a.contains("++")
            && !a.contains("--")
    }
    const DROP: &[&str] = &["cb_read", "cb_write", "cb_borrow_unminted", "cb_borrow_check", "cb_frame_hold", "cb_frame_unhold", "cb_borrow_range_unminted"];
    if !DROP.iter().any(|d| out.contains(d)) {
        return out.to_string();
    }
    let mut lines: Vec<String> = out.lines().map(|l| l.to_string()).collect();
    let mut zero: HashSet<String> = HashSet::new();
    let mut zref: HashSet<String> = HashSet::new();
    loop {
        let (assigned, addressed) = scan(&lines);
        let once = |x: &str| assigned.get(x) == Some(&1) && !addressed.contains(x);
        let is_zero = |a: &str, zero: &HashSet<String>, zref: &HashSet<String>| -> bool {
            let a = a.trim();
            if a == "0" || zero.contains(a) {
                return true;
            }
            let r = a.strip_suffix(".tok").map(|r| r.trim_start_matches('(').trim_end_matches(')'));
            r.map_or(false, |r| zref.contains(r))
        };
        let mut changed = false;
        let mut keep: Vec<String> = Vec::with_capacity(lines.len());
        for l in &lines {
            let t = l.trim();
            // A check through a token known 0: the statement goes.
            if DROP.iter().any(|d| {
                call_args(t, d, true).map_or(false, |args| {
                    let mut parts = Vec::new();
                    let (mut depth, mut cur) = (0i32, String::new());
                    for c in args.chars() {
                        match c {
                            '(' | '{' | '[' => {
                                depth += 1;
                                cur.push(c);
                            }
                            ')' | '}' | ']' => {
                                depth -= 1;
                                cur.push(c);
                            }
                            ',' if depth == 0 => {
                                parts.push(std::mem::take(&mut cur));
                            }
                            _ => cur.push(c),
                        }
                    }
                    parts.push(cur);
                    is_zero(&parts[0], &zero, &zref) && parts[1..].iter().all(|p| inert(p.trim()))
                })
            }) {
                // After anything but a statement's or block's end (a
                // braceless `if`'s condition, a label) an empty statement
                // stands in its place.
                let prev = keep.iter().rev().map(|k| k.trim()).find(|k| !k.is_empty()).unwrap_or("{");
                if !(prev.ends_with(';') || prev.ends_with('{') || prev.ends_with('}')) {
                    let ind = &l[..l.len() - l.trim_start().len()];
                    keep.push(format!("{};", ind));
                }
                changed = true;
                continue;
            }
            if let Some(decl) = t.strip_prefix("uint64_t ").and_then(|r| r.strip_suffix(';')) {
                if let Some((x, e)) = decl.split_once(" = ") {
                    let x = x.trim();
                    if x.bytes().all(ident_char) && once(x) {
                        let e = e.trim();
                        // `cb_borrow(0, …)` and `cb_borrow_range(0, …)` are 0.
                        if let Some(args) = call_args(e, "cb_borrow", false).or_else(|| call_args(e, "cb_borrow_range", false).filter(|a| {
                            let mut depth = 0i32;
                            let mut parts = vec![String::new()];
                            for c in a.chars() {
                                match c {
                                    '(' | '{' | '[' => depth += 1,
                                    ')' | '}' | ']' => depth -= 1,
                                    _ => {}
                                }
                                if c == ',' && depth == 0 {
                                    parts.push(String::new());
                                } else {
                                    parts.last_mut().unwrap().push(c);
                                }
                            }
                            parts[1..].iter().all(|p| inert(p.trim()))
                        })) {
                            if is_zero(first_arg(args), &zero, &zref) {
                                let ind = &l[..l.len() - l.trim_start().len()];
                                keep.push(format!("{}uint64_t {} = 0;", ind, x));
                                zero.insert(x.to_string());
                                changed = true;
                                continue;
                            }
                        }
                        if !zero.contains(x) && is_zero(e, &zero, &zref) {
                            zero.insert(x.to_string());
                            changed = true;
                        }
                    }
                }
            }
            // `cb_ref R = ((cb_ref){ p, tok });` with the token known 0.
            if let Some(decl) = t.strip_prefix("cb_ref ").and_then(|r| r.strip_suffix(';')) {
                if let Some((x, e)) = decl.split_once(" = ") {
                    let x = x.trim();
                    if x.bytes().all(ident_char) && once(x) && !zref.contains(x) {
                        if let Some(inner) = e.trim().strip_prefix("((cb_ref){").and_then(|r| r.strip_suffix("})")) {
                            if is_zero(last_arg(inner), &zero, &zref) {
                                zref.insert(x.to_string());
                                changed = true;
                            }
                        }
                    }
                }
            }
            keep.push(l.clone());
        }
        lines = keep;
        if !changed {
            break;
        }
    }
    let mut s = lines.join("\n");
    if out.ends_with('\n') {
        s.push('\n');
    }
    s
}

pub(super) fn elide_implied_checks(out: &str) -> String {
    // (base, projection, exclusive) checked, and bases whose object's
    // initialization was checked.
    let mut seen: Vec<(String, Vec<String>, bool)> = Vec::new();
    let mut inited: HashSet<String> = HashSet::new();
    // `cb_stmt_push`/`cb_stmt_push_at` open a scope: no path's state changes.
    const PURE: &[&str] = &["cb_read", "cb_borrow_unminted", "cb_borrow_check", "cb_at", "cb_fault", "cb_index_check", "cb_hash_bytes", "cb_bytes_eq", "cb_bytes_lt", "cb_load_ref", "cb_stmt_push", "cb_stmt_push_at"];
    // `uint64_t tkN = <token>;` copies: a check through either names one path.
    let mut alias: HashMap<String, String> = HashMap::new();
    fn split_args(s: &str) -> Vec<String> {
        let mut out = Vec::new();
        let (mut depth, mut cur) = (0i32, String::new());
        for c in s.chars() {
            match c {
                '(' | '{' | '[' => {
                    depth += 1;
                    cur.push(c);
                }
                ')' | '}' | ']' => {
                    depth -= 1;
                    cur.push(c);
                }
                ',' if depth == 0 => {
                    out.push(cur.trim().to_string());
                    cur.clear();
                }
                _ => cur.push(c),
            }
        }
        if !cur.trim().is_empty() {
            out.push(cur.trim().to_string());
        }
        out
    }
    // The projection's steps, or None if it has a range.
    fn projs(a: &str) -> Option<Vec<String>> {
        if a == "NULL" {
            return Some(Vec::new());
        }
        let inner = a.strip_prefix("(cb_proj[]){")?.strip_suffix('}')?;
        let mut v = Vec::new();
        for st in inner.split("},") {
            let st = st.trim().trim_start_matches('{').trim_end_matches('}').trim();
            if st.is_empty() {
                continue;
            }
            if !(st.starts_with("CB_FIELD") || st.starts_with("CB_PAYLOAD")) {
                return None;
            }
            v.push(st.split_whitespace().collect::<Vec<_>>().join(""));
        }
        Some(v)
    }
    let calls = |line: &str| -> Vec<String> {
        let b = line.as_bytes();
        let mut v = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i].is_ascii_alphabetic() || b[i] == b'_' {
                let st = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                if i < b.len() && b[i] == b'(' && (st == 0 || !(b[st - 1].is_ascii_alphanumeric() || b[st - 1] == b'_')) {
                    v.push(line[st..i].to_string());
                }
            } else {
                i += 1;
            }
        }
        v
    };
    let mut res = String::with_capacity(out.len());
    for line in out.lines() {
        let t = line.trim_start();
        // A join, or control arriving from elsewhere: forget everything.
        if t.starts_with('}') || t.starts_with("else") || (t.ends_with(':') && !t.contains(' ')) || t.starts_with("case ") || t.starts_with("default") {
            seen.clear();
            inited.clear();
            res.push_str(line);
            res.push('\n');
            continue;
        }
        if let Some(rest) = t.strip_prefix("uint64_t ") {
            if let Some((name, val)) = rest.strip_suffix(';').and_then(|r| r.split_once(" = ")) {
                let simple = |v: &str| v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '(' || c == ')');
                if !name.contains(' ') && simple(val) && !val.contains("()") && !val.chars().next().map_or(true, |c| c.is_ascii_digit()) {
                    let canon = alias.get(val).cloned().unwrap_or_else(|| val.to_string());
                    alias.insert(name.to_string(), canon);
                }
            }
        }
        let cs = calls(t);
        let kind = ["cb_read(", "cb_borrow_unminted(", "cb_borrow_check("].iter().find(|k| t.starts_with(**k)).map(|k| &k[..k.len() - 1]);
        if let (Some(k), true) = (kind, t.ends_with(");")) {
            let args = split_args(&t[k.len() + 1..t.len() - 2]);
            let base0 = args.first().cloned().unwrap_or_default();
            let base = alias.get(&base0).cloned().unwrap_or(base0);
            let pr = args.get(1).and_then(|a| projs(a));
            let excl = match k {
                "cb_read" => false,
                _ => args.get(3).map_or(false, |m| m == "CB_EXCLUSIVE"),
            };
            let tests_init = k != "cb_borrow_unminted";
            if let Some(p) = pr {
                let covered = seen.iter().any(|(b, q, ex)| *b == base && p.starts_with(q) && (*ex || !excl));
                let init_ok = !tests_init || inited.contains(&base);
                if covered && init_ok {
                    continue;
                }
                seen.push((base.clone(), p, excl));
                if tests_init {
                    inited.insert(base);
                }
            }
            res.push_str(line);
            res.push('\n');
            continue;
        }
        // Any other call may change the runtime's state.
        if cs.iter().any(|c| !PURE.contains(&c.as_str()) && !c.starts_with("__builtin") && !matches!(c.as_str(), "if" | "while" | "for" | "switch" | "sizeof" | "return")) {
            seen.clear();
            inited.clear();
        }
        res.push_str(line);
        res.push('\n');
    }
    res
}

// Whether generated C calls nothing that could call back into the
// program: only C keywords, builtins, and the runtime's location
// bookkeeping (`cb_at`) and faults (`cb_fault`, which does not return).
// A destructor the runtime runs, a `fn` value or closure, and any other
// runtime call count as calls.
pub(super) fn calls_nothing(body: &str) -> bool {
    const LEAF: &[&str] = &["cb_at", "cb_fault", "if", "while", "for", "switch", "sizeof", "offsetof", "return"];
    let b = body.as_bytes();
    let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut i = 0;
    while i < b.len() {
        if !is_id(b[i]) || (i > 0 && is_id(b[i - 1])) || b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut e = i;
        while e < b.len() && is_id(b[e]) {
            e += 1;
        }
        let id = &body[i..e];
        let mut k = e;
        while k < b.len() && b[k] == b' ' {
            k += 1;
        }
        if b.get(k) == Some(&b'(') && !LEAF.contains(&id) && !id.starts_with("__builtin") {
            return false;
        }
        i = e;
    }
    true
}

// `marks`: a location dropped is kept as a `LINE_MARK` comment for
// `line_directives`, which removes it; it reads as nothing to the one
// pass that runs between (`elide_implied_checks`: no call in it).
pub(super) fn drop_dead_locations(out: &str, marks: bool) -> String {
    const PURE: &[&str] = &["cb_at", "cb_str_eq", "cb_live_in", "if", "while", "for", "sizeof", "offsetof", "__builtin_expect", "__builtin_add_overflow", "__builtin_sub_overflow", "__builtin_mul_overflow"];
    fn reads_location(line: &str) -> bool {
        let b = line.as_bytes();
        let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let mut i = 0;
        while i < b.len() {
            if !is_id(b[i]) || (i > 0 && is_id(b[i - 1])) || b[i].is_ascii_digit() {
                i += 1;
                continue;
            }
            let mut e = i;
            while e < b.len() && is_id(b[e]) {
                e += 1;
            }
            let id = &line[i..e];
            let mut k = e;
            while k < b.len() && b[k] == b' ' {
                k += 1;
            }
            if b.get(k) == Some(&b'(') && !PURE.contains(&id) {
                if id != "cb_fault" && id != "cb_index_check" && id != "cb_elem_access" && id != "cb_elem_access_if" {
                    return true;
                }
                // Its own location, unless that is `CB_NOLOC`: up to the
                // call's closing parenthesis.
                let mut depth = 0;
                let mut j = k;
                while j < b.len() {
                    match b[j] {
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                if line[k..j.min(b.len())].contains("CB_NOLOC") {
                    return true;
                }
            }
            i = e;
        }
        false
    }
    fn boundary(t: &str) -> bool {
        t.ends_with('{') || t.starts_with('}') || t.starts_with("break") || t.starts_with("continue") || t.starts_with("return") || t.starts_with("__builtin_unreachable") || t.ends_with(':')
    }
    let lines: Vec<&str> = out.lines().collect();
    let mut res = String::with_capacity(out.len());
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("cb_at(") {
            let mut dead = false;
            for next in &lines[i + 1..] {
                let t = next.trim_start();
                if t.starts_with("cb_at(") {
                    dead = true;
                    break;
                }
                if boundary(t) || reads_location(t) {
                    break;
                }
            }
            if dead {
                if marks {
                    if let Some((f, l)) = at_of(line.trim_start()) {
                        writeln!(res, "{}{} {}*/", LINE_MARK, f, l).unwrap();
                    }
                }
                continue;
            }
        }
        res.push_str(line);
        res.push('\n');
    }
    res
}

pub(super) fn resolve_scopes(out: &str) -> String {
    resolve_scopes_in(out, false)
}

// `resolve_scopes`, `decided` for a decided program's lowering (D-0285): the
// entries `cbrt.h` makes constants there (`CB_PATHLESS`, `CB_DECIDED`) record
// nothing in any scope, and an element's reference (`_pre`) has no path.
pub(super) fn resolve_scopes_in(out: &str, decided: bool) -> String {
    const DECIDED_FREE: &[&str] = &[
        "cb_borrow", "cb_borrow_range", "cb_elem_borrow", "cb_elem_borrow_here", "cb_elem_borrow_datum", "cb_valid", "cb_frame_hold", "cb_frame_unhold",
        "cb_send_tok_datum", "cb_send_datum_none", "cb_recv_datum", "cb_recv_datum_tok", "cb_peek_datum_tok", "cb_frozen_ok",
    ];
    enum Eff {
        Scope,
        Frame,
        All,
    }
    const FREE: &[&str] = &[
        "cb_fault", "cb_fault_msg", "cb_index_check", "cb_str_eq", "cb_write_out", "cb_write_err", "cb_hash_bytes", "cb_bytes_eq", "cb_read_in", "cb_arg_bytes", "cb_set_args", "cb_text_int", "cb_text_f64", "cb_text_f32", "cb_text_bool", "cb_parse_int", "cb_parse_float", "cb_parse_bool", "cb_frame_here", "cb_file_read", "cb_file_write", "cb_file_op", "cb_file_at", "cb_fs_op", "cb_fs_query", "cb_clock_read", "cb_event_op", "cb_bytes_less", "cb_format", "cb_at", "cb_unit", "cb_str", "cb_i128", "cb_u128", "cb_ref", "cb_proj", "cb_fmt_arg",
        // A type in a cast (D-0239's test of a box's kind before a call).
        "cb_fnbox",
        // Pure inline helpers (`cbrt.h`), and a query that records nothing.
        "cb_bytes_lt", "cb_bytes_cmp", "cb_find_bytes", "cb_char_boundary", "cb_popcount64", "cb_popcount128", "cb_clz", "cb_clz128", "cb_ctz", "cb_ctz128", "cb_rotl", "cb_rotl128", "cb_rotr", "cb_rotr128", "cb_rotl8", "cb_rotl16", "cb_rotl32", "cb_rotl64", "cb_rotr8", "cb_rotr16", "cb_rotr32", "cb_rotr64", "cb_fnv_more", "cb_live_at",
        "cb_read", "cb_write", "cb_write_quiet", "cb_borrow_unminted", "cb_borrow_range_unminted", "cb_borrow_unstamped", "cb_call_done", "cb_load_ref", "cb_store_ref", "cb_hold_ref", "cb_copy_datum", "cb_move_to", "cb_send", "cb_send_ref", "cb_send_datum", "cb_borrow_check", "cb_elem_access", "cb_elem_access_if", "cb_live_in",
        // A question about the value at an address (`owns`), recording nothing.
        "cb_owns",
        // D-0269: a frame's table of objectless locals and its registration
        // (`Fx::notes_finish`): the thread's own state, no scope's.
        "cb_note", "cb_nt", "cb_nt_on", "cb_notes_push", "cb_notes_pop", "cb_threaded",
        // Raw memory from the allocator (`lower_box_new`): recorded for
        // `deallocate`, in no scope.
        "cb_allocate",
        // An objectless local's drop: no record, no scope (D-0200: and its
        // inline form); the stack's depth check records nothing either.
        "cb_drop_local", "cb_drop_plain_buf", "cb_stack_check",
        // D-0249: a schedule's point at an atomic operation, recording nothing.
        "cb_atomic_point",
        // D-0253: an atomic's wake count, and the processor's pause; neither
        // records anything in a scope or a frame.
        "cb_atomic_wait", "cb_spin_hint",
    ];
    // `cb_absorb` ends a temporary made in the statement (a struct or
    // variant literal's part): it detaches that object from its own
    // record, whatever scopes are open, so it needs only that statement.
    // `cb_absorb_refs` (a part holding references) does the same.
    // `cb_elem_borrow` mints a path and puts it in flight; the caller's
    // `cb_recv_ref` stamps it into the statement.
    // `cb_raw_move_in` (`lower_box_new`) ends or detaches a value made in
    // the statement, as `cb_absorb` does.
    // `cb_recv_datum` and `cb_recv_datum_tok` stamp what they receive into
    // the statement, as `cb_recv_ref` does.
    const SCOPE: &[&str] = &["cb_new", "cb_new_uninit", "cb_recv", "cb_recv_ref", "cb_rec_ref", "cb_recv_datum", "cb_recv_datum_tok", "cb_forget_obj", "cb_take", "cb_borrow", "cb_borrow_range", "cb_absorb", "cb_absorb_refs", "cb_elem_borrow", "cb_raw_move_in"];
    let effects = |line: &str, out: &mut Vec<Eff>| {
        let b = line.as_bytes();
        let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let mut i = 0;
        while i < b.len() {
            if !is_id(b[i]) || (i > 0 && is_id(b[i - 1])) {
                i += 1;
                continue;
            }
            let mut e = i;
            while e < b.len() && is_id(b[e]) {
                e += 1;
            }
            let id = &line[i..e];
            // A string literal's array is data: `cb_s_` and its content
            // hash, `x`s after it on a collision (`lower_expr`'s `StrLit`),
            // or the older `cb_s0`, `cb_s1`, … .
            let literal = (id.len() > 4 && id.starts_with("cb_s") && id[4..].bytes().all(|c| c.is_ascii_digit()))
                || (id.len() >= 13 && id.starts_with("cb_s_") && id[5..13].bytes().all(|c| c.is_ascii_hexdigit()) && id[13..].bytes().all(|c| c == b'x'));
            if id.starts_with("cb_") && !literal {
                // A fn value's call thunk (`Gen::thunk`) records nothing in
                // the caller's scopes (`cb_call_self`), as a direct call.
                if id.starts_with("cb_call_") || (decided && DECIDED_FREE.contains(&id)) {
                } else if SCOPE.contains(&id) {
                    out.push(Eff::Scope);
                } else if id == "cb_bind" {
                    out.push(Eff::Frame);
                } else if !FREE.contains(&id) {
                    out.push(Eff::All);
                }
            } else if !decided && id.ends_with("_pre") && b.get(e) == Some(&b'(') {
                out.push(Eff::Scope);
            }
            i = e;
        }
    };
    fn marker(line: &str) -> Option<(char, u32)> {
        let m = line.trim_start().strip_prefix('\u{1}')?;
        let k = m.chars().next()?;
        let id = m[1..].split('\u{2}').next().unwrap_or("");
        Some((k, id.parse().expect("scope marker id")))
    }
    // A frame marker's location, when it carries one.
    fn marker_loc(line: &str) -> Option<&str> {
        line.trim_start().strip_prefix('\u{1}')?.split_once('\u{2}').map(|(_, l)| l)
    }

    let mut open: Vec<(bool, u32)> = Vec::new();
    let mut used: HashSet<u32> = HashSet::new();
    let mut effs = Vec::new();
    for line in out.lines() {
        match marker(line) {
            Some(('F' | 'G', id)) => open.push((true, id)),
            // D-0292: a recording body's frame is always emitted.
            Some(('R', id)) => {
                open.push((true, id));
                used.insert(id);
            }
            Some(('S', id)) => open.push((false, id)),
            Some(('f' | 's' | 'e', id)) => {
                let top = open.pop();
                assert_eq!(top.map(|t| t.1), Some(id), "cobc: scope markers out of order");
            }
            Some(_) => {}
            None => {
                effs.clear();
                effects(line, &mut effs);
                for e in &effs {
                    match e {
                        Eff::Scope => used.extend(open.iter().rev().find(|t| !t.0).map(|t| t.1)),
                        Eff::Frame => used.extend(open.iter().rev().find(|t| t.0).map(|t| t.1)),
                        Eff::All => used.extend(open.iter().map(|t| t.1)),
                    }
                }
            }
        }
    }
    assert!(open.is_empty(), "cobc: scope markers left open");

    let mut res = String::with_capacity(out.len());
    for line in out.lines() {
        match marker(line) {
            Some((k, id)) => {
                let at = marker_loc(line).map(|l| format!("cb_{}_push_at({});", if k == 'F' { "frame" } else { "stmt" }, l));
                let call = match k {
                    'F' => at.as_deref().unwrap_or("cb_frame_push();"),
                    'G' => "cb_frame_push_fn();",
                    'R' => "cb_frame_push_rec();",
                    'S' => at.as_deref().unwrap_or("cb_stmt_push();"),
                    'f' | 'g' => "cb_frame_pop();",
                    's' | 't' => "cb_stmt_pop();",
                    _ => continue,
                };
                if used.contains(&id) {
                    let ind = &line[..line.len() - line.trim_start().len()];
                    res.push_str(ind);
                    res.push_str(call);
                    res.push('\n');
                }
            }
            None => {
                res.push_str(line);
                res.push('\n');
            }
        }
    }
    res
}

// A function's length in lines for the size limits, `LINE_MARK`s not
// counted: the directives change no decision.
pub(super) fn code_lines(text: &str) -> usize {
    text.lines().filter(|l| !l.starts_with(LINE_MARK)).count()
}

pub(super) fn line_directives(text: &str, files: &[String]) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    let mut cur: Option<(usize, usize)> = None;
    let mut dirty = false;
    for l in text.split_inclusive('\n') {
        let body = l.trim_end_matches('\n');
        let t = body.trim_start();
        let starts_definition = !body.is_empty() && body.len() == t.len() && !t.starts_with(['{', '}', '#', '/']);
        if starts_definition {
            if dirty {
                out.push_str(LINE_RESET);
                out.push('\n');
                dirty = false;
            }
            cur = None;
        }
        if let Some(at) = t.strip_prefix(LINE_MARK).and_then(|m| m.strip_suffix("*/")).and_then(|m| m.split_once(' ')).and_then(|(f, l)| Some((f.parse().ok()?, l.parse().ok()?))) {
            cur = Some(at).filter(|(f, _): &(usize, usize)| *f < files.len());
            continue;
        }
        if let Some(at) = at_of(t).filter(|(f, _)| *f < files.len()) {
            cur = Some(at);
        }
        if let Some((f, n)) = cur.filter(|_| !t.is_empty() && !t.starts_with('#')) {
            writeln!(out, "#line {} \"{}\"", n, c_escape(&files[f])).unwrap();
            dirty = true;
        }
        out.push_str(l);
    }
    if dirty {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(LINE_RESET);
        out.push('\n');
    }
    out
}

// `cb_at(3u, 12u);`: file 3, line 12 (`Lower::at`).
pub(super) fn at_of(t: &str) -> Option<(usize, usize)> {
    let rest = t.strip_prefix("cb_at(")?.strip_suffix(");")?;
    let (f, l) = rest.split_once(", ")?;
    Some((f.strip_suffix('u')?.parse().ok()?, l.strip_suffix('u')?.parse().ok()?))
}

pub(super) fn c_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(super) fn resolve_arrive(text: &str, twins: &HashSet<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(ARRIVE_MARK) {
        let head = &rest[..i];
        let start = head.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map_or(0, |k| k + 1);
        let name = &head[start..];
        out.push_str(head);
        if twins.contains(name) {
            out.push_str("__arrive");
        }
        rest = &rest[i + ARRIVE_MARK.len()..];
    }
    out.push_str(rest);
    out
}
