// The whole-program and lexical parts of `rule.control.frame-references`
// (`spec/14` §6, D-0282), written from the specification:
//
// - the access summaries `reads(F, p)` and `writes(F, p)` of every
//   function's reference parameters, computed over the program to a fixed
//   point;
// - each body's source positions: where every expression ends in source
//   (evaluation) order, which binding each mention names, and the loops,
//   for the rule's lexical liveness;
// - the call-site rule's "certainly overlap" on keys.
//
// The state-dependent half (inherited facts, the call-site rule, element
// accesses, evaluation order, moving out through a reference) runs in
// `statics.rs` beside `rule.control.flow-analysis`, over the same state.

use crate::ast::*;
use crate::statics::StepK;
use std::collections::{BTreeSet, HashMap};

// A projection: a field path from a parameter's referent, cut at the first
// index or range (the empty path is the whole referent).
pub type Proj = Vec<StepK>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub reads: BTreeSet<Proj>,
    pub writes: BTreeSet<Proj>,
}

// Per function with a body, per parameter: its summary when it is a
// reference parameter (a reference or a slice), else None.
pub type Summaries = HashMap<String, Vec<Option<Summary>>>;

// How a callee's parameter takes its argument, for the rule: as a reference
// or slice in a mode, as a type parameter's type (instantiated in the
// fragment at a reference only by `Vec<ref<…>>` and `Channel<ref<…>>`), as a
// value that holds a reference, or as a value that holds none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PK {
    Ref(Mode),
    TyParam,
    Holds,
    Plain,
}

pub fn ty_pk(t: &Ty) -> PK {
    match t {
        Ty::Ref(_, m) | Ty::Slice(m) => PK::Ref(*m),
        // A closure or `fn` value is a frame-held reference (D-0281: a
        // closure passed and called is tier S): it holds its captures' borrows.
        Ty::Fn(_) => PK::Holds,
        t if t.holds_ref() || *t == Ty::VecR => PK::Holds,
        _ => PK::Plain,
    }
}

// The parameters of the `std` functions the fragment calls, as the rule
// sees them; None for a name that is not one.
pub fn std_params(f: &str) -> Option<Vec<PK>> {
    use Mode::*;
    Some(match f {
        "Mutex::new" | "Atomic::new" | "Channel::new" => vec![PK::Plain],
        "Vec::new" => vec![],
        "lock" | "Atomic::load" | "Atomic::notify_all" | "Channel::recv" | "Channel::close" => vec![PK::Ref(Shared)],
        "Atomic::store" | "Atomic::fetch_add" | "Atomic::wait" => vec![PK::Ref(Shared), PK::Plain],
        "Vec::push" => vec![PK::Ref(Excl), PK::TyParam],
        "Vec::pop" => vec![PK::Ref(Excl)],
        "Vec::index_exclusive" => vec![PK::Ref(Excl), PK::Plain],
        "swap" => vec![PK::Ref(Excl), PK::Ref(Excl)],
        "replace" => vec![PK::Ref(Excl), PK::Plain],
        "Channel::send" => vec![PK::Ref(Shared), PK::TyParam],
        _ => return None,
    })
}

// Whether a call's result is a reference (a reference or a slice) itself.
fn returns_ref(prog: &Program, f: &str) -> bool {
    match prog.func(f) {
        Some(func) => matches!(func.ret, Ty::Ref(..) | Ty::Slice(_)),
        None => f == "Vec::index_exclusive",
    }
}

fn param_pks(prog: &Program, f: &str) -> Vec<PK> {
    match prog.func(f) {
        Some(func) => func.params.iter().map(|p| ty_pk(&p.0)).collect(),
        None => std_params(f).unwrap_or_default(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum U {
    Read,
    Write,
    Borrow(Mode),
}

// One function body's summary pass: which (parameter, path) each local
// reference is rooted at, and the reads and writes found.
struct SumCx<'a> {
    prog: &'a Program,
    sums: &'a Summaries,
    // The reference parameters by name: their index.
    params: HashMap<String, usize>,
    // Those of them that are shared.
    shared: BTreeSet<String>,
    // "a local reference bound from a p-rooted borrow, or from a call on one
    // that returns a reference, is rooted where that is" (flow-insensitive:
    // every binding of the local counts).
    alias: HashMap<String, BTreeSet<(usize, Proj)>>,
    out: Vec<Summary>,
}

impl<'a> SumCx<'a> {
    // Where a reference value is rooted, as (parameter, path).
    fn ref_root(&self, e: &Expr) -> BTreeSet<(usize, Proj)> {
        match e {
            Expr::Var(n) => self.var_roots(n),
            Expr::Borrow(_, p) => self.place_root(p),
            Expr::Slice(_, n, ..) => self.var_roots(n),
            Expr::Call(f, args) if returns_ref(self.prog, f) => {
                let pks = param_pks(self.prog, f);
                for (i, a) in args.iter().enumerate() {
                    if matches!(pks.get(i), Some(PK::Ref(_))) {
                        return self.ref_root(a);
                    }
                }
                BTreeSet::new()
            }
            _ => BTreeSet::new(),
        }
    }

    fn var_roots(&self, n: &str) -> BTreeSet<(usize, Proj)> {
        let mut s = self.alias.get(n).cloned().unwrap_or_default();
        if let Some(i) = self.params.get(n) {
            s.insert((*i, vec![]));
        }
        s
    }

    // Where a place lies in a parameter's referent, if it does.
    fn place_root(&self, e: &Expr) -> BTreeSet<(usize, Proj)> {
        match e {
            Expr::Deref(a) => self.ref_root(a),
            // `p.r`, `(*p).r` of a reference to a Hold (auto-deref).
            Expr::Field(a) => {
                let base = match &**a {
                    Expr::Deref(b) => self.ref_root(b),
                    Expr::Var(n) => self.var_roots(n),
                    other => self.place_root(other),
                };
                base.into_iter().map(|(i, mut p)| {
                    if !p.iter().any(|s| !matches!(s, StepK::Field)) {
                        p.push(StepK::Field);
                    }
                    (i, p)
                }).collect()
            }
            // `s[k]` of a slice parameter: cut at the index.
            Expr::Index(n, _) => self.var_roots(n),
            _ => BTreeSet::new(),
        }
    }

    fn record(&mut self, roots: BTreeSet<(usize, Proj)>, u: U) {
        for (i, p) in roots {
            let s = &mut self.out[i];
            match u {
                U::Read | U::Borrow(Mode::Shared) => s.reads.insert(p),
                U::Write | U::Borrow(Mode::Excl) => s.writes.insert(p),
            };
        }
    }

    fn visit(&mut self, e: &Expr, u: U) {
        match e {
            Expr::Int(_) | Expr::Var(_) | Expr::Join(_) => {}
            Expr::Deref(a) => {
                let r = self.place_root(e);
                self.record(r, u);
                self.visit(a, U::Read);
            }
            Expr::Field(a) => {
                let r = self.place_root(e);
                self.record(r, u);
                match &**a {
                    Expr::Deref(b) => self.visit(b, U::Read),
                    other => self.visit(other, U::Read),
                }
            }
            Expr::Index(..) => {
                let r = self.place_root(e);
                self.record(r, u);
            }
            Expr::Borrow(m, a) => self.visit(a, U::Borrow(*m)),
            Expr::Slice(m, n, ..) => {
                let r = self.var_roots(n);
                self.record(r, U::Borrow(*m));
            }
            Expr::Add(a, b) | Expr::Gt(a, b) => {
                self.visit(a, U::Read);
                self.visit(b, U::Read);
            }
            Expr::Call(f, args) | Expr::Spawn(f, args) => {
                let pks = param_pks(self.prog, f);
                for (i, a) in args.iter().enumerate() {
                    self.visit(a, U::Read);
                    let roots = self.ref_root(a);
                    if roots.is_empty() {
                        continue;
                    }
                    let callee = self.sums.get(f.as_str()).and_then(|v| v.get(i).cloned().flatten());
                    match (pks.get(i), callee) {
                        // "passing a p-rooted reference at π to G's reference
                        // parameter q puts π·ρ in reads for each ρ in reads(G,
                        // q), and likewise for writes"
                        (Some(PK::Ref(_)), Some(g)) if self.prog.func(f).is_some() => {
                            for (pi, p) in roots {
                                for rho in &g.reads {
                                    self.out[pi].reads.insert(concat(&p, rho));
                                }
                                for rho in &g.writes {
                                    self.out[pi].writes.insert(concat(&p, rho));
                                }
                            }
                        }
                        // "to a function with no body, π in reads, and in
                        // writes when the parameter is exclusive"; a function
                        // of `std` has the same summary.
                        (Some(PK::Ref(m)), _) => {
                            let m = *m;
                            self.record(roots.clone(), U::Read);
                            if m == Mode::Excl {
                                self.record(roots, U::Write);
                            }
                        }
                        // "to a parameter of a type parameter's type … π in
                        // reads, and in writes unless the argument is a shared
                        // borrow or a shared binding"
                        (Some(PK::TyParam), _) => {
                            let shared = matches!(a, Expr::Borrow(Mode::Shared, _)) || matches!(a, Expr::Var(n) if self.is_shared_ref(n));
                            self.record(roots.clone(), U::Read);
                            if !shared {
                                self.record(roots, U::Write);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Expr::HoldLit(a) | Expr::JobLit(a) | Expr::CallVal(a) | Expr::SpawnClo(a) => self.visit(a, U::Read),
            Expr::ArrLit(xs) => xs.iter().for_each(|x| self.visit(x, U::Read)),
            // "a closure that captures a p-rooted reference puts π in both"
            Expr::Closure(c) => {
                for x in &c.caps {
                    let r = self.var_roots(x);
                    self.record(r.clone(), U::Read);
                    self.record(r, U::Write);
                }
            }
        }
    }

    fn is_shared_ref(&self, n: &str) -> bool {
        self.shared.contains(n)
    }

    fn bind(&mut self, name: &str, init: &Expr) {
        let r = self.ref_root(init);
        if !r.is_empty() {
            self.alias.entry(name.to_string()).or_default().extend(r);
        }
    }

    fn block(&mut self, b: &Block) {
        for s in &b.stmts {
            match &s.kind {
                StmtKind::Let { name, init, .. } => {
                    self.visit(init, U::Read);
                    self.bind(name, init);
                }
                StmtKind::Assign { place, value } => {
                    match place {
                        // A whole local written: not a place of a referent.
                        Expr::Var(n) => self.bind(n, value),
                        p => self.visit(p, U::Write),
                    }
                    self.visit(value, U::Read);
                }
                StmtKind::Expr(e) | StmtKind::Print(e) | StmtKind::Discard(e) => self.visit(e, U::Read),
                StmtKind::Block(b) => self.block(b),
                StmtKind::If { cond, then, els } => {
                    self.visit(cond, U::Read);
                    self.block(then);
                    if let Some(e) = els {
                        self.block(e);
                    }
                }
                StmtKind::While { cond, body } => {
                    self.visit(cond, U::Read);
                    self.block(body);
                }
                StmtKind::Match { scrut, binder, some, none } => {
                    self.visit(scrut, U::Read);
                    self.bind(binder, scrut);
                    self.block(some);
                    self.block(none);
                }
                StmtKind::Break | StmtKind::Continue => {}
            }
        }
        if let Some(t) = &b.tail {
            self.visit(t, U::Read);
        }
    }
}

fn concat(a: &[StepK], b: &[StepK]) -> Proj {
    let mut p = a.to_vec();
    p.extend(b.iter().cloned());
    p
}

// "The summaries are computed over the whole program to a fixed point."
// The fragment has no `unsafe`; the `std` functions are not in `prog.funcs`
// (their summaries are the fixed {ε} the rule gives them, in `std_params`).
pub fn summaries(prog: &Program) -> Summaries {
    let mut sums: Summaries = HashMap::new();
    for f in &prog.funcs {
        sums.insert(f.name.clone(), f.params.iter().map(|(t, _)| matches!(ty_pk(t), PK::Ref(_)).then(Summary::default)).collect());
    }
    for _ in 0..64 {
        let mut next = sums.clone();
        for f in &prog.funcs {
            let params: HashMap<String, usize> = f.params.iter().enumerate().filter(|(_, p)| matches!(ty_pk(&p.0), PK::Ref(_))).map(|(i, p)| (p.1.clone(), i)).collect();
            let shared = f.params.iter().filter(|p| ty_pk(&p.0) == PK::Ref(Mode::Shared)).map(|p| p.1.clone()).collect();
            let mut cx = SumCx { prog, sums: &sums, params, shared, alias: HashMap::new(), out: vec![Summary::default(); f.params.len()] };
            // The aliases of a body are found before its accesses are read
            // through them: two passes.
            cx.block(&f.body);
            cx.out = vec![Summary::default(); f.params.len()];
            cx.block(&f.body);
            let out = cx.out;
            let v = next.get_mut(&f.name).unwrap();
            for (i, s) in out.into_iter().enumerate() {
                if let Some(Some(cur)) = v.get_mut(i) {
                    cur.reads.extend(s.reads);
                    cur.writes.extend(s.writes);
                }
            }
        }
        if next == sums {
            break;
        }
        sums = next;
    }
    sums
}

// A body's source positions (the rule's lexical liveness): every
// expression's position at its end in evaluation order, which is source
// order in the fragment; each binding's declaration and mentions; the
// loops.
#[derive(Default)]
pub struct Positions {
    pub at: HashMap<*const Expr, usize>,
    pub stmt_end: HashMap<*const Stmt, usize>,
    // A `let` or `match` statement -> the binding it declares.
    pub decl: HashMap<*const Stmt, usize>,
    pub decl_seq: Vec<usize>,
    pub mentions: Vec<Vec<usize>>,
    pub loops: Vec<(usize, usize)>,
    next: usize,
    scopes: Vec<HashMap<String, usize>>,
}

impl Positions {
    // `params` are bound at the entry (a closure body's captures too), as
    // bindings 0, 1, ….
    pub fn of(params: &[String], body: &Block) -> Positions {
        let mut p = Positions { scopes: vec![HashMap::new()], ..Positions::default() };
        for n in params {
            p.declare(n);
        }
        p.block(body);
        p
    }

    fn declare(&mut self, n: &str) -> usize {
        let id = self.decl_seq.len();
        self.decl_seq.push(self.next);
        self.mentions.push(Vec::new());
        self.scopes.last_mut().unwrap().insert(n.to_string(), id);
        id
    }

    // "A mention resolves to the binding in scope where it is written."
    fn mention(&mut self, n: &str) {
        let at = self.next;
        if let Some(id) = self.scopes.iter().rev().find_map(|s| s.get(n).copied()) {
            self.mentions[id].push(at);
        }
    }

    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Int(_) => {}
            Expr::Var(n) | Expr::Index(n, _) | Expr::Join(n) | Expr::Slice(_, n, _, _) => self.mention(n),
            Expr::Deref(a) | Expr::Field(a) | Expr::Borrow(_, a) | Expr::HoldLit(a) | Expr::JobLit(a) | Expr::CallVal(a) | Expr::SpawnClo(a) => self.expr(a),
            // A closure mentions what it captures; its body is a body of its own.
            Expr::Closure(c) => {
                for x in &c.caps {
                    self.mention(x);
                }
            }
            Expr::Add(a, b) | Expr::Gt(a, b) => {
                self.expr(a);
                self.expr(b);
            }
            Expr::Call(_, xs) | Expr::Spawn(_, xs) | Expr::ArrLit(xs) => xs.iter().for_each(|x| self.expr(x)),
        }
        self.at.insert(e as *const Expr, self.next);
        self.next += 1;
    }

    fn block(&mut self, b: &Block) {
        self.scopes.push(HashMap::new());
        for s in &b.stmts {
            self.stmt(s);
        }
        if let Some(t) = &b.tail {
            self.expr(t);
        }
        self.scopes.pop();
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::Let { name, init, .. } => {
                self.expr(init);
                let id = self.declare(name);
                self.decl.insert(s as *const Stmt, id);
            }
            StmtKind::Assign { place, value } => {
                self.expr(place);
                self.expr(value);
            }
            StmtKind::Expr(e) | StmtKind::Print(e) | StmtKind::Discard(e) => self.expr(e),
            StmtKind::Block(b) => self.block(b),
            StmtKind::If { cond, then, els } => {
                self.expr(cond);
                self.block(then);
                if let Some(e) = els {
                    self.block(e);
                }
            }
            StmtKind::While { cond, body } => {
                let start = self.next;
                self.expr(cond);
                self.block(body);
                self.loops.push((start, self.next));
            }
            StmtKind::Match { scrut, binder, some, none } => {
                self.expr(scrut);
                self.scopes.push(HashMap::new());
                let id = self.declare(binder);
                self.decl.insert(s as *const Stmt, id);
                self.block(some);
                self.scopes.pop();
                self.block(none);
            }
            StmtKind::Break | StmtKind::Continue => {}
        }
        self.stmt_end.insert(s as *const Stmt, self.next);
        self.next += 1;
    }

    // "A frame-held reference h is live at a point P when the binding of that
    // name in scope at P is mentioned after P in the source, or anywhere in a
    // loop that encloses P and that its declaration precedes."
    pub fn live_after(&self, id: usize, p: usize) -> bool {
        let Some(ms) = self.mentions.get(id) else { return false };
        ms.iter().any(|&s| s > p) || self.loops.iter().any(|&(st, en)| st <= p && p < en && self.decl_seq[id] < st && ms.iter().any(|&s| st <= s && s < en))
    }
}

// Two keys "certainly overlap" when their roots are equal (the caller
// compares them), their paths agree step by step, and the longer path's
// remainder holds no range that may be empty. Every index and bound of the
// fragment is a literal, so "may overlap" never arises here.
pub fn certain(a: &[StepK], b: &[StepK]) -> bool {
    let n = a.len().min(b.len());
    let agree = a.iter().zip(b.iter()).all(|(x, y)| match (x, y) {
        (StepK::Field, StepK::Field) => true,
        (StepK::Idx(i), StepK::Idx(j)) => i == j,
        (StepK::Range(lo, hi), StepK::Idx(k)) | (StepK::Idx(k), StepK::Range(lo, hi)) => lo <= k && k < hi,
        (StepK::Range(lo, hi), StepK::Range(lo2, hi2)) => lo.max(lo2) < hi.min(hi2),
        _ => false,
    });
    let rest = if a.len() > n { &a[n..] } else { &b[n..] };
    agree && rest.iter().all(|s| !matches!(s, StepK::Range(lo, hi) if lo >= hi))
}
