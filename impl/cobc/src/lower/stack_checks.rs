// D-0309 (1b): the stack check (`cb_stack_check`, D-0077) kept only where
// the depth of calls can grow without bound, or where a chain of
// functions without one could use more than `BUDGET` bytes below the last
// check.
//
// A function keeps its check when it is on a cycle of direct calls (its
// own recursion, or a mutual one), when its address is taken anywhere in
// the program (a `fn` value, a closure's body, a destructor in a type
// descriptor, a thread's body: anything the program or the runtime may
// call through a pointer, so any cycle through one passes a check), or
// when nothing in the program calls it (an entry: `main`, a library's
// stub). Every other function is reached only by direct calls from a
// bounded chain of callers, and below the nearest of those that checks,
// the chain it heads is at most `BUDGET` bytes deep, which the floor's
// margin (`find_stack_floor`: at least 256 KiB) covers, as it covered a
// leaf (`calls_nothing`) before.
//
// The rule is order-independent, so both compilers reach the same
// answer whatever order they visit the functions in: a function's *need*
// is its frame plus the largest need among the callees that carry no
// check; a function without a forced check keeps its check exactly when
// some caller's frame plus its need exceeds `BUDGET`. Each decision reads
// only its callees' (callees first, any reverse topological order).
//
// A frame is estimated from the function's C, over rather than under:
// 16 bytes for every `;`, a `struct`'s size (`_Static_assert`) where one
// is named (4 KiB when unknown), `N` times that or 16 for `[N]`, and the
// runtime's own value types by their size. Static locals take no stack.

use std::collections::{HashMap, HashSet};

use super::passes::{dead_locations_flow, dead_locations_straight, reads_location_in};

pub(super) const BUDGET: u64 = 64 << 10;
const UNKNOWN_STRUCT: u64 = 4 << 10;
const CHECK_LINE: &str = "cb_stack_check();";

// The runtime's value types a body may declare, by their C names, at
// their size or more (`cbrt.h`).
const RUNTIME_SIZES: &[(&str, u64)] = &[("cb_ref", 16), ("cb_str", 16), ("cb_i128", 16), ("cb_u128", 16), ("cb_proj", 16), ("cb_fnbox", 32), ("cb_fmt_arg", 64), ("jmp_buf", 512)];

struct Def {
    unit: usize,
    name: String,
    // Lines `{` and `}` of the body, in the unit's lines.
    open: usize,
    close: usize,
    check: Option<usize>,
}

fn is_ident_byte(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphanumeric()
}

// The function a definition line names: the word before its `(`, its
// attributes left out.
fn def_name(l: &str) -> Option<&str> {
    let mut rest = l;
    while let Some(i) = rest.find("__attribute__((") {
        let after = &rest[i..];
        let end = after.find("))").map_or(after.len(), |e| e + 2);
        rest = &rest[i + end..];
    }
    let open = rest.find('(')?;
    let head = rest[..open].trim_end();
    let start = head.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
    let name = &head[start..];
    (!name.is_empty()).then_some(name)
}

// The definitions in a unit's lines: a line at the left margin ending in
// `)` whose next line (past `#line` directives) is `{`, to the next `}` at
// the left margin.
fn defs_in(unit: usize, lines: &[&str], out: &mut Vec<Def>) {
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        let starts = !l.is_empty() && !l.starts_with([' ', '{', '}', '#']);
        if starts && l.ends_with(')') {
            let mut j = i + 1;
            while j < lines.len() && lines[j].starts_with("#line") {
                j += 1;
            }
            if j < lines.len() && lines[j] == "{" {
                // Only a program function (`f_…`) takes part: a closure's
                // body (`cl_…`) is always called through its box, and
                // anything else the tail writes keeps what it has.
                if let Some(name) = def_name(l).filter(|n| n.starts_with("f_")) {
                    let mut k = j + 1;
                    while k < lines.len() && lines[k] != "}" {
                        k += 1;
                    }
                    let check = (j + 1..k).find(|&m| lines[m].trim() == CHECK_LINE);
                    out.push(Def { unit, name: name.to_string(), open: j, close: k, check });
                    i = k + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
}

// Each identifier of `text` outside string and character literals, with
// whether the next byte past spaces is `(`.
fn each_ident(text: &str, f: &mut dyn FnMut(&str, bool)) {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' {
            i += 1;
            while i < b.len() && b[i] != c {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if c == b'_' || c.is_ascii_alphabetic() {
            let start = i;
            while i < b.len() && is_ident_byte(b[i]) {
                i += 1;
            }
            let mut k = i;
            while k < b.len() && b[k] == b' ' {
                k += 1;
            }
            f(&text[start..i], b.get(k) == Some(&b'('));
            continue;
        }
        if c.is_ascii_digit() {
            while i < b.len() && is_ident_byte(b[i]) {
                i += 1;
            }
            continue;
        }
        i += 1;
    }
}

// The bytes of the stack a body may use (see the head of this file).
fn frame_of(lines: &[&str], sizes: &HashMap<String, u64>) -> u64 {
    let mut total: u64 = 256;
    for l in lines {
        let t = l.trim_start();
        if t.starts_with('#') || t.starts_with("static ") {
            continue;
        }
        let b = t.as_bytes();
        total += 16 * b.iter().filter(|&&c| c == b';').count() as u64;
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            if c == b'"' {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                continue;
            }
            if c == b'_' || c.is_ascii_alphabetic() {
                let start = i;
                while i < b.len() && is_ident_byte(b[i]) {
                    i += 1;
                }
                let word = &t[start..i];
                if word == "struct" {
                    // `struct NAME`, then, unless a pointer, its size, and
                    // `NAME x[N]`'s N times it.
                    let mut k = i;
                    while k < b.len() && b[k] == b' ' {
                        k += 1;
                    }
                    let ns = k;
                    while k < b.len() && is_ident_byte(b[k]) {
                        k += 1;
                    }
                    let name = &t[ns..k];
                    let mut m = k;
                    while m < b.len() && b[m] == b' ' {
                        m += 1;
                    }
                    if name.is_empty() || b.get(m) == Some(&b'*') {
                        i = k;
                        continue;
                    }
                    let size = sizes.get(name).copied().unwrap_or(UNKNOWN_STRUCT);
                    // A declarator `x[N]` after it.
                    let ds = m;
                    while m < b.len() && is_ident_byte(b[m]) {
                        m += 1;
                    }
                    let mut count = 1;
                    if m > ds && b.get(m) == Some(&b'[') {
                        let mut e = m + 1;
                        while e < b.len() && b[e].is_ascii_digit() {
                            e += 1;
                        }
                        if e > m + 1 && b.get(e) == Some(&b']') {
                            count = t[m + 1..e].parse::<u64>().unwrap_or(u64::MAX / (64 << 20));
                            m = e + 1;
                        }
                    }
                    total = total.saturating_add(size.saturating_mul(count));
                    i = m.max(k);
                    continue;
                }
                if let Some((_, s)) = RUNTIME_SIZES.iter().find(|(n, _)| *n == word) {
                    total += s;
                }
                continue;
            }
            if c == b'[' {
                let mut e = i + 1;
                while e < b.len() && b[e].is_ascii_digit() {
                    e += 1;
                }
                if e > i + 1 && b.get(e) == Some(&b']') {
                    let n = t[i + 1..e].parse::<u64>().unwrap_or(u64::MAX / 64);
                    total = total.saturating_add(n.saturating_mul(16));
                } else if e == i + 1 && b.get(e) == Some(&b']') {
                    // `x[] = { … }`: its elements, by the commas after it.
                    let n = t[e..].bytes().filter(|&c| c == b',').count() as u64 + 1;
                    total = total.saturating_add(n * 16);
                }
                i = e;
                continue;
            }
            i += 1;
        }
    }
    total
}

// The sizes `_Static_assert(sizeof(struct NAME) == N && …` states.
fn struct_sizes(texts: &[&str]) -> HashMap<String, u64> {
    let mut out = HashMap::new();
    out.insert("cb_note".to_string(), 32);
    for t in texts {
        for l in t.lines() {
            let Some(rest) = l.strip_prefix("_Static_assert(sizeof(struct ") else { continue };
            let Some(close) = rest.find(')') else { continue };
            let name = &rest[..close];
            let Some(eq) = rest[close..].strip_prefix(") == ") else { continue };
            let digits: String = eq.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<u64>() {
                out.insert(name.to_string(), n);
            }
        }
    }
    out
}

// The functions on a cycle of direct calls (a strongly connected component
// of more than one, or one calling itself): Tarjan's, iterative.
fn cyclic(n: usize, edges: &[Vec<usize>]) -> (Vec<bool>, Vec<usize>) {
    let mut index = vec![usize::MAX; n];
    let mut low = vec![0; n];
    let mut on = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut out = vec![false; n];
    // Reverse topological order of the components: callees first.
    let mut order: Vec<usize> = Vec::new();
    let mut next = 0;
    for root in 0..n {
        if index[root] != usize::MAX {
            continue;
        }
        let mut work: Vec<(usize, usize)> = vec![(root, 0)];
        index[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on[root] = true;
        while let Some(&(v, k)) = work.last() {
            if k < edges[v].len() {
                work.last_mut().unwrap().1 += 1;
                let w = edges[v][k];
                if index[w] == usize::MAX {
                    index[w] = next;
                    low[w] = next;
                    next += 1;
                    stack.push(w);
                    on[w] = true;
                    work.push((w, 0));
                } else if on[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            work.pop();
            if let Some(&(p, _)) = work.last() {
                low[p] = low[p].min(low[v]);
            }
            if low[v] == index[v] {
                let mut comp = Vec::new();
                loop {
                    let w = stack.pop().unwrap();
                    on[w] = false;
                    comp.push(w);
                    if w == v {
                        break;
                    }
                }
                let cyc = comp.len() > 1 || edges[v].contains(&v);
                for &w in &comp {
                    out[w] = cyc;
                }
                comp.sort();
                order.extend(comp);
            }
        }
    }
    (out, order)
}

// Takes `cb_stack_check();` out of every function of `units` the rule above
// lets go without one. `others`: the rest of the program's C (thunks,
// descriptors, the tail), read for addresses taken; `types`: where the
// structs' sizes are stated. `fresh`: the program's `main`, which the tail
// calls through a pointer as the first frame of a thread whose stack is
// all unused (`cb_run_main`): its mentions in `others` take no address,
// and having no caller forces no check; a call to it, or an address taken,
// in the program's own code still does.
pub(super) fn prune_stack_checks(units: &mut [String], others: &[&str], types: &[&str], fresh: Option<&str>) {
    let sizes = struct_sizes(types);
    let mut defs: Vec<Def> = Vec::new();
    {
        let unit_lines: Vec<Vec<&str>> = units.iter().map(|u| u.split('\n').collect()).collect();
        for (k, lines) in unit_lines.iter().enumerate() {
            defs_in(k, lines, &mut defs);
        }
    }
    let mut id: HashMap<String, usize> = HashMap::new();
    for (k, d) in defs.iter().enumerate() {
        // A name defined twice (never, in one program): neither is pruned.
        if id.insert(d.name.clone(), k).is_some() {
            return;
        }
    }
    let n = defs.len();
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut callers: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut taken = vec![false; n];
    let mut frame = vec![0u64; n];
    let mut alloca = vec![false; n];
    {
        let unit_lines: Vec<Vec<&str>> = units.iter().map(|u| u.split('\n').collect()).collect();
        // Inside each body: its calls, its addresses taken, its frame.
        for (k, d) in defs.iter().enumerate() {
            let body = &unit_lines[d.unit][d.open + 1..d.close];
            let mut seen: HashSet<usize> = HashSet::new();
            for l in body {
                each_ident(l, &mut |w, call| {
                    if w == "alloca" || w == "__builtin_alloca" {
                        alloca[k] = true;
                    }
                    if let Some(&t) = id.get(w) {
                        if call {
                            if seen.insert(t) {
                                edges[k].push(t);
                            }
                        } else {
                            taken[t] = true;
                        }
                    }
                });
            }
            frame[k] = frame_of(body, &sizes);
        }
        // Outside every body: a name not called is an address taken (a
        // prototype or definition line names it before `(`).
        for (u, lines) in unit_lines.iter().enumerate() {
            let mut inside = vec![false; lines.len()];
            for d in defs.iter().filter(|d| d.unit == u) {
                for flag in inside.iter_mut().take(d.close + 1).skip(d.open) {
                    *flag = true;
                }
            }
            for (i, l) in lines.iter().enumerate() {
                if inside[i] {
                    continue;
                }
                each_ident(l, &mut |w, call| {
                    if let Some(&t) = id.get(w) {
                        if !call {
                            taken[t] = true;
                        }
                    }
                });
            }
        }
    }
    for t in others {
        each_ident(t, &mut |w, call| {
            if let Some(&k) = id.get(w) {
                if !call && Some(w) != fresh {
                    taken[k] = true;
                }
            }
        });
    }
    for k in 0..n {
        for &t in &edges[k] {
            callers[t].push(k);
        }
    }
    let (cyc, order) = cyclic(n, &edges);
    // Callees first: each function's need, and whether it keeps its check.
    let mut checked = vec![false; n];
    let mut need = vec![0u64; n];
    for &k in &order {
        let below = edges[k].iter().filter(|&&t| !checked[t] && t != k).map(|&t| need[t]).max().unwrap_or(0);
        need[k] = frame[k].saturating_add(below);
        let entry = callers[k].is_empty() && Some(defs[k].name.as_str()) != fresh;
        let forced = cyc[k] || taken[k] || entry || alloca[k];
        checked[k] = forced || callers[k].iter().any(|&p| frame[p].saturating_add(need[k]) > BUDGET);
    }
    // A function without a check of its own (`calls_nothing`) is left as it
    // is; one with a check loses it where the rule allows. D-0315 (step 6):
    // and a location stored only for a quiet callee goes.
    let mut drop: Vec<Vec<usize>> = quiet_call_locations(units, &defs, &edges, &checked);
    for (k, d) in defs.iter().enumerate() {
        if let (false, Some(line)) = (checked[k], d.check) {
            drop[d.unit].push(line);
        }
    }
    for (u, lines) in drop.into_iter().enumerate() {
        if lines.is_empty() {
            continue;
        }
        let gone: HashSet<usize> = lines.into_iter().collect();
        let text: String = units[u].split('\n').enumerate().filter(|(i, _)| !gone.contains(i)).map(|(_, l)| l).collect::<Vec<_>>().join("\n");
        units[u] = text;
    }
}

// CHG-0430: in a decided program, a small function of its own (its body at
// most `INLINE_LINES` lines past its `#line` directives, with no loop, no
// self tail call and no stack check left by `prune_stack_checks`, so on no
// cycle of calls, its address never taken and no entry) is defined
// `CB_SMALL`, and its prototype with it: `cbrt.h` makes that `static inline
// __attribute__((always_inline))` under GCC, which otherwise leaves a call per
// use of `pop`, `Option::ok_or` and their kind, and `static` under Clang,
// which inlines them itself. Only a function `localize` made `static`,
// with no attribute of its own, in a unit that is not pure `std`, takes part.
pub(super) const INLINE_LINES: usize = 32;
pub(super) const ALWAYS_INLINE: &str = "CB_SMALL ";

pub(super) fn inline_small(units: &mut [String], pure: &[bool], protos: &mut String) {
    let mut names: HashSet<String> = HashSet::new();
    for (k, u) in units.iter_mut().enumerate() {
        // CHG-0433: not a pure `std` unit's, `static` now too (`localize`):
        // GCC inlines those better by its own measure (every small one
        // forced inline into Huffman's `compress` made it 12% slower).
        if pure[k] {
            continue;
        }
        let lines: Vec<&str> = u.split('\n').collect();
        let mut defs: Vec<Def> = Vec::new();
        defs_in(0, &lines, &mut defs);
        let mut heads: Vec<usize> = Vec::new();
        for d in &defs {
            // The definition's own line: the last before `{` that is no `#line`.
            let Some(head) = (0..d.open).rev().find(|&i| !lines[i].starts_with("#line")) else { continue };
            let l = lines[head];
            if !l.starts_with("static ") || l.contains("__attribute__") || l.starts_with("static inline") {
                continue;
            }
            let body = &lines[d.open + 1..d.close];
            if body.iter().filter(|b| !b.starts_with("#line")).count() > INLINE_LINES {
                continue;
            }
            let bad = |b: &&str| {
                let t = b.trim_start();
                b.trim() == CHECK_LINE || t.starts_with("for (") || t.starts_with("while (") || t.starts_with("do {") || b.contains("goto self_tail;") || b.contains("setjmp") || b.contains("alloca") || b.contains("va_start")
            };
            if body.iter().any(bad) {
                continue;
            }
            heads.push(head);
            names.insert(d.name.clone());
        }
        if heads.is_empty() {
            continue;
        }
        let set: HashSet<usize> = heads.into_iter().collect();
        let text: String = lines.iter().enumerate().map(|(i, l)| if set.contains(&i) { format!("{}{}", ALWAYS_INLINE, &l["static ".len()..]) } else { l.to_string() }).collect::<Vec<_>>().join("\n");
        *u = text;
    }
    if names.is_empty() {
        return;
    }
    let mut out = String::with_capacity(protos.len() + 40 * names.len());
    for l in protos.split_inclusive('\n') {
        match l.strip_prefix("static ") {
            Some(rest) if !rest.starts_with("inline") && !rest.contains("__attribute__") && def_name(rest).map_or(false, |n| names.contains(n)) => {
                out.push_str(ALWAYS_INLINE);
                out.push_str(rest);
            }
            _ => out.push_str(l),
        }
    }
    *protos = out;
}

// D-0309: a frame's notes (D-0269, `cb_nt`) are registered with the runtime
// only once a thread has been spawned (`cb_threaded`), and only `cb_spawn`
// spawns one. Where no part of the program calls it, nothing ever reads the
// notes, and their lines go. Their stores took each objectless local's
// address, and an address taken anywhere in a function keeps the C compiler
// from holding the local's fields in registers across a store through any
// pointer: a `Vec`'s length across the stores to its elements (`sieve`
// −37% under gcc, with the copy `cb_drop_plain_buf` makes).
pub(super) fn drop_unused_notes(units: &mut [String], others: &[&str]) {
    let spawns = |t: &str| t.lines().any(|l| l.contains("cb_spawn(") && !l.trim_start().starts_with("uint64_t cb_spawn("));
    if units.iter().any(|u| spawns(u)) || others.iter().any(|t| spawns(t)) {
        return;
    }
    for u in units.iter_mut() {
        // D-0315 (step 4): a table registered on every call stays.
        if !u.contains("cb_nt") || u.contains("    uint8_t cb_nt_on = 1;") {
            continue;
        }
        let kept: Vec<String> = u
            .split('\n')
            .filter(|line| {
                let t = line.trim_start();
                !(t.starts_with("struct cb_note cb_nt[")
                    || t == "uint8_t cb_nt_on = cb_threaded;"
                    || t.starts_with("if (cb_nt_on) cb_notes_")
                    || (t.starts_with("cb_nt[") && t.contains("] = (struct cb_note){")))
            })
            .map(|line| if line.contains("cb_nt[") { without_live(line) } else { line.to_string() })
            .collect();
        *u = kept.join("\n");
    }
}

// `line` without its ` cb_nt[k].live = 0;` (the end of a drop).
fn without_live(line: &str) -> String {
    let mut s = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(i) = rest.find("cb_nt[") {
        let tail = &rest[i + 6..];
        let digits = tail.bytes().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && tail[digits..].starts_with("].live = 0;") {
            let pre = &rest[..i];
            s.push_str(pre.strip_suffix(' ').unwrap_or(pre));
            rest = &tail[digits + 11..];
        } else {
            s.push_str(&rest[..i + 6]);
            rest = tail;
        }
    }
    s.push_str(rest);
    s
}

// D-0315 (step 6): the executing statement's location (`cb_at`) stored
// before a call only that call could read is dropped where the callee is
// quiet: no line of its body reads the location (`reads_location`, a call
// of a quiet function aside: a fault without a location, the runtime's
// calls, a stack check it keeps) and none stores it, so whatever reads the
// location later still meets a store the analysis keeps. Quiet is decided
// over the program's functions (`f_…`) to a fixed point, by `edges`; a body
// where a store is followed by a call of a quiet function is analysed again
// with those calls read as nothing (`dead_locations_flow`, else
// `dead_locations_straight`). The lines to drop, by unit.
fn quiet_call_locations(units: &[String], defs: &[Def], edges: &[Vec<usize>], checked: &[bool]) -> Vec<Vec<usize>> {
    let n = defs.len();
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); units.len()];
    let unit_lines: Vec<Vec<&str>> = units.iter().map(|u| u.split('\n').collect()).collect();
    let names: HashSet<String> = defs.iter().map(|d| d.name.clone()).collect();
    let mut loud: Vec<bool> = (0..n)
        .map(|k| {
            let d = &defs[k];
            if checked[k] && d.check.is_some() {
                return true;
            }
            (d.open + 1..d.close).any(|m| Some(m) != d.check && (unit_lines[d.unit][m].contains("cb_at(") || reads_location_in(unit_lines[d.unit][m].trim(), Some(&names))))
        })
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for k in 0..n {
            if !loud[k] && edges[k].iter().any(|&c| loud[c]) {
                loud[k] = true;
                changed = true;
            }
        }
    }
    let quiet: HashSet<String> = (0..n).filter(|&k| !loud[k]).map(|k| defs[k].name.clone()).collect();
    if quiet.is_empty() {
        return out;
    }
    for (k, d) in defs.iter().enumerate() {
        if !edges[k].iter().any(|&c| !loud[c]) {
            continue;
        }
        let lines = &unit_lines[d.unit];
        let mut hit = false;
        for m in d.open + 1..d.close {
            if !lines[m].trim_start().starts_with("cb_at(") {
                continue;
            }
            let mut j = m + 1;
            while j < d.close && lines[j].starts_with("#line") {
                j += 1;
            }
            if j < d.close {
                each_ident(lines[j], &mut |w, call| {
                    if call && quiet.contains(w) {
                        hit = true;
                    }
                });
            }
            if hit {
                break;
            }
        }
        if !hit {
            continue;
        }
        let body = &lines[d.open + 1..d.close];
        let dead = dead_locations_flow(body, None, Some(&quiet)).unwrap_or_else(|| dead_locations_straight(body, Some(&quiet)));
        for (i, &x) in dead.iter().enumerate() {
            if x {
                out[d.unit].push(d.open + 1 + i);
            }
        }
    }
    out
}
