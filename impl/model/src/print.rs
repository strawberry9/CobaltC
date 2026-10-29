// Writes a fragment program as CobaltC, in the house style `cobfmt` keeps
// (four-space indentation, Allman braces), one statement per line, so that
// a diagnostic's line names one statement.

use crate::ast::*;

pub fn expr(e: &Expr) -> String {
    match e {
        Expr::Int(k) => k.to_string(),
        Expr::Var(n) => n.clone(),
        Expr::Deref(a) => format!("*{}", atom(a)),
        Expr::Field(a) => format!("{}.r", atom(a)),
        Expr::Index(v, k) => format!("{}[{}]", v, k),
        Expr::Borrow(m, a) => format!("&{}{}", if *m == Mode::Excl { "mut " } else { "" }, atom(a)),
        Expr::Add(a, b) => format!("{} + {}", expr(a), expr(b)),
        Expr::Gt(a, b) => format!("{} > {}", expr(a), expr(b)),
        Expr::Call(f, xs) => format!("{}({})", f, xs.iter().map(expr).collect::<Vec<_>>().join(", ")),
        Expr::Spawn(f, xs) => {
            let mut s = format!("spawn({}", f);
            for x in xs {
                s.push_str(", ");
                s.push_str(&expr(x));
            }
            s.push(')');
            s
        }
        Expr::Join(t) => format!("join({})", t),
        Expr::HoldLit(a) => format!("Hold {{ .r = {} }}", expr(a)),
        Expr::JobLit(a) => format!("Job {{ .h = {} }}", expr(a)),
        Expr::ArrLit(xs) => format!("[{}]", xs.iter().map(expr).collect::<Vec<_>>().join(", ")),
        Expr::Slice(m, v, lo, hi) => format!("&{}{}[{} .. {}]", if *m == Mode::Excl { "mut " } else { "" }, v, lo, hi),
        Expr::Closure(c) => {
            let caps = c.caps.join(", ");
            let ret = if c.ret == Ty::Unit { String::new() } else { format!(" : {}", c.ret.text()) };
            format!("{}[{}](){} {}", if c.mv { "move " } else { "" }, caps, ret, inline_tail(&c.body))
        }
        Expr::CallVal(a) => match &**a {
            Expr::Var(n) => format!("{}()", n),
            other => format!("({})()", expr(other)),
        },
        Expr::SpawnClo(a) => format!("spawn({})", expr(a)),
    }
}

// A closure's body on one line, as `cobfmt` keeps it: `{ s1; s2; e }`.
fn inline_tail(b: &Block) -> String {
    let mut parts = Vec::new();
    for s in &b.stmts {
        let mut o = String::new();
        stmt(s, 0, &mut o);
        parts.push(o.trim().to_string());
    }
    if let Some(t) = &b.tail {
        parts.push(expr(t));
    }
    if parts.is_empty() {
        "{}".into()
    } else {
        format!("{{ {} }}", parts.join(" "))
    }
}

// An operand of a prefix or postfix operator: parenthesized when it is
// itself a prefix form under a postfix one (`(*s).r` is written `s.r`).
fn atom(e: &Expr) -> String {
    match e {
        Expr::Add(..) | Expr::Gt(..) => format!("({})", expr(e)),
        _ => expr(e),
    }
}

fn ind(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push_str("    ");
    }
}

pub fn block(b: &Block, depth: usize, out: &mut String) {
    ind(out, depth);
    out.push_str("{\n");
    for s in &b.stmts {
        stmt(s, depth + 1, out);
    }
    if let Some(t) = &b.tail {
        ind(out, depth + 1);
        out.push_str(&expr(t));
        out.push('\n');
    }
    ind(out, depth);
    out.push_str("}\n");
}

pub fn stmt(s: &Stmt, depth: usize, out: &mut String) {
    match &s.kind {
        StmtKind::Let { name, ty, init } => {
            ind(out, depth);
            let t = ty.as_ref().map_or("auto".to_string(), |t| t.text());
            out.push_str(&format!("{} {} = {};\n", t, name, expr(init)));
        }
        StmtKind::Assign { place, value } => {
            ind(out, depth);
            out.push_str(&format!("{} = {};\n", expr(place), expr(value)));
        }
        StmtKind::Expr(e) => {
            ind(out, depth);
            out.push_str(&format!("{};\n", expr(e)));
        }
        StmtKind::Print(e) => {
            ind(out, depth);
            out.push_str(&format!("printf(\"%v\\n\", {});\n", expr(e)));
        }
        StmtKind::Block(b) => block(b, depth, out),
        StmtKind::Discard(e) => {
            ind(out, depth);
            out.push_str(&format!("_ = {};\n", expr(e)));
        }
        StmtKind::Match { scrut, binder, some, none } => {
            ind(out, depth);
            out.push_str(&format!("match ({})\n", expr(scrut)));
            ind(out, depth);
            out.push_str("{\n");
            ind(out, depth + 1);
            out.push_str(&format!("Some({}) : {},\n", binder, inline(some)));
            ind(out, depth + 1);
            out.push_str(&format!("None{} : {},\n", " ".repeat(binder.len() + 2), inline(none)));
            ind(out, depth);
            out.push_str("}\n");
        }
        StmtKind::Break => {
            ind(out, depth);
            out.push_str("break;\n");
        }
        StmtKind::Continue => {
            ind(out, depth);
            out.push_str("continue;\n");
        }
        StmtKind::While { cond, body } => {
            ind(out, depth);
            out.push_str(&format!("while ({})\n", expr(cond)));
            block(body, depth, out);
        }
        StmtKind::If { cond, then, els } => {
            ind(out, depth);
            out.push_str(&format!("if ({})\n", expr(cond)));
            block(then, depth, out);
            if let Some(b) = els {
                ind(out, depth);
                out.push_str("else\n");
                block(b, depth, out);
            }
        }
    }
}

// An arm's block on one line: `{ s1; s2; }`, `{}` when empty.
fn inline(b: &Block) -> String {
    if b.stmts.is_empty() {
        return "{}".into();
    }
    let mut parts = Vec::new();
    for s in &b.stmts {
        let mut o = String::new();
        stmt(s, 0, &mut o);
        parts.push(o.trim().to_string());
    }
    format!("{{ {} }}", parts.join(" "))
}

pub fn func(f: &Func, out: &mut String) {
    let ps: Vec<String> = f.params.iter().map(|(t, n)| format!("{} {}", t.text(), n)).collect();
    out.push_str(&format!("fn {}({})", f.name, ps.join(", ")));
    if f.ret != Ty::Unit {
        out.push_str(&format!(" : {}", f.ret.text()));
    }
    out.push('\n');
    block(&f.body, 0, out);
}

pub fn program(p: &Program, header: &str) -> String {
    let mut out = String::new();
    out.push_str(header);
    out.push_str("import std;\n");
    if p.hold {
        out.push_str("\nstruct Hold\n{\n    ref<i32, exclusive> r;\n}\n");
    }
    if p.job {
        out.push_str("\nstruct Job\n{\n    handle<void> h;\n}\n");
    }
    for (n, k) in &p.statics {
        out.push_str(&format!("\nstatic Atomic<i32> {} = {};\n", n, k));
    }
    for f in &p.funcs {
        out.push('\n');
        func(f, &mut out);
    }
    out
}

// The helpers a generated program may call, as CobaltC. A program includes
// the ones it calls; the model reads them back with the parser, so their
// bodies run through the same rules as `main`'s.
pub const HELPERS: &[(&str, &str)] = &[
    ("bump", "fn bump(ref<i32, exclusive> p)\n{\n    *p = *p + 1;\n}\n"),
    ("peek", "fn peek(ref<i32, shared> p) : i32\n{\n    *p\n}\n"),
    ("two", "fn two(ref<i32, exclusive> a, ref<i32, exclusive> b)\n{\n    *a = *a + 1;\n    *b = *b + 10;\n}\n"),
    ("pass", "fn pass(ref<i32, exclusive> p) : ref<i32, exclusive>\n{\n    p\n}\n"),
    ("bump_hold", "fn bump_hold(Hold h)\n{\n    *h.r = *h.r + 1;\n}\n"),
    ("read_hold", "fn read_hold(ref<Hold, shared> h) : i32\n{\n    *h.r\n}\n"),
    ("add_locked", "fn add_locked(ref<mutex<i32>, shared> m)\n{\n    auto g = lock(m);\n    *g = *g + 1;\n}\n"),
    ("read_locked", "fn read_locked(ref<mutex<i32>, shared> m) : i32\n{\n    auto g = lock(m);\n    *g\n}\n"),
    ("lock_add_hold", "fn lock_add_hold(ref<mutex<i32>, shared> m, ref<Hold, shared> h)\n{\n    auto g = lock(m);\n    *g = *g + *h.r;\n}\n"),
    ("bump_first", "fn bump_first(Vec<ref<i32, exclusive>> w)\n{\n    *w[0] = 9;\n}\n"),
    ("swap_first", "fn swap_first(Vec<ref<i32, exclusive>> w)\n{\n    i32 z = 100;\n    swap(*Vec::index_exclusive(&mut w, 0), &mut z);\n}\n"),
    ("take_one", "fn take_one(ref<Channel<ref<i32, exclusive>>, shared> ch)\n{\n    match (Channel::recv(ch))\n    {\n        Some(r) : { *r = *r + 100; },\n        None    : {},\n    }\n}\n"),
    ("call_val", "fn call_val(fn() : void f)\n{\n    f();\n}\n"),
    ("call_shared", "fn call_shared(ref<fn() : void, shared> f)\n{\n    (*f)();\n}\n"),
    ("bump_slice", "fn bump_slice(slice<i32, exclusive> s)\n{\n    s[0] = s[0] + 1;\n}\n"),
    ("first", "fn first(slice<i32, shared> s) : i32\n{\n    s[0]\n}\n"),
    ("take_slice", "fn take_slice(ref<Channel<slice<i32, exclusive>>, shared> ch)\n{\n    match (Channel::recv(ch))\n    {\n        Some(s) : { s[0] = s[0] + 100; },\n        None    : {},\n    }\n}\n"),
    ("call_twice", "fn call_twice(fn() : void f)\n{\n    auto g = f;\n    g();\n    f();\n}\n"),
];

pub fn helper_needs_hold(name: &str) -> bool {
    name.contains("hold")
}
