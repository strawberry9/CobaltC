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

// D-0294: in a decided program a record that nothing reads is not kept. An
// object made (`uint64_t o = cb_new(&v, …);`), perhaps bound and taken back
// (`cb_bind`, `cb_take`), moved (`cb_move_to`), promoted (`cb_result`) and
// ended (`cb_forget_obj`, `cb_absorb`, `cb_raw_move_in`) in one straight
// run of statements -- no branch, no call that could fault (a destructor
// run at the fault is observable) and no frame's end between, a scope's end
// only once the object was promoted past it or is bound -- with its ids
// named nowhere else, is a record no check reads (every path entry is a
// constant there) whose end comes before any scope's that could destroy
// the value: a struct or variant literal that leaves as a result or an
// argument, a `std` body's result bound and taken back. The statements go.
// Run before the scopes are resolved, so they open no scope for it.
pub(super) fn elide_record_chains(out: &str) -> String {
    fn ident_char(c: u8) -> bool {
        c.is_ascii_alphanumeric() || c == b'_'
    }
    fn count_in(text: &str, x: &str) -> usize {
        let b = text.as_bytes();
        let mut n = 0;
        let mut at = 0;
        while let Some(k) = text[at..].find(x) {
            let i = at + k;
            let j = i + x.len();
            if (i == 0 || !ident_char(b[i - 1])) && (j >= b.len() || !ident_char(b[j])) {
                n += 1;
            }
            at = j;
        }
        n
    }
    // `uint64_t NAME = CALL(ARG…);`: (NAME, the text after `CALL(`).
    fn decl<'l>(t: &'l str, call: &str) -> Option<(&'l str, &'l str)> {
        let rest = t.strip_prefix("uint64_t ")?;
        let eq = rest.find(" = ")?;
        let name = &rest[..eq];
        if name.is_empty() || !name.bytes().all(ident_char) {
            return None;
        }
        let args = rest[eq + 3..].strip_prefix(call)?.strip_prefix('(')?.strip_suffix(");")?;
        Some((name, args))
    }
    fn first_arg(args: &str) -> &str {
        args.split(',').next().unwrap_or("").trim()
    }
    const CONTROL: &[&str] = &["if ", "if(", "else", "while ", "while(", "for ", "for(", "switch", "case ", "default:", "do ", "do{", "return", "goto ", "break;", "continue;"];
    let mut lines: Vec<String> = out.lines().map(|l| l.to_string()).collect();
    let mut drop: Vec<usize> = Vec::new();
    for i in 0..lines.len() {
        let Some((start, _)) = decl(lines[i].trim(), "cb_new") else { continue };
        let start = start.to_string();
        let mut live = start.clone();
        let mut ids: Vec<String> = vec![start.clone()];
        let mut chain: Vec<usize> = vec![i];
        let mut root: Option<String> = None;
        let mut promoted = false;
        let mut ok = false;
        for j in i + 1..lines.len() {
            let t = lines[j].trim();
            if t.is_empty() || t.starts_with("#line") {
                continue;
            }
            let named = ids.iter().any(|x| count_in(t, x) > 0);
            if !named {
                if t == "cb_stmt_pop();" {
                    if root.is_none() && !promoted {
                        break;
                    }
                    promoted = false;
                    continue;
                }
                // A statement that could fault (any call: a destructor the
                // fault would run on the value is observable) ends the run;
                // a plain store or declaration, a position, a scope's start
                // cannot.
                let plain = !t.contains('(') || t.starts_with("cb_at(") || t.starts_with("cb_stmt_push_at(");
                if !plain || t.contains('{') || t.contains('}') || !t.ends_with(';') || CONTROL.iter().any(|k| t.starts_with(k)) {
                    break;
                }
                continue;
            }
            if let Some((r, args)) = decl(t, "cb_bind") {
                if root.is_some() || args != live {
                    break;
                }
                root = Some(r.to_string());
                ids.push(r.to_string());
                chain.push(j);
                continue;
            }
            if let Some((x, args)) = decl(t, "cb_take") {
                if root.as_deref() != Some(first_arg(args)) {
                    break;
                }
                root = None;
                live = x.to_string();
                ids.push(x.to_string());
                promoted = false;
                chain.push(j);
                continue;
            }
            if root.is_some() {
                break;
            }
            if t == format!("cb_result({});", live) {
                promoted = true;
                chain.push(j);
                continue;
            }
            if t.strip_prefix("cb_move_to(").and_then(|a| a.strip_suffix(");")).map_or(false, |a| first_arg(a) == live && a.contains(", &")) {
                chain.push(j);
                continue;
            }
            if t == format!("cb_forget_obj({});", live) || t == format!("cb_absorb({});", live) || t.strip_prefix("cb_raw_move_in(").and_then(|a| a.strip_suffix(");")).map_or(false, |a| first_arg(a) == live) {
                chain.push(j);
                ok = true;
                break;
            }
            break;
        }
        if !ok {
            continue;
        }
        // Every id named only in the chain's own statements.
        let in_chain = |x: &str| chain.iter().map(|&k| count_in(&lines[k], x)).sum::<usize>();
        if ids.iter().any(|x| count_in(out, x) != in_chain(x)) {
            continue;
        }
        drop.extend(chain);
    }
    if drop.is_empty() {
        return out.to_string();
    }
    for &i in &drop {
        lines[i] = String::from(";");
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
    const PURE: &[&str] = &["cb_read", "cb_borrow_unminted", "cb_borrow_check", "cb_at", "cb_fault", "cb_index_check", "cb_hash_bytes", "cb_bytes_eq", "cb_bytes_lt", "cb_load_ref", "cb_stmt_push", "cb_stmt_push_at", "cb_fmt_space", "cb_fmt_u64", "cb_fmt_i64", "cb_fmt_hex", "cb_fmt_pad"];
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

// The calls that read no location.
const LOCATION_PURE: &[&str] = &["cb_at", "cb_str_eq", "cb_live_in", "if", "while", "for", "sizeof", "offsetof", "__builtin_expect", "__builtin_add_overflow", "__builtin_sub_overflow", "__builtin_mul_overflow"];

// Whether a line could read the executing statement's location: a call of
// anything but the pure helpers, or a fault or check whose own location is
// `CB_NOLOC`.
pub(super) fn reads_location(line: &str) -> bool {
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
        if b.get(k) == Some(&b'(') && !LOCATION_PURE.contains(&id) {
            // A confined vector's push (`native_vec_push`'s `_nc` form)
            // sets its own location where it can fault.
            let own = (id.starts_with("f_std__Vec__push_") && id.ends_with("_nc")) || matches!(id, "cb_fault" | "cb_index_check" | "cb_elem_access" | "cb_elem_access_if");
            if !own {
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

// The `cb_at` lines `drop_dead_locations` drops, in straight-line code
// only: the next line to set the location comes before any line that
// could read it, and before any brace or jump.
fn dead_locations_straight(lines: &[&str]) -> Vec<bool> {
    fn boundary(t: &str) -> bool {
        t.ends_with('{') || t.starts_with('}') || t.starts_with("break") || t.starts_with("continue") || t.starts_with("return") || t.starts_with("__builtin_unreachable") || t.ends_with(':')
    }
    let mut dead = vec![false; lines.len()];
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("cb_at(") {
            for next in &lines[i + 1..] {
                let t = next.trim_start();
                if t.starts_with("cb_at(") {
                    dead[i] = true;
                    break;
                }
                if boundary(t) || reads_location(t) {
                    break;
                }
            }
        }
    }
    dead
}

// A body's statement as `dead_locations_flow` sees it: an opening line, a
// closing `}`, a jump (perhaps under an `if`), a label, or any other line.
#[derive(Clone, Copy, PartialEq)]
enum FlowLine<'a> {
    Block,
    If,
    ElseIf,
    Else,
    Forever,
    Loop,
    Close,
    Break(bool),
    Continue(bool),
    Goto(&'a str, bool),
    Exit(bool),
    Label(&'a str),
    Other,
}

// The jump a statement makes, under an `if (…)` or not (the bool).
fn flow_jump(t: &str) -> Option<FlowLine<'_>> {
    let (tail, cond) = match t.strip_prefix("if (") {
        Some(rest) => {
            let mut depth = 1;
            let mut end = None;
            for (k, c) in rest.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(k);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            (rest[end? + 1..].strip_prefix(' ')?, true)
        }
        None => (t, false),
    };
    let is_label = |x: &str| !x.is_empty() && x.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_');
    if tail == "break;" {
        Some(FlowLine::Break(cond))
    } else if tail == "continue;" {
        Some(FlowLine::Continue(cond))
    } else if let Some(l) = tail.strip_prefix("goto ").and_then(|x| x.strip_suffix(';')).filter(|x| is_label(x)) {
        Some(FlowLine::Goto(l, cond))
    } else if (tail == "return;" || tail.starts_with("return ")) && tail.ends_with(';') || tail == "__builtin_unreachable();" {
        Some(FlowLine::Exit(cond))
    } else {
        None
    }
}

// Whether a line names `w` as a whole word.
fn has_word(t: &str, w: &str) -> bool {
    let b = t.as_bytes();
    let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut from = 0;
    while let Some(k) = t[from..].find(w) {
        let (s, e) = (from + k, from + k + w.len());
        if (s == 0 || !is_id(b[s - 1])) && (e == b.len() || !is_id(b[e])) {
            return true;
        }
        from = e;
    }
    false
}

// The `cb_at` lines `drop_dead_locations` drops, over the body's flow of
// control: a location is dead where no path from it reaches a line that
// could read it before a line that sets it again, and of those left, one
// is dropped where every path to it has set that location already (and
// called nothing since). A `return` reads none: a caller sets its own
// before reading it after a call. None when the
// body holds a line this does not follow (a `switch`, a `do`, a jump out
// of a statement expression).
fn dead_locations_flow(lines: &[&str], tail_entry: Option<bool>) -> Option<Vec<bool>> {
    // `} else {` and `} else if (…) {` as two lines: the branch's close,
    // then the next branch.
    let mut node_line: Vec<usize> = Vec::with_capacity(lines.len());
    let mut texts: Vec<&str> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("} else") {
            node_line.push(i);
            texts.push("}");
            node_line.push(i);
            texts.push(&t[2..]);
            if !rest.ends_with('{') {
                return None;
            }
        } else {
            node_line.push(i);
            texts.push(t);
        }
    }
    let n = texts.len();
    let mut kind: Vec<FlowLine> = Vec::with_capacity(n);
    for t in &texts {
        let k = if t.ends_with('{') {
            if *t == "{" {
                FlowLine::Block
            } else if *t == "for (;;) {" {
                FlowLine::Forever
            } else if t.starts_with("for (") {
                FlowLine::Loop
            } else if t.starts_with("if (") {
                FlowLine::If
            } else if t.starts_with("else if (") {
                FlowLine::ElseIf
            } else if *t == "else {" {
                FlowLine::Else
            } else {
                return None;
            }
        } else if t.starts_with('}') {
            if *t != "}" {
                return None;
            }
            FlowLine::Close
        } else if let Some(j) = flow_jump(t) {
            j
        } else if let Some(l) = t.strip_suffix(';').unwrap_or(t).trim_end().strip_suffix(':').filter(|l| !l.is_empty() && l.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')) {
            FlowLine::Label(l)
        } else {
            // A jump inside a line leaves only that line's own statement.
            // A statement under an `if`, `for` or `while` on the next line.
            let head = ["if (", "for (", "while ("].iter().any(|h| t.starts_with(h)) && !t.ends_with(';');
            if head || t.ends_with("else") || has_word(t, "goto") || has_word(t, "return") || t.starts_with("else") || t.starts_with("case ") || t.starts_with("default:") {
                return None;
            }
            if (t.contains("break;") || t.contains("continue;")) && !(t.contains("switch (") || t.contains("for (") || t.contains("while (") || t.contains("do {")) {
                return None;
            }
            FlowLine::Other
        };
        kind.push(k);
    }
    // The braces matched, each line's innermost loop, and the labels.
    let mut close_of = vec![usize::MAX; n];
    let mut open_of = vec![usize::MAX; n];
    let mut loop_of = vec![usize::MAX; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut loops: Vec<usize> = Vec::new();
    let mut labels: HashMap<&str, usize> = HashMap::new();
    for i in 0..n {
        loop_of[i] = loops.last().copied().unwrap_or(usize::MAX);
        match kind[i] {
            FlowLine::Block | FlowLine::If | FlowLine::ElseIf | FlowLine::Else | FlowLine::Forever | FlowLine::Loop => {
                if matches!(kind[i], FlowLine::ElseIf | FlowLine::Else) && (i == 0 || kind[i - 1] != FlowLine::Close || !matches!(kind[open_of[i - 1]], FlowLine::If | FlowLine::ElseIf)) {
                    return None;
                }
                stack.push(i);
                if matches!(kind[i], FlowLine::Forever | FlowLine::Loop) {
                    loops.push(i);
                }
            }
            FlowLine::Close => {
                let o = stack.pop()?;
                close_of[o] = i;
                open_of[i] = o;
                if matches!(kind[o], FlowLine::Forever | FlowLine::Loop) {
                    loops.pop();
                }
            }
            FlowLine::Label(l) => {
                if labels.insert(l, i).is_some() {
                    return None;
                }
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        return None;
    }
    // An `if` chain's end: the line after its last branch.
    let chain_end = |mut o: usize| -> usize {
        loop {
            let c = close_of[o];
            if c + 1 < n && matches!(kind[c + 1], FlowLine::ElseIf | FlowLine::Else) {
                o = c + 1;
            } else {
                return c + 1;
            }
        }
    };
    let mut reads = vec![false; n];
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        let next = i + 1;
        let t = texts[i];
        match kind[i] {
            FlowLine::Block | FlowLine::Else | FlowLine::Forever | FlowLine::Label(_) | FlowLine::Other => {
                reads[i] = !matches!(kind[i], FlowLine::Label(_)) && reads_location(t);
                succ[i].push(next);
            }
            FlowLine::If | FlowLine::ElseIf | FlowLine::Loop => {
                reads[i] = reads_location(t);
                succ[i].push(next);
                succ[i].push(close_of[i] + 1);
            }
            FlowLine::Close => {
                let o = open_of[i];
                match kind[o] {
                    FlowLine::Forever | FlowLine::Loop => succ[i].push(o),
                    FlowLine::If | FlowLine::ElseIf | FlowLine::Else => succ[i].push(chain_end(o)),
                    _ => succ[i].push(next),
                }
            }
            FlowLine::Break(c) | FlowLine::Continue(c) => {
                reads[i] = reads_location(t);
                let lp = loop_of[i];
                if lp == usize::MAX {
                    return None;
                }
                succ[i].push(if matches!(kind[i], FlowLine::Break(_)) { close_of[lp] + 1 } else { lp });
                if c {
                    succ[i].push(next);
                }
            }
            FlowLine::Goto(l, c) => {
                reads[i] = reads_location(t);
                match labels.get(l) {
                    Some(&at) => succ[i].push(at),
                    // D-0309: a self tail call's jump, to the body's first
                    // line past a prologue that reads no location
                    // (`tail_entry`).
                    None if l == "self_tail" && tail_entry == Some(false) => succ[i].push(0),
                    // A label outside the body, or a prologue that may
                    // read the location.
                    None => reads[i] = true,
                }
                if c {
                    succ[i].push(next);
                }
            }
            FlowLine::Exit(c) => {
                reads[i] = reads_location(t);
                if c {
                    succ[i].push(next);
                }
            }
        }
    }
    let kills: Vec<bool> = texts.iter().map(|t| t.starts_with("cb_at(")).collect();
    // Live: a path reaches a reader before a `cb_at`.
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, ss) in succ.iter().enumerate() {
        for &s in ss {
            if s < n {
                preds[s].push(i);
            }
        }
    }
    let mut live = reads.clone();
    let mut work: Vec<usize> = (0..n).filter(|&i| live[i]).collect();
    while let Some(s) = work.pop() {
        for &p in &preds[s] {
            if !live[p] && !kills[p] {
                live[p] = true;
                work.push(p);
            }
        }
    }
    let mut dead_node = vec![false; n];
    for i in 0..n {
        dead_node[i] = kills[i] && !succ[i].iter().any(|&s| s < n && live[s]);
    }
    // Then a `cb_at` kept whose location every path to it has already set:
    // the location known on entry to each line (None: not yet reached;
    // Some(None): unknown), a call making it unknown.
    let calls = |t: &str| {
        let b = t.as_bytes();
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
            let mut k = e;
            while k < b.len() && b[k] == b' ' {
                k += 1;
            }
            if b.get(k) == Some(&b'(') && !LOCATION_PURE.contains(&&t[i..e]) {
                return true;
            }
            i = e;
        }
        false
    };
    let mut known: Vec<Option<Option<&str>>> = vec![None; n];
    if n > 0 {
        known[0] = Some(None);
    }
    let mut work: Vec<usize> = if n > 0 { vec![0] } else { Vec::new() };
    let mut queued = vec![false; n];
    if n > 0 {
        queued[0] = true;
    }
    while let Some(i) = work.pop() {
        queued[i] = false;
        let Some(here) = known[i] else { continue };
        let after = if kills[i] && !dead_node[i] {
            Some(texts[i])
        } else if !kills[i] && calls(texts[i]) {
            None
        } else {
            here
        };
        for &s in &succ[i] {
            if s >= n {
                continue;
            }
            let joined = match known[s] {
                None => Some(after),
                Some(k) if k == after => Some(k),
                Some(_) => Some(None),
            };
            if joined != known[s] {
                known[s] = joined;
                if !queued[s] {
                    queued[s] = true;
                    work.push(s);
                }
            }
        }
    }
    let mut dead = vec![false; lines.len()];
    for i in 0..n {
        if dead_node[i] || (kills[i] && known[i] == Some(Some(texts[i]))) {
            dead[node_line[i]] = true;
        }
    }
    Some(dead)
}

// The executing statement's location (`cb_at`) is read only by a fault
// raised without a location of its own (`CB_NOLOC`), and inside the
// runtime (pops, destructors, the checks it faults from): any call of
// anything but the pure helpers. A `cb_at` no path from which reaches
// such a line before the next `cb_at` is dropped (`dead_locations_flow`,
// or in straight-line code only, `dead_locations_straight`, where the
// body's flow is not followed).
// `marks`: a location dropped is kept as a `LINE_MARK` comment for
// `line_directives`, which removes it; it reads as nothing to the one
// pass that runs between (`elide_implied_checks`: no call in it).
// `tail_entry`: for a body a self tail call jumps back into, whether the
// prologue between `self_tail` and the body may read the location.
pub(super) fn drop_dead_locations(out: &str, marks: bool, tail_entry: Option<bool>) -> String {
    let lines: Vec<&str> = out.lines().collect();
    let dead = dead_locations_flow(&lines, tail_entry).unwrap_or_else(|| dead_locations_straight(&lines));
    let mut res = String::with_capacity(out.len());
    for (i, line) in lines.iter().enumerate() {
        if dead[i] {
            if marks {
                if let Some((f, l)) = at_of(line.trim_start()) {
                    writeln!(res, "{}{} {}*/", LINE_MARK, f, l).unwrap();
                }
            }
            continue;
        }
        res.push_str(line);
        res.push('\n');
    }
    res
}

// Whether `w` occurs in `t` as a whole word, and `t` with each such
// occurrence replaced by `by`.
fn replace_word(t: &str, w: &str, by: &str) -> String {
    let b = t.as_bytes();
    let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut out = String::with_capacity(t.len());
    let mut from = 0;
    while let Some(k) = t[from..].find(w) {
        let (s, e) = (from + k, from + k + w.len());
        out.push_str(&t[from..s]);
        if (s == 0 || !is_id(b[s - 1])) && (e == b.len() || !is_id(b[e])) {
            out.push_str(by);
        } else {
            out.push_str(w);
        }
        from = e;
    }
    out.push_str(&t[from..]);
    out
}

// D-0309: each loop `lower_while` marked (`VER_MARK`: the bound of its
// variable, fixed for the loop, and the fixed vector lengths its tagged
// bounds checks compare that variable with) as two: where the bound is at
// most every length, a copy without the tagged checks (each would pass:
// the variable is below the bound wherever the body reads it), its labels
// renamed; otherwise the loop as it was. A nested loop, marked after the
// loop around it, is copied first, its checks no longer tagged when the
// outer one is.
pub(super) fn version_loops(text: &str) -> String {
    if !text.contains(VER_MARK) {
        return text.to_string();
    }
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let mut copies = 0usize;
    while let Some(m) = lines.iter().rposition(|l| l.trim_start().starts_with(VER_MARK)) {
        let spec = lines[m].trim().strip_prefix(VER_MARK).and_then(|r| r.strip_suffix("*/")).unwrap_or("").to_string();
        let parts: Vec<&str> = spec.split(' ').filter(|p| !p.is_empty()).collect();
        let start = m + 1;
        // The loop: its `for` to the matching `}`.
        let mut end = None;
        if parts.len() >= 2 && lines.get(start).map(|l| l.trim()) == Some("for (;;) {") {
            let mut depth = 0i64;
            for (i, l) in lines.iter().enumerate().skip(start) {
                let t = l.trim();
                if t.starts_with('}') {
                    depth -= 1;
                }
                if t.ends_with('{') {
                    depth += 1;
                }
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
        }
        let Some(end) = end else {
            lines.remove(m);
            continue;
        };
        let (bound, lens) = (parts[0], &parts[1..]);
        // A tagged line without its tag, when the tag names one of `lens`.
        let tagged = |l: &str| -> Option<String> {
            let t = l.trim_start();
            let (len, rest) = t.strip_prefix(VER_TAG)?.split_once("*/ ")?;
            lens.contains(&len).then(|| format!("{}{}", &l[..l.len() - t.len()], rest))
        };
        let mut fast: Vec<String> = Vec::new();
        let mut slow: Vec<String> = Vec::new();
        for l in &lines[start..=end] {
            match tagged(l) {
                Some(untagged) => slow.push(untagged),
                None => {
                    fast.push(l.clone());
                    slow.push(l.clone());
                }
            }
        }
        let labels: Vec<String> = fast
            .iter()
            .filter_map(|l| {
                let t = l.trim();
                let n = t.strip_suffix(';').unwrap_or(t).trim_end().strip_suffix(':')?;
                (!n.is_empty() && n.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')).then(|| n.to_string())
            })
            .collect();
        for l in fast.iter_mut() {
            for n in &labels {
                if l.contains(n.as_str()) {
                    *l = replace_word(l, n, &format!("{}_v{}", n, copies));
                }
            }
        }
        copies += 1;
        let ind: String = lines[m].chars().take_while(|c| *c == ' ').collect();
        let test: Vec<String> = lens.iter().map(|l| format!("{} <= {}", bound, l)).collect();
        let mut new = Vec::with_capacity(fast.len() + slow.len() + 3);
        new.push(format!("{}if ({}) {{", ind, test.join(" && ")));
        new.extend(fast);
        new.push(format!("{}}} else {{", ind));
        new.extend(slow);
        new.push(format!("{}}}", ind));
        lines.splice(m..=end, new);
    }
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

// The identifiers of a C expression that name variables: not after `.` or
// `->` (a field), not a type (`struct`, `s_…`, `cb_ref`, `void`).
fn expr_vars(e: &str) -> Vec<String> {
    let b = e.as_bytes();
    let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !is_id(b[i]) || b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < b.len() && is_id(b[j]) {
            j += 1;
        }
        let id = &e[i..j];
        let field = (i >= 1 && b[i - 1] == b'.') || (i >= 2 && &e[i - 2..i] == "->");
        if !field && !matches!(id, "struct" | "void" | "cb_ref") && !id.starts_with("s_") && !out.iter().any(|o| o == id) {
            out.push(id.to_string());
        }
        i = j;
    }
    out
}

// The arguments of the call that begins at `open` (its `(`), split at the
// commas outside parentheses and braces, and where the call ends.
fn split_call(t: &str, open: usize) -> Option<(Vec<&str>, usize)> {
    let b = t.as_bytes();
    let mut depth = 0i64;
    let mut from = open + 1;
    let mut args = Vec::new();
    for (j, &c) in b.iter().enumerate().skip(open) {
        match c {
            b'(' | b'{' => depth += 1,
            b')' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    args.push(t[from..j].trim());
                    return Some((args, j));
                }
            }
            b',' if depth == 1 => {
                args.push(t[from..j].trim());
                from = j + 1;
            }
            _ => {}
        }
    }
    None
}

// A line's calls but the pure helpers (`LOCATION_PURE`) and `cb_at`.
fn calls_other(line: &str) -> bool {
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
        let mut k = e;
        while k < b.len() && b[k] == b' ' {
            k += 1;
        }
        if b.get(k) == Some(&b'(') && !LOCATION_PURE.contains(&&line[i..e]) {
            return true;
        }
        i = e;
    }
    false
}

// D-0309: a loop whose one use of a vector is a confined push
// (`native_vec_push`'s `_nc` form, `f_std__Vec__push_T_nc(V, x, F, L)`),
// the same `V` each time, and which calls nothing else, can fault nowhere
// and leaves only through its end or a `break` (no `return`, no `goto`
// out, no label reached from outside), keeps the vector's header in
// locals: its pointer, length and capacity, the length stored back before
// `grow` and after the loop. Nothing else can read the header meanwhile;
// and a store of an element, which C lets alias anything when the element
// is a byte, no longer makes the compiler load the header again.
pub(super) fn keep_push_headers(text: &str) -> String {
    if !text.contains("f_std__Vec__push_") || !text.contains("_nc(") {
        return text.to_string();
    }
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let mut kept = 0usize;
    let mut s = 0usize;
    while s < lines.len() {
        if lines[s].trim() != "for (;;) {" {
            s += 1;
            continue;
        }
        // The loop: its `for` to the matching `}`.
        let mut end = None;
        let mut depth = 0i64;
        for (i, l) in lines.iter().enumerate().skip(s) {
            let t = l.trim();
            if t.starts_with('}') {
                depth -= 1;
            }
            if t.ends_with('{') {
                depth += 1;
            }
            if depth == 0 {
                end = Some(i);
                break;
            }
        }
        let Some(e) = end else { break };
        // The pushes: (line, name, V, x, F, L).
        let mut push: Option<(String, String)> = None;
        let mut pushes: Vec<(usize, Vec<String>)> = Vec::new();
        let mut ok = true;
        for (i, l) in lines.iter().enumerate().take(e).skip(s + 1) {
            let t = l.trim();
            if t.starts_with("f_std__Vec__push_") {
                let name_end = t.find('(').unwrap_or(0);
                let name = &t[..name_end];
                match split_call(t, name_end) {
                    Some((args, close)) if name.ends_with("_nc") && args.len() == 4 && &t[close + 1..] == ";" => {
                        let key = (name.to_string(), args[0].to_string());
                        if push.as_ref().map_or(false, |p| *p != key) || [args[1], args[2], args[3]].iter().any(|a| calls_other(a)) {
                            ok = false;
                            break;
                        }
                        push = Some(key);
                        pushes.push((i, args.iter().map(|a| a.to_string()).collect()));
                    }
                    _ => {
                        ok = false;
                        break;
                    }
                }
                continue;
            }
            if calls_other(t) || has_word(t, "return") || t.contains(VER_MARK) || t.contains(VER_TAG) {
                ok = false;
                break;
            }
        }
        let Some((name, v)) = push.filter(|_| ok) else {
            s += 1;
            continue;
        };
        // `V`'s variables named nowhere else in the loop; every `goto` to a
        // label of the loop, and every label of the loop reached from it.
        let vars = expr_vars(&v);
        let label_of = |t: &str| -> Option<String> {
            let n = t.strip_suffix(';').unwrap_or(t).trim_end().strip_suffix(':')?;
            (!n.is_empty() && n.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')).then(|| n.to_string())
        };
        let labels: Vec<String> = lines[s..=e].iter().filter_map(|l| label_of(l.trim())).collect();
        let inside = |i: usize| s < i && i < e;
        for (i, l) in lines.iter().enumerate() {
            let t = l.trim();
            let is_push = pushes.iter().any(|(p, _)| *p == i);
            if inside(i) && !is_push && vars.iter().any(|w| has_word(t, w)) {
                ok = false;
            }
            if is_push {
                let args = &pushes.iter().find(|(p, _)| *p == i).unwrap().1;
                if args[1..].iter().any(|a| vars.iter().any(|w| has_word(a, w))) {
                    ok = false;
                }
            }
            if let Some(target) = t.split("goto ").nth(1).map(|r| r.trim_end_matches(';').trim()) {
                if inside(i) != labels.iter().any(|n| n == target) {
                    ok = false;
                }
            }
        }
        if !ok {
            s += 1;
            continue;
        }
        let ty = &name["f_std__Vec__push_".len()..name.len() - "_nc".len()];
        let ind: String = lines[s].chars().take_while(|c| *c == ' ').collect();
        let k = kept;
        kept += 1;
        for (i, a) in &pushes {
            let pi: String = lines[*i].chars().take_while(|c| *c == ' ').collect();
            let at = if a[2] == "CB_NOLOC" { String::new() } else { format!("cb_at({}, {}); ", a[2], a[3]) };
            lines[*i] = format!(
                "{}{{ if (__builtin_expect(kh{k}_n == kh{k}_c, 0)) {{ kh{k}_v->len = kh{k}_n; {}f_std__Vec__grow_{}({}); kh{k}_p = kh{k}_v->ptr; kh{k}_c = kh{k}_v->cap; }} kh{k}_p[kh{k}_n] = {}; kh{k}_n += 1; }}",
                pi, at, ty, v, a[1], k = k
            );
        }
        lines.insert(e + 1, format!("{}kh{k}_v->len = kh{k}_n; }}", ind, k = k));
        lines.insert(
            s,
            format!(
                "{}{{ struct s_std__Vec_{} *kh{k}_v = (struct s_std__Vec_{} *)({}).p; __typeof__(kh{k}_v->ptr) kh{k}_p = kh{k}_v->ptr; uint64_t kh{k}_n = kh{k}_v->len; uint64_t kh{k}_c = kh{k}_v->cap;",
                ind, ty, ty, v, k = k
            ),
        );
        s = e + 3;
    }
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
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
        // D-0297: a literal format written in place.
        "cb_fmt_space", "cb_fmt_u64", "cb_fmt_i64", "cb_fmt_hex", "cb_fmt_pad",
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
                let ind = &line[..line.len() - line.trim_start().len()];
                if used.contains(&id) {
                    res.push_str(ind);
                    res.push_str(call);
                    res.push('\n');
                }
                // D-0293: a decided body's borrows from vouched tokens are
                // light inline (`cbrt.h`'s `cb_lf`), frame or no frame.
                if k == 'G' {
                    res.push_str(ind);
                    res.push_str("enum { cb_lf = 1 };\n");
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
