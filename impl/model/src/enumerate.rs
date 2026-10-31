// The fragment's programs, enumerated: every sequence of up to N
// statements drawn from a fixed alphabet of statement templates over a
// fixed name space, each well-scoped (a name is used only after the
// statement that declares it). `x` and `y` are declared first and printed
// last, so a lost or misplaced write shows in the output.
//
//   cobmodel enum --size N --out DIR [--max K] [--random M --len L --seed S]
//
// Writes DIR/pNNNNNN.cb, each with its model outcome set in a header
// comment (`// MODEL: …`), and DIR/index.tsv. Every program is run through
// the model; when there are more than K, the K written are every program
// with more than one outcome or a dynamic fault, then an even sample of
// the rest. `--random M --len L` adds M random programs of L statements.

use crate::ast::*;
use crate::explore;
use crate::parse;
use crate::print;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::process::ExitCode;

#[derive(Clone, Default)]
struct Env {
    names: BTreeMap<String, Ty>,
}

impl Env {
    fn has(&self, n: &str) -> bool {
        self.names.contains_key(n)
    }
    fn excl_refs(&self) -> Vec<String> {
        self.names.iter().filter(|(_, t)| **t == Ty::refi(Mode::Excl)).map(|(n, _)| n.clone()).collect()
    }
    fn any_refs(&self) -> Vec<String> {
        self.names.iter().filter(|(_, t)| matches!(t, Ty::Ref(i, _) if **i == Ty::I32)).map(|(n, _)| n.clone()).collect()
    }
    // Closure bindings: `cl…` without `move`, `mk…` with; (name, result type).
    fn closures(&self, prefix: &str) -> Vec<(String, Ty)> {
        self.names.iter().filter_map(|(n, t)| match t {
            Ty::Fn(r) if n.starts_with(prefix) => Some((n.clone(), (**r).clone())),
            _ => None,
        }).collect()
    }
    fn slices(&self) -> Vec<(String, Mode)> {
        self.names.iter().filter_map(|(n, t)| match t {
            Ty::Slice(m) => Some((n.clone(), *m)),
            _ => None,
        }).collect()
    }
    fn void_handles(&self) -> Vec<String> {
        self.names.iter().filter(|(_, t)| **t == Ty::Handle(Box::new(Ty::Unit))).map(|(n, _)| n.clone()).collect()
    }
}

fn var(n: &str) -> Expr {
    Expr::Var(n.to_string())
}
fn int(k: i64) -> Expr {
    Expr::Int(k)
}
fn de(e: Expr) -> Expr {
    Expr::Deref(Box::new(e))
}
fn bor(m: Mode, e: Expr) -> Expr {
    Expr::Borrow(m, Box::new(e))
}
fn add1(e: Expr) -> Expr {
    Expr::Add(Box::new(e), Box::new(int(1)))
}
fn call(f: &str, xs: Vec<Expr>) -> Expr {
    Expr::Call(f.to_string(), xs)
}
fn st(kind: StmtKind) -> Stmt {
    Stmt { kind, line: 0 }
}
fn let_(n: &str, ty: Option<Ty>, e: Expr) -> Stmt {
    st(StmtKind::Let { name: n.to_string(), ty, init: e })
}
fn assign(p: Expr, v: Expr) -> Stmt {
    st(StmtKind::Assign { place: p, value: v })
}
fn ex(e: Expr) -> Stmt {
    st(StmtKind::Expr(e))
}
fn pr(e: Expr) -> Stmt {
    st(StmtKind::Print(e))
}
fn blk(xs: Vec<Stmt>) -> Stmt {
    st(StmtKind::Block(Block { stmts: xs, ..Default::default() }))
}
fn clo(mv: bool, caps: &[&str], ret: Ty, stmts: Vec<Stmt>, tail: Option<Expr>) -> Expr {
    Expr::Closure(Box::new(Clo { id: 0, mv, caps: caps.iter().map(|c| c.to_string()).collect(), ret, body: Block { stmts, tail, ..Default::default() } }))
}
fn callv(e: Expr) -> Expr {
    Expr::CallVal(Box::new(e))
}
fn slice(m: Mode, v: &str, lo: i64, hi: i64) -> Expr {
    Expr::Slice(m, v.to_string(), lo, hi)
}
// A name the statement consumes: kept declared (it may not be declared
// again in the block), no longer offered for a use.
fn gone(n: &str) -> (String, Ty) {
    (n.to_string(), Ty::Unit)
}

// Every statement the alphabet offers in `env`, with the names it declares.
fn alphabet(env: &Env) -> Vec<(Stmt, Vec<(String, Ty)>)> {
    let mut out = Vec::new();
    let ints = ["x", "y"];
    let excl = env.excl_refs();
    let refs = env.any_refs();
    let fresh = |base: &str| -> Option<String> { (1..=2).map(|i| format!("{}{}", base, i)).find(|n| !env.has(n)) };
    // Declarations.
    if let Some(r) = fresh("r") {
        for x in ints {
            out.push((let_(&r, None, bor(Mode::Excl, var(x))), vec![(r.clone(), Ty::refi(Mode::Excl))]));
        }
        for q in &excl {
            out.push((let_(&r, None, bor(Mode::Excl, de(var(q)))), vec![(r.clone(), Ty::refi(Mode::Excl))]));
        }
        if env.has("v1") {
            out.push((let_(&r, None, bor(Mode::Excl, Expr::Index("v1".into(), 0))), vec![(r.clone(), Ty::refi(Mode::Excl))]));
        }
        for x in ints {
            out.push((let_(&r, None, call("pass", vec![bor(Mode::Excl, var(x))])), vec![(r.clone(), Ty::refi(Mode::Excl))]));
        }
    }
    if let Some(s) = fresh("s") {
        for x in ints {
            out.push((let_(&s, None, bor(Mode::Shared, var(x))), vec![(s.clone(), Ty::refi(Mode::Shared))]));
        }
        for q in &excl {
            out.push((let_(&s, None, bor(Mode::Shared, de(var(q)))), vec![(s.clone(), Ty::refi(Mode::Shared))]));
        }
        if env.has("v1") {
            out.push((let_(&s, None, bor(Mode::Shared, Expr::Index("v1".into(), 0))), vec![(s.clone(), Ty::refi(Mode::Shared))]));
        }
        if env.has("q1") {
            out.push((let_(&s, None, Expr::Field(Box::new(var("q1")))), vec![(s.clone(), Ty::refi(Mode::Shared))]));
        }
    }
    if !env.has("h1") {
        out.push((let_("h1", Some(Ty::Hold), Expr::HoldLit(Box::new(bor(Mode::Excl, var("x"))))), vec![("h1".into(), Ty::Hold)]));
        for q in &excl {
            out.push((let_("h1", Some(Ty::Hold), Expr::HoldLit(Box::new(var(q)))), vec![("h1".into(), Ty::Hold)]));
        }
    } else if !env.has("q1") {
        out.push((let_("q1", None, bor(Mode::Shared, var("h1"))), vec![("q1".into(), Ty::Ref(Box::new(Ty::Hold), Mode::Shared))]));
    }
    if !env.has("v1") {
        out.push((let_("v1", Some(Ty::Vec), call("Vec::new", vec![])), vec![("v1".into(), Ty::Vec)]));
    }
    if let Some(t) = fresh("t") {
        let h = Ty::Handle(Box::new(Ty::Unit));
        for x in ints {
            out.push((let_(&t, None, Expr::Spawn("bump".into(), vec![bor(Mode::Excl, var(x))])), vec![(t.clone(), h.clone())]));
        }
        for q in &excl {
            out.push((let_(&t, None, Expr::Spawn("bump".into(), vec![var(q)])), vec![(t.clone(), h.clone())]));
        }
        if env.has("h1") {
            out.push((let_(&t, None, Expr::Spawn("bump_hold".into(), vec![var("h1")])), vec![(t.clone(), h.clone())]));
        }
        let hi = Ty::Handle(Box::new(Ty::I32));
        for x in ints {
            out.push((let_(&t, None, Expr::Spawn("peek".into(), vec![bor(Mode::Shared, var(x))])), vec![(t.clone(), hi.clone())]));
        }
    }
    // Mutexes (`spec/19` §2).
    if !env.has("m1") {
        out.push((let_("m1", Some(Ty::Mutex), call("Mutex::new", vec![int(0)])), vec![("m1".into(), Ty::Mutex)]));
    } else {
        if !env.has("g1") {
            out.push((let_("g1", None, call("lock", vec![bor(Mode::Shared, var("m1"))])), vec![("g1".into(), Ty::Guard)]));
        }
        out.push((blk(vec![let_("g9", None, call("lock", vec![bor(Mode::Shared, var("m1"))])), assign(de(var("g9")), add1(de(var("g9"))))]), vec![]));
        out.push((pr(call("read_locked", vec![bor(Mode::Shared, var("m1"))])), vec![]));
        if let Some(t) = fresh("t") {
            out.push((let_(&t, None, Expr::Spawn("add_locked".into(), vec![bor(Mode::Shared, var("m1"))])), vec![(t.clone(), Ty::Handle(Box::new(Ty::Unit)))]));
            if env.has("h1") {
                out.push((let_(&t, None, Expr::Spawn("lock_add_hold".into(), vec![bor(Mode::Shared, var("m1")), bor(Mode::Shared, var("h1"))])), vec![(t.clone(), Ty::Handle(Box::new(Ty::Unit)))]));
            }
        }
    }
    if env.has("g1") {
        out.push((assign(de(var("g1")), add1(de(var("g1")))), vec![]));
        out.push((pr(de(var("g1"))), vec![]));
    }
    if env.has("h1") {
        if let Some(t) = fresh("t") {
            out.push((let_(&t, None, Expr::Spawn("read_hold".into(), vec![bor(Mode::Shared, var("h1"))])), vec![(t.clone(), Ty::Handle(Box::new(Ty::I32)))]));
        }
    }
    // Atomics (`spec/19` §2a, D-0249, D-0253): operations through a shared
    // reference, a wait that a notification must end, and the accesses that
    // conflict with a shared one.
    let aty = Ty::Ref(Box::new(Ty::Atomic), Mode::Shared);
    if !env.has("at1") {
        out.push((let_("at1", Some(Ty::Atomic), call("Atomic::new", vec![int(0)])), vec![("at1".into(), Ty::Atomic)]));
    } else if env.names.get("at1") == Some(&Ty::Atomic) {
        let a = || bor(Mode::Shared, var("at1"));
        out.push((st(StmtKind::Discard(call("Atomic::fetch_add", vec![a(), int(1)]))), vec![]));
        out.push((ex(call("Atomic::store", vec![a(), int(1)])), vec![]));
        out.push((blk(vec![ex(call("Atomic::store", vec![a(), int(1)])), ex(call("Atomic::notify_all", vec![a()]))]), vec![]));
        out.push((ex(call("Atomic::wait", vec![a(), int(0)])), vec![]));
        out.push((pr(call("Atomic::load", vec![a()])), vec![]));
        if !env.has("ar1") {
            out.push((let_("ar1", None, a()), vec![("ar1".into(), aty.clone())]));
        }
        if !env.has("ax1") {
            out.push((let_("ax1", None, bor(Mode::Excl, var("at1"))), vec![("ax1".into(), Ty::Ref(Box::new(Ty::Atomic), Mode::Excl))]));
        }
        if !env.has("at2") {
            out.push((let_("at2", None, var("at1")), vec![("at2".into(), Ty::Atomic), gone("at1")]));
        }
    }
    if env.names.get("ar1") == Some(&aty) {
        let hu = Ty::Handle(Box::new(Ty::Unit));
        let bodies = vec![
            vec![st(StmtKind::Discard(call("Atomic::fetch_add", vec![var("ar1"), int(1)])))],
            vec![ex(call("Atomic::store", vec![var("ar1"), int(1)])), ex(call("Atomic::notify_all", vec![var("ar1")]))],
            vec![ex(call("Atomic::wait", vec![var("ar1"), int(0)]))],
        ];
        for body in bodies {
            if let Some(t) = fresh("t") {
                out.push((let_(&t, None, Expr::SpawnClo(Box::new(clo(true, &["ar1"], Ty::Unit, body, None)))), vec![(t.clone(), hu.clone())]));
            }
        }
        out.push((st(StmtKind::Discard(call("Atomic::fetch_add", vec![var("ar1"), int(1)]))), vec![]));
    }
    // D-0258: a static atomic `S1`, every thread's by name: its operations,
    // a wait, a shared reference kept, closures that reach it with no
    // capture, and the refused exclusive borrow.
    {
        let sv = || bor(Mode::Shared, var("S1"));
        out.push((st(StmtKind::Discard(call("Atomic::fetch_add", vec![sv(), int(1)]))), vec![]));
        out.push((blk(vec![ex(call("Atomic::store", vec![sv(), int(1)])), ex(call("Atomic::notify_all", vec![sv()]))]), vec![]));
        out.push((ex(call("Atomic::wait", vec![sv(), int(0)])), vec![]));
        out.push((pr(call("Atomic::load", vec![sv()])), vec![]));
        if !env.has("sr1") {
            out.push((let_("sr1", None, sv()), vec![("sr1".into(), aty.clone())]));
        }
        if !env.has("sx1") {
            out.push((let_("sx1", None, bor(Mode::Excl, var("S1"))), vec![("sx1".into(), Ty::Ref(Box::new(Ty::Atomic), Mode::Excl))]));
        }
        let hu = Ty::Handle(Box::new(Ty::Unit));
        let bodies = vec![
            vec![st(StmtKind::Discard(call("Atomic::fetch_add", vec![sv(), int(1)])))],
            vec![ex(call("Atomic::store", vec![sv(), int(1)])), ex(call("Atomic::notify_all", vec![sv()]))],
            vec![ex(call("Atomic::wait", vec![sv(), int(0)]))],
        ];
        for body in bodies {
            if let Some(t) = fresh("t") {
                out.push((let_(&t, None, Expr::SpawnClo(Box::new(clo(true, &[], Ty::Unit, body, None)))), vec![(t.clone(), hu.clone())]));
            }
        }
    }
    // Channels of exclusive references (`spec/19` §3, D-0213's lend).
    let cty = Ty::Chan(Box::new(Ty::refi(Mode::Excl)));
    if !env.has("c1") {
        out.push((let_("c1", Some(cty.clone()), call("Channel::new", vec![int(1)])), vec![("c1".into(), cty.clone())]));
    } else {
        if let Some(t) = fresh("t") {
            out.push((let_(&t, None, Expr::Spawn("take_one".into(), vec![bor(Mode::Shared, var("c1"))])), vec![(t.clone(), Ty::Handle(Box::new(Ty::Unit)))]));
        }
        for x in ints {
            out.push((st(StmtKind::Discard(call("Channel::send", vec![bor(Mode::Shared, var("c1")), bor(Mode::Excl, var(x))]))), vec![]));
        }
        for r in &excl {
            out.push((st(StmtKind::Discard(call("Channel::send", vec![bor(Mode::Shared, var("c1")), var(r)]))), vec![]));
        }
        out.push((ex(call("Channel::close", vec![bor(Mode::Shared, var("c1"))])), vec![]));
        let some = Block { stmts: vec![assign(de(var("k")), add1(de(var("k"))))], ..Default::default() };
        out.push((st(StmtKind::Match { scrut: call("Channel::recv", vec![bor(Mode::Shared, var("c1"))]), binder: "k".into(), some, none: Block::default() }), vec![]));
    }
    // A vector of exclusive references: references kept in storage the
    // vector owns, not lent when the vector moves to a thread (`spec/19` §1).
    if !env.has("w1") {
        out.push((let_("w1", Some(Ty::VecR), call("Vec::new", vec![])), vec![("w1".into(), Ty::VecR)]));
    } else if !env.has("w1moved") {
        for x in ints {
            out.push((ex(call("Vec::push", vec![bor(Mode::Excl, var("w1")), bor(Mode::Excl, var(x))])), vec![]));
        }
        for r in &excl {
            out.push((ex(call("Vec::push", vec![bor(Mode::Excl, var("w1")), var(r)])), vec![]));
        }
        let w0 = Expr::Index("w1".into(), 0);
        out.push((assign(de(w0.clone()), int(5)), vec![]));
        out.push((pr(de(w0)), vec![]));
        out.push((ex(call("Vec::pop", vec![bor(Mode::Excl, var("w1"))])), vec![]));
        if let Some(t) = fresh("t") {
            out.push((let_(&t, None, Expr::Spawn("bump_first".into(), vec![var("w1")])), vec![(t.clone(), Ty::Handle(Box::new(Ty::Unit))), ("w1moved".into(), Ty::Unit)]));
            out.push((let_(&t, None, Expr::Spawn("swap_first".into(), vec![var("w1")])), vec![(t.clone(), Ty::Handle(Box::new(Ty::Unit))), ("w1moved".into(), Ty::Unit)]));
        }
    }
    // Closures (`rule.fn.closure`, `[Closure-Call]`, D-0239).
    let hu = Ty::Handle(Box::new(Ty::Unit));
    let fu = Ty::Fn(Box::new(Ty::Unit));
    let fi = Ty::Fn(Box::new(Ty::I32));
    if let Some(c) = fresh("cl") {
        for x in ints {
            out.push((let_(&c, None, clo(false, &[x], Ty::Unit, vec![assign(var(x), add1(var(x)))], None)), vec![(c.clone(), fu.clone())]));
        }
        out.push((let_(&c, None, clo(false, &["y"], Ty::Unit, vec![pr(var("y"))], None)), vec![(c.clone(), fu.clone())]));
        for r in &excl {
            out.push((let_(&c, None, clo(false, &[r], Ty::Unit, vec![assign(de(var(r)), add1(de(var(r))))], None)), vec![(c.clone(), fu.clone())]));
        }
    }
    if let Some(k) = fresh("mk") {
        out.push((let_(&k, None, clo(true, &["x"], Ty::I32, vec![assign(var("x"), add1(var("x")))], Some(var("x")))), vec![(k.clone(), fi.clone())]));
        for r in &excl {
            out.push((let_(&k, None, clo(true, &[r], Ty::Unit, vec![assign(de(var(r)), add1(de(var(r))))], None)), vec![(k.clone(), fu.clone())]));
        }
    }
    for (c, rt) in env.closures("cl").into_iter().chain(env.closures("mk")) {
        if rt == Ty::Unit {
            out.push((ex(callv(var(&c))), vec![]));
            out.push((ex(call("call_val", vec![var(&c)])), vec![]));
            out.push((ex(call("call_shared", vec![bor(Mode::Shared, var(&c))])), vec![]));
        } else {
            out.push((pr(callv(var(&c))), vec![]));
        }
        if !env.has("fr1") {
            out.push((let_("fr1", None, bor(Mode::Shared, var(&c))), vec![("fr1".into(), Ty::Ref(Box::new(Ty::Fn(Box::new(rt.clone()))), Mode::Shared))]));
        }
    }
    if let Some(Ty::Ref(f, _)) = env.names.get("fr1") {
        if **f == fu {
            out.push((ex(callv(de(var("fr1")))), vec![]));
        } else {
            out.push((pr(callv(de(var("fr1")))), vec![]));
        }
    }
    if let Some(t) = fresh("t") {
        for (k, rt) in env.closures("mk") {
            out.push((let_(&t, None, Expr::SpawnClo(Box::new(var(&k)))), vec![(t.clone(), Ty::Handle(Box::new(rt)))]));
        }
        for r in &excl {
            out.push((let_(&t, None, Expr::SpawnClo(Box::new(clo(true, &[r], Ty::Unit, vec![assign(de(var(r)), add1(de(var(r))))], None)))), vec![(t.clone(), hu.clone())]));
        }
    }
    out.push((blk(vec![let_("cl9", None, clo(false, &["x"], Ty::Unit, vec![assign(var("x"), add1(var("x")))], None)), ex(callv(var("cl9")))]), vec![]));
    // Slices (`[Slice-Form]`, `[Slice-Index]`, D-0070, D-0238).
    if env.has("v1") {
        if let Some(sl) = fresh("sl") {
            out.push((let_(&sl, None, slice(Mode::Excl, "v1", 0, 1)), vec![(sl.clone(), Ty::Slice(Mode::Excl))]));
            out.push((let_(&sl, None, slice(Mode::Excl, "v1", 1, 2)), vec![(sl.clone(), Ty::Slice(Mode::Excl))]));
            out.push((let_(&sl, None, slice(Mode::Shared, "v1", 0, 1)), vec![(sl.clone(), Ty::Slice(Mode::Shared))]));
            for (e, m) in env.slices() {
                if m == Mode::Excl {
                    out.push((let_(&sl, None, slice(Mode::Excl, &e, 0, 1)), vec![(sl.clone(), Ty::Slice(Mode::Excl))]));
                }
            }
        }
        if let Some(t) = fresh("t") {
            out.push((let_(&t, None, Expr::Spawn("bump_slice".into(), vec![slice(Mode::Excl, "v1", 0, 1)])), vec![(t.clone(), hu.clone())]));
        }
        out.push((pr(call("first", vec![slice(Mode::Shared, "v1", 0, 1)])), vec![]));
        out.push((ex(call("bump_slice", vec![slice(Mode::Excl, "v1", 0, 1)])), vec![]));
    }
    for (sl, m) in env.slices() {
        out.push((pr(Expr::Index(sl.clone(), 0)), vec![]));
        if m == Mode::Excl {
            let e0 = Expr::Index(sl.clone(), 0);
            out.push((assign(e0.clone(), add1(e0)), vec![]));
            out.push((ex(call("bump_slice", vec![var(&sl)])), vec![]));
            if let Some(t) = fresh("t") {
                out.push((let_(&t, None, Expr::Spawn("bump_slice".into(), vec![var(&sl)])), vec![(t.clone(), hu.clone())]));
            }
        }
    }
    // Handles in a `Vec` and in a struct (`[Handle-Destructor]` where what
    // holds them is destroyed).
    if !env.has("hv1") {
        out.push((let_("hv1", Some(Ty::VecH), call("Vec::new", vec![])), vec![("hv1".into(), Ty::VecH)]));
    } else {
        for x in ints {
            out.push((ex(call("Vec::push", vec![bor(Mode::Excl, var("hv1")), Expr::Spawn("bump".into(), vec![bor(Mode::Excl, var(x))])])), vec![]));
        }
        for t in env.void_handles() {
            out.push((ex(call("Vec::push", vec![bor(Mode::Excl, var("hv1")), var(&t)])), vec![gone(&t)]));
        }
        let some = Block { stmts: vec![ex(Expr::Join("h".into()))], ..Default::default() };
        out.push((st(StmtKind::Match { scrut: call("Vec::pop", vec![bor(Mode::Excl, var("hv1"))]), binder: "h".into(), some, none: Block::default() }), vec![]));
    }
    if !env.has("j1") {
        for x in ints {
            out.push((let_("j1", Some(Ty::Job), Expr::JobLit(Box::new(Expr::Spawn("bump".into(), vec![bor(Mode::Excl, var(x))])))), vec![("j1".into(), Ty::Job)]));
        }
        for t in env.void_handles() {
            out.push((let_("j1", Some(Ty::Job), Expr::JobLit(Box::new(var(&t)))), vec![("j1".into(), Ty::Job), gone(&t)]));
        }
    }
    out.push((blk(vec![let_("hv9", Some(Ty::VecH), call("Vec::new", vec![])), ex(call("Vec::push", vec![bor(Mode::Excl, var("hv9")), Expr::Spawn("bump".into(), vec![bor(Mode::Excl, var("x"))])]))]), vec![]));
    out.push((blk(vec![let_("j9", Some(Ty::Job), Expr::JobLit(Box::new(Expr::Spawn("bump".into(), vec![bor(Mode::Excl, var("y"))]))))]), vec![]));
    // A channel of exclusive slices: a send lends the slice (`[Send]`).
    let csty = Ty::Chan(Box::new(Ty::Slice(Mode::Excl)));
    if env.has("v1") && !env.has("cs1") {
        out.push((let_("cs1", Some(csty.clone()), call("Channel::new", vec![int(1)])), vec![("cs1".into(), csty.clone())]));
    } else if env.has("cs1") {
        out.push((st(StmtKind::Discard(call("Channel::send", vec![bor(Mode::Shared, var("cs1")), slice(Mode::Excl, "v1", 0, 1)]))), vec![]));
        for (sl, m) in env.slices() {
            if m == Mode::Excl {
                out.push((st(StmtKind::Discard(call("Channel::send", vec![bor(Mode::Shared, var("cs1")), var(&sl)]))), vec![]));
            }
        }
        if let Some(t) = fresh("t") {
            out.push((let_(&t, None, Expr::Spawn("take_slice".into(), vec![bor(Mode::Shared, var("cs1"))])), vec![(t.clone(), hu.clone())]));
        }
    }
    // Slices and guards inside closures, and in threads through them.
    for (sl, m) in env.slices() {
        if m == Mode::Excl {
            let e0 = Expr::Index(sl.clone(), 0);
            let body = vec![assign(e0.clone(), add1(e0))];
            if let Some(c) = fresh("cl") {
                out.push((let_(&c, None, clo(false, &[&sl], Ty::Unit, body.clone(), None)), vec![(c.clone(), fu.clone())]));
            }
            if let Some(t) = fresh("t") {
                out.push((let_(&t, None, Expr::SpawnClo(Box::new(clo(true, &[&sl], Ty::Unit, body, None)))), vec![(t.clone(), hu.clone())]));
            }
        }
    }
    if env.has("g1") {
        let body = vec![assign(de(var("g1")), add1(de(var("g1"))))];
        if let Some(c) = fresh("cl") {
            out.push((let_(&c, None, clo(false, &["g1"], Ty::Unit, body.clone(), None)), vec![(c.clone(), fu.clone())]));
        }
        if let Some(t) = fresh("t") {
            out.push((let_(&t, None, Expr::SpawnClo(Box::new(clo(true, &["g1"], Ty::Unit, body, None)))), vec![(t.clone(), hu.clone()), gone("g1")]));
        }
    }
    if env.has("m1") {
        if let Some(c) = fresh("cl") {
            let body = vec![let_("g9", None, call("lock", vec![bor(Mode::Shared, var("m1"))])), assign(de(var("g9")), add1(de(var("g9"))))];
            out.push((let_(&c, None, clo(false, &["m1"], Ty::Unit, body, None)), vec![(c.clone(), fu.clone())]));
        }
    }
    // A closure that owns a resource (`[Fn-Value-Read]`: read, it moves,
    // leaving the place empty; `[Fn-Value-Empty-Call]`).
    if !env.has("v2") {
        out.push((let_("v2", Some(Ty::Vec), call("Vec::new", vec![])), vec![("v2".into(), Ty::Vec)]));
    } else if !env.has("mo1") && env.names.get("v2") == Some(&Ty::Vec) {
        let body = vec![ex(call("Vec::push", vec![bor(Mode::Excl, var("v2")), int(1)]))];
        out.push((let_("mo1", None, clo(true, &["v2"], Ty::Unit, body, None)), vec![("mo1".into(), fu.clone()), gone("v2")]));
    }
    if env.names.get("mo1") == Some(&fu) {
        out.push((ex(callv(var("mo1"))), vec![]));
        out.push((ex(call("call_val", vec![var("mo1")])), vec![gone("mo1")]));
        out.push((ex(call("call_twice", vec![var("mo1")])), vec![gone("mo1")]));
        out.push((ex(call("call_shared", vec![bor(Mode::Shared, var("mo1"))])), vec![]));
        if let Some(t) = fresh("t") {
            out.push((let_(&t, None, Expr::SpawnClo(Box::new(var("mo1")))), vec![(t.clone(), hu.clone()), gone("mo1")]));
        }
    }
    // Arrays: literal-index elements are places of the flow analysis
    // (`[Index-Checked]`), unlike a `Vec`'s.
    if !env.has("a1") {
        out.push((let_("a1", Some(Ty::Arr), Expr::ArrLit(vec![int(0), int(0)])), vec![("a1".into(), Ty::Arr)]));
    } else {
        for k in 0..2 {
            let e = Expr::Index("a1".into(), k);
            out.push((assign(e.clone(), add1(e.clone())), vec![]));
            out.push((pr(e.clone()), vec![]));
            out.push((ex(call("bump", vec![bor(Mode::Excl, e.clone())])), vec![]));
            if let Some(r) = fresh("r") {
                out.push((let_(&r, None, bor(Mode::Excl, e.clone())), vec![(r.clone(), Ty::refi(Mode::Excl))]));
            }
            if let Some(s2) = fresh("s") {
                out.push((let_(&s2, None, bor(Mode::Shared, e.clone())), vec![(s2.clone(), Ty::refi(Mode::Shared))]));
            }
            if let Some(t) = fresh("t") {
                out.push((let_(&t, None, Expr::Spawn("bump".into(), vec![bor(Mode::Excl, e.clone())])), vec![(t.clone(), hu.clone())]));
                out.push((let_(&t, None, Expr::Spawn("bump_slice".into(), vec![slice(Mode::Excl, "a1", k, k + 1)])), vec![(t.clone(), hu.clone())]));
            }
        }
        if let Some(sl) = fresh("sl") {
            out.push((let_(&sl, None, slice(Mode::Excl, "a1", 0, 1)), vec![(sl.clone(), Ty::Slice(Mode::Excl))]));
            out.push((let_(&sl, None, slice(Mode::Shared, "a1", 0, 2)), vec![(sl.clone(), Ty::Slice(Mode::Shared))]));
        }
    }
    // Loops: a counted `while` around one statement, run twice (the flow
    // analysis' join at the back edge; a borrow or a thread of the first
    // pass meeting the second).
    let lp = |body: Vec<Stmt>| {
        let mut stmts = body;
        stmts.push(assign(var("i"), add1(var("i"))));
        blk(vec![let_("i", Some(Ty::I32), int(0)), st(StmtKind::While { cond: Expr::Gt(Box::new(int(2)), Box::new(var("i"))), body: Block { stmts, ..Default::default() } })])
    };
    out.push((lp(vec![ex(call("bump", vec![bor(Mode::Excl, var("x"))]))]), vec![]));
    out.push((lp(vec![let_("r9", None, bor(Mode::Excl, var("x"))), assign(de(var("r9")), add1(de(var("r9"))))]), vec![]));
    out.push((lp(vec![let_("t9", None, Expr::Spawn("bump".into(), vec![bor(Mode::Excl, var("x"))]))]), vec![]));
    for r in &excl {
        out.push((lp(vec![assign(de(var(r)), add1(de(var(r))))]), vec![]));
        out.push((lp(vec![assign(var(r), bor(Mode::Excl, var("y")))]), vec![]));
    }
    if env.has("v1") {
        out.push((lp(vec![ex(call("Vec::push", vec![bor(Mode::Excl, var("v1")), int(7)]))]), vec![]));
    }
    if env.has("w1") && !env.has("w1moved") {
        for r in &excl {
            out.push((lp(vec![ex(call("Vec::push", vec![bor(Mode::Excl, var("w1")), var(r)]))]), vec![]));
        }
        out.push((lp(vec![ex(call("Vec::push", vec![bor(Mode::Excl, var("w1")), bor(Mode::Excl, var("x"))]))]), vec![]));
    }
    if env.has("hv1") {
        out.push((lp(vec![ex(call("Vec::push", vec![bor(Mode::Excl, var("hv1")), Expr::Spawn("bump".into(), vec![bor(Mode::Excl, var("x"))])]))]), vec![]));
    }
    if env.has("h1") {
        out.push((lp(vec![ex(call("bump_hold", vec![var("h1")]))]), vec![]));
    }
    for (c, rt) in env.closures("cl") {
        if rt == Ty::Unit {
            out.push((lp(vec![ex(callv(var(&c)))]), vec![]));
        }
    }
    for (sl, m) in env.slices() {
        if m == Mode::Excl {
            let e0 = Expr::Index(sl.clone(), 0);
            out.push((lp(vec![assign(e0.clone(), add1(e0))]), vec![]));
        }
    }
    // Loops left early: `while (2 > i) { i = i + 1; A; if (i > 1) { B;
    // break; } C; }` and its `continue` (the flow analysis' edges to the
    // exit and the test; `[Break]` exits the blocks it leaves, ending their
    // bindings and waiting for their handles).
    let lpj = |pre: Vec<Stmt>, inner: Vec<Stmt>, post: Vec<Stmt>, brk: bool| {
        let mut b = inner;
        b.push(st(if brk { StmtKind::Break } else { StmtKind::Continue }));
        let mut stmts = vec![assign(var("i"), add1(var("i")))];
        stmts.extend(pre);
        stmts.push(st(StmtKind::If { cond: Expr::Gt(Box::new(var("i")), Box::new(int(1))), then: Block { stmts: b, ..Default::default() }, els: None }));
        stmts.extend(post);
        blk(vec![let_("i", Some(Ty::I32), int(0)), st(StmtKind::While { cond: Expr::Gt(Box::new(int(2)), Box::new(var("i"))), body: Block { stmts, ..Default::default() } })])
    };
    for brk in [true, false] {
        let bx = || ex(call("bump", vec![bor(Mode::Excl, var("x"))]));
        out.push((lpj(vec![let_("t9", None, Expr::Spawn("bump".into(), vec![bor(Mode::Excl, var("x"))]))], vec![], vec![], brk), vec![]));
        out.push((lpj(vec![], vec![let_("t9", None, Expr::Spawn("bump".into(), vec![bor(Mode::Excl, var("x"))]))], vec![bx()], brk), vec![]));
        out.push((lpj(vec![let_("r9", None, bor(Mode::Excl, var("x")))], vec![], vec![assign(de(var("r9")), add1(de(var("r9"))))], brk), vec![]));
        out.push((lpj(vec![], vec![let_("r9", None, bor(Mode::Excl, var("x"))), assign(de(var("r9")), add1(de(var("r9"))))], vec![bx()], brk), vec![]));
        for r in &excl {
            out.push((lpj(vec![], vec![assign(var(r), bor(Mode::Excl, var("x")))], vec![assign(de(var(r)), add1(de(var(r))))], brk), vec![]));
            out.push((lpj(vec![assign(de(var(r)), add1(de(var(r))))], vec![assign(var(r), bor(Mode::Excl, var("y")))], vec![], brk), vec![]));
        }
        if env.has("w1") && !env.has("w1moved") {
            out.push((lpj(vec![], vec![ex(call("Vec::push", vec![bor(Mode::Excl, var("w1")), bor(Mode::Excl, var("x"))]))], vec![bx()], brk), vec![]));
        }
        if env.has("hv1") {
            out.push((lpj(vec![], vec![ex(call("Vec::push", vec![bor(Mode::Excl, var("hv1")), Expr::Spawn("bump".into(), vec![bor(Mode::Excl, var("x"))])]))], vec![bx()], brk), vec![]));
        }
        for (c, rt) in env.closures("cl") {
            if rt == Ty::Unit {
                out.push((lpj(vec![], vec![ex(callv(var(&c)))], vec![bx()], brk), vec![]));
            }
        }
    }
    // Uses.
    for x in ints {
        out.push((assign(var(x), add1(var(x))), vec![]));
        out.push((ex(call("bump", vec![bor(Mode::Excl, var(x))])), vec![]));
    }
    out.push((ex(call("two", vec![bor(Mode::Excl, var("x")), bor(Mode::Excl, var("y"))])), vec![]));
    out.push((ex(call("swap", vec![bor(Mode::Excl, var("x")), bor(Mode::Excl, var("y"))])), vec![]));
    for r in &excl {
        out.push((assign(de(var(r)), add1(de(var(r)))), vec![]));
        out.push((ex(call("bump", vec![var(r)])), vec![]));
        out.push((ex(call("two", vec![var(r), var(r)])), vec![]));
        out.push((ex(call("two", vec![var(r), bor(Mode::Excl, var("x"))])), vec![]));
        // D-0235: `swap` and `replace` through a reference passed on whole.
        out.push((ex(call("swap", vec![var(r), bor(Mode::Excl, var("y"))])), vec![]));
        out.push((st(StmtKind::Discard(call("replace", vec![var(r), int(9)]))), vec![]));
        for x in ints {
            out.push((assign(var(r), bor(Mode::Excl, var(x))), vec![]));
        }
        out.push((blk(vec![let_("z", Some(Ty::I32), int(3)), assign(var(r), bor(Mode::Excl, var("z")))]), vec![]));
    }
    for r in &refs {
        out.push((pr(de(var(r))), vec![]));
        out.push((pr(call("peek", vec![var(r)])), vec![]));
    }
    if env.has("h1") {
        let hr = Expr::Field(Box::new(var("h1")));
        out.push((assign(de(hr.clone()), add1(de(hr.clone()))), vec![]));
        out.push((ex(call("bump_hold", vec![var("h1")])), vec![]));
        out.push((pr(call("read_hold", vec![bor(Mode::Shared, var("h1"))])), vec![]));
    }
    if env.has("q1") {
        let qr = Expr::Field(Box::new(var("q1")));
        out.push((pr(de(qr.clone())), vec![]));
        out.push((assign(de(qr.clone()), int(1)), vec![]));
        out.push((ex(call("bump", vec![qr.clone()])), vec![]));
        out.push((pr(call("peek", vec![qr])), vec![]));
    }
    if env.has("v1") {
        out.push((ex(call("Vec::push", vec![bor(Mode::Excl, var("v1")), int(7)])), vec![]));
        out.push((ex(call("Vec::pop", vec![bor(Mode::Excl, var("v1"))])), vec![]));
        out.push((assign(Expr::Index("v1".into(), 0), int(3)), vec![]));
        out.push((pr(Expr::Index("v1".into(), 0)), vec![]));
    }
    for (n, t) in &env.names {
        match t {
            Ty::Handle(r) if **r == Ty::Unit => out.push((ex(Expr::Join(n.clone())), vec![])),
            Ty::Handle(_) => out.push((pr(Expr::Join(n.clone())), vec![])),
            _ => {}
        }
    }
    // Compound statements.
    out.push((blk(vec![let_("w", None, bor(Mode::Excl, var("x"))), assign(de(var("w")), add1(de(var("w"))))]), vec![]));
    out.push((
        st(StmtKind::If {
            cond: Expr::Gt(Box::new(var("x")), Box::new(int(1))),
            then: Block { stmts: vec![ex(call("bump", vec![bor(Mode::Excl, var("y"))]))], ..Default::default() },
            els: Some(Block { stmts: vec![assign(var("y"), add1(var("y")))], ..Default::default() }),
        }),
        vec![],
    ));
    out
}

fn program(stmts: &[Stmt]) -> Program {
    let mut body = vec![let_("x", Some(Ty::I32), int(1)), let_("y", Some(Ty::I32), int(2))];
    body.extend(stmts.iter().cloned());
    body.push(pr(var("x")));
    body.push(pr(var("y")));
    let main = Func { name: "main".into(), params: vec![], ret: Ty::Unit, body: Block { stmts: body, ..Default::default() }, line: 0 };
    let mut used = BTreeSet::new();
    let text = {
        let mut s = String::new();
        print::block(&main.body, 0, &mut s);
        s
    };
    for (h, _) in print::HELPERS {
        if text.contains(&format!("{}(", h)) || text.contains(&format!("spawn({},", h)) {
            used.insert(*h);
        }
    }
    let mut funcs = Vec::new();
    let mut hold = text.contains("Hold");
    let job = text.contains("Job");
    for (h, src) in print::HELPERS {
        if used.contains(h) {
            let p = parse::parse(&format!("{}\nfn main()\n{{\n}}\n", src)).expect("helper");
            funcs.push(p.func(h).unwrap().clone());
            if print::helper_needs_hold(h) {
                hold = true;
            }
        }
    }
    funcs.push(main);
    let statics = if text.contains("S1") { vec![("S1".to_string(), 0)] } else { Vec::new() };
    Program { hold, job, funcs, statics }
}

// Every well-scoped sequence of up to n statements.
fn sequences(n: usize) -> Vec<Vec<Stmt>> {
    let mut out = Vec::new();
    let mut env = Env::default();
    env.names.insert("x".into(), Ty::I32);
    env.names.insert("y".into(), Ty::I32);
    fn go(env: &Env, cur: &mut Vec<Stmt>, n: usize, out: &mut Vec<Vec<Stmt>>) {
        out.push(cur.clone());
        if cur.len() == n {
            return;
        }
        for (s, decls) in alphabet(env) {
            let mut e2 = env.clone();
            for (k, t) in decls {
                match k.strip_prefix('-') {
                    Some(g) => {
                        e2.names.remove(g);
                    }
                    None => {
                        e2.names.insert(k, t);
                    }
                }
            }
            cur.push(s);
            go(&e2, cur, n, out);
            cur.pop();
        }
    }
    go(&env, &mut Vec::new(), n, &mut out);
    out
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn random_seq(rng: &mut Rng, len: usize) -> Vec<Stmt> {
    let mut env = Env::default();
    env.names.insert("x".into(), Ty::I32);
    env.names.insert("y".into(), Ty::I32);
    let mut cur = Vec::new();
    for _ in 0..len {
        let a = alphabet(&env);
        let (s, decls) = a[(rng.next() % a.len() as u64) as usize].clone();
        for (k, t) in decls {
            match k.strip_prefix('-') {
                Some(g) => {
                    env.names.remove(g);
                }
                None => {
                    env.names.insert(k, t);
                }
            }
        }
        cur.push(s);
    }
    cur
}

pub struct Item {
    pub text: String,
    pub report: explore::Report,
}

pub fn render(stmts: &[Stmt]) -> Option<(String, Program)> {
    let p = program(stmts);
    let text = print::program(&p, "");
    // Read back, so the model runs exactly the text the tools will.
    let parsed = parse::parse(&text).ok()?;
    Some((text, parsed))
}

fn interesting(r: &explore::Report) -> bool {
    r.outcomes.len() > 1 || r.outcomes.iter().any(|o| matches!(&o.end, explore::End::Fault { phase: "dynamic", .. }))
}

pub fn main(args: &[String]) -> ExitCode {
    let mut size = 2;
    let mut out_dir = String::from("model-programs");
    let mut max = usize::MAX;
    let mut random = 0;
    let mut len = 6;
    let mut seed = 1u64;
    let mut i = 0;
    while i < args.len() {
        let v = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--size" => size = v.parse().unwrap_or(2),
            "--out" => out_dir = v,
            "--max" => max = v.parse().unwrap_or(usize::MAX),
            "--random" => random = v.parse().unwrap_or(0),
            "--len" => len = v.parse().unwrap_or(6),
            "--seed" => seed = v.parse().unwrap_or(1),
            _ => {
                eprintln!("enum: unknown option {}", args[i]);
                return ExitCode::from(2);
            }
        }
        i += 2;
    }
    let mut seqs = sequences(size);
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    for _ in 0..random {
        seqs.push(random_seq(&mut rng, len));
    }
    eprintln!("enum: {} programs (size up to {}, {} random of {})", seqs.len(), size, random, len);
    let mut items: Vec<Item> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for (k, s) in seqs.iter().enumerate() {
        let Some((text, prog)) = render(s) else {
            eprintln!("enum: program {} does not read back", k);
            continue;
        };
        if !seen.insert(text.clone()) {
            continue;
        }
        let report = explore::run(&prog, true);
        for o in &report.outcomes {
            let key = match &o.end {
                explore::End::Ok => "ok".to_string(),
                explore::End::Fault { diag, phase, .. } => format!("{} ({})", diag, phase),
                explore::End::TooBig => "too-big".into(),
            };
            *tally.entry(key).or_default() += 1;
        }
        if k % 5000 == 0 {
            eprintln!("enum: {} / {}", k, seqs.len());
        }
        items.push(Item { text, report });
    }
    // Choose what to write.
    let (mut keep, rest): (Vec<usize>, Vec<usize>) = (0..items.len()).partition(|&i| interesting(&items[i].report));
    if keep.len() > max {
        let step = keep.len() as f64 / max as f64;
        keep = (0..max).map(|j| keep[(j as f64 * step) as usize]).collect();
    } else if keep.len() + rest.len() > max {
        let room = max - keep.len();
        let step = rest.len() as f64 / room.max(1) as f64;
        keep.extend((0..room).map(|j| rest[(j as f64 * step) as usize]));
    } else {
        keep.extend(rest);
    }
    keep.sort();
    std::fs::create_dir_all(&out_dir).expect("out dir");
    let mut index = String::from("file\toutcomes\tstates\n");
    for (j, i) in keep.iter().enumerate() {
        let it = &items[*i];
        let name = format!("p{:06}.cb", j);
        let mut header = String::new();
        for o in &it.report.outcomes {
            writeln!(header, "// MODEL: {}", o.text()).unwrap();
        }
        // The header shifts every line by its length; the model's lines are
        // those of the text below it, so the header is one line per outcome
        // and the outcome lines are corrected by reading the file back.
        let full = format!("{}{}", header, it.text);
        let prog = parse::parse(&full).expect("reads back");
        let r = explore::run(&prog, true);
        let mut header = String::new();
        for o in &r.outcomes {
            writeln!(header, "// MODEL: {}", o.text()).unwrap();
        }
        let full = format!("{}{}", header, it.text);
        std::fs::write(format!("{}/{}", out_dir, name), &full).expect("write");
        let outs: Vec<String> = r.outcomes.iter().map(|o| o.text()).collect();
        writeln!(index, "{}\t{}\t{}", name, outs.join(" | "), r.states).unwrap();
    }
    std::fs::write(format!("{}/index.tsv", out_dir), index).expect("index");
    eprintln!("enum: {} distinct programs, {} written to {}", items.len(), keep.len(), out_dir);
    for (k, v) in tally {
        eprintln!("  {:>7}  {}", v, k);
    }
    ExitCode::SUCCESS
}
