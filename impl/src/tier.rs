// D-0282 (`rule.control.frame-references`, spec/14 §6): access summaries.
//
// CobaltC checks a conflict when an access happens (D-0018), not when a
// borrow is formed: passing `g` exclusively to a function that writes only
// `g.types` does not conflict with a live shared reference into `g.prog`.
// A static rule at the call site that saw only the parameter's type would
// have to assume the callee reaches all of `*g`, and would reject such
// programs. Each function therefore has an *access summary*: for each of
// its reference parameters, the projections of the referent it may read
// and may write, directly or through the functions it calls, computed over
// the whole program to a fixed point.
//
// A projection is a field path from the parameter's referent, cut at the
// first index (an element is inside the container the path names so far);
// the empty path is the whole referent. The summary over-approximates: a
// form the walk does not follow (a call through a `fn` value, a function
// with no body, a closure that captures a reference rooted in a parameter)
// counts as reading, and for an exclusive parameter writing, the whole of
// what it reaches. A write through a reference read out of the referent
// (a reference kept in data) is not attributed: such a reference is tier D,
// whose conflicts the run checks (D-0281 §15).

use crate::ast::{Block, Expr, ExprKind, FnDecl, Mode, Stmt, Type};
use crate::interp::Items;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

pub type Proj = Vec<String>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParamAccess {
    pub reads: BTreeSet<Proj>,
    pub writes: BTreeSet<Proj>,
}

impl ParamAccess {
    fn add(set: &mut BTreeSet<Proj>, p: Proj) -> bool {
        // A path below one already present adds nothing.
        if set.iter().any(|q| p.starts_with(q)) {
            return false;
        }
        set.retain(|q| !q.starts_with(&p));
        set.insert(p);
        true
    }

    pub fn read(&mut self, p: Proj) -> bool {
        Self::add(&mut self.reads, p)
    }

    pub fn write(&mut self, p: Proj) -> bool {
        Self::add(&mut self.writes, p)
    }
}

#[derive(Default)]
pub struct Summaries {
    by_fn: HashMap<String, Vec<ParamAccess>>,
}

impl Summaries {
    // The summary of `key`'s parameter `i`, or `None` when the function has
    // none (no body: the caller assumes the parameter's mode reaches all).
    pub fn param(&self, key: &str, i: usize) -> Option<&ParamAccess> {
        self.by_fn.get(key).and_then(|v| v.get(i))
    }
}

// D-0283: a point check at a call. The callee may access, through argument
// `arg`, the projection `fields` of its referent with `mode`; whether that
// meets `other` (another argument's referent, or what a live holder of the
// caller borrows, with `other_mode`) depends on values (two indexes), so
// the run compares the places when the call is made, and a call whose
// places overlap faults there, before the callee runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PointOther {
    Arg(usize),
    Holder(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointCheck {
    pub arg: usize,
    pub fields: Proj,
    pub mode: Mode,
    pub other: PointOther,
    pub other_mode: Mode,
    // The callee as a message names it.
    pub callee: String,
}

impl PointCheck {
    // The fault's message.
    pub fn message(&self) -> String {
        let what = if self.mode == Mode::Exclusive { "write" } else { "read" };
        format!("`{}` may {} through argument {}, and a live reference reaches the same place: the call is checked when it is made (D-0283)", self.callee, what, self.arg + 1)
    }
}

// A checked body's name: the function's, with its type arguments.
pub fn body_key(name: &str, targs: &[Type]) -> String {
    if targs.is_empty() {
        name.to_string()
    } else {
        format!("{}<{}>", name, targs.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(","))
    }
}

pub fn record_point_check(items: &Items, site: (usize, usize), pc: PointCheck) {
    let mut m = items.point_checks.lock().unwrap();
    let v = m.entry(site).or_insert_with(|| Arc::new(Vec::new()));
    if !v.contains(&pc) {
        Arc::make_mut(v).push(pc);
    }
    items.any_point_checks.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn point_checks_at(items: &Items, site: (usize, usize)) -> Option<Arc<Vec<PointCheck>>> {
    if !items.any_point_checks.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    items.point_checks.lock().unwrap().get(&site).cloned()
}

// The program's summaries, computed once per `Items` (its functions are
// all loaded before the static pass runs).
pub fn summaries(items: &Items) -> Arc<Summaries> {
    items.tier_summaries.get_or_init(|| Arc::new(compute(items))).clone()
}

fn is_ref_param(t: &Type) -> Option<Mode> {
    match t {
        Type::Ref(_, m) | Type::Slice(_, m) => Some(m.clone()),
        Type::Named(n, a) if n == "std::StringView" && a.is_empty() => Some(Mode::Shared),
        _ => None,
    }
}

fn compute(items: &Items) -> Summaries {
    let mut keys: Vec<&String> = items.fns.keys().collect();
    keys.sort();
    let mut s = Summaries::default();
    for k in &keys {
        let f = &items.fns[*k];
        s.by_fn.insert((*k).clone(), vec![ParamAccess::default(); f.params.len()]);
    }
    // Monotone: each round can only add projections, and the sets are
    // bounded by the program's field paths, so this ends.
    loop {
        let mut changed = false;
        for k in &keys {
            let f = items.fns[*k].clone();
            let mut w = Walk { items, sums: &s, env: HashMap::new(), out: vec![ParamAccess::default(); f.params.len()], shared: Default::default() };
            for p in &f.params {
                if matches!(is_ref_param(&p.ty), Some(Mode::Shared)) {
                    w.shared.insert(p.name.clone());
                }
            }
            for (i, p) in f.params.iter().enumerate() {
                if is_ref_param(&p.ty).is_some() {
                    w.env.insert(p.name.clone(), vec![(i, Vec::new(), false)]);
                }
            }
            w.block(&f.body);
            let out = std::mem::take(&mut w.out);
            drop(w);
            // A `std` type's fields are its own: a `std` function's access
            // to a parameter is to the whole of it (a `Vec`'s elements sit
            // behind its `ptr`, which `reserve` writes).
            // So is one with trusted code (`unsafe`), which reaches storage
            // through raw pointers the walk does not follow. Both take
            // their parameters' declared modes: an exclusive one may write
            // all it reaches, a shared one read it.
            let opaque = k.starts_with("std::") || has_unsafe(&f.body);
            let out: Vec<ParamAccess> = if opaque {
                f.params
                    .iter()
                    .map(|p| {
                        let mut b = ParamAccess::default();
                        match is_ref_param(&p.ty) {
                            Some(Mode::Exclusive) => {
                                b.read(Vec::new());
                                b.write(Vec::new());
                            }
                            Some(Mode::Shared) => {
                                b.read(Vec::new());
                            }
                            None => {}
                        }
                        b
                    })
                    .collect()
            } else {
                out
            };
            let cur = s.by_fn.get_mut(*k).unwrap();
            for (i, acc) in out.into_iter().enumerate() {
                for r in acc.reads {
                    changed |= cur[i].read(r);
                }
                for x in acc.writes {
                    changed |= cur[i].write(x);
                }
            }
        }
        if !changed {
            break;
        }
    }
    s
}

// A root: (parameter index, projection, cut). `cut` once the path crossed
// an index: further fields add nothing.
type Root = (usize, Proj, bool);

struct Walk<'a> {
    items: &'a Items,
    sums: &'a Summaries,
    // Local reference bindings rooted in a parameter's referent.
    env: HashMap<String, Vec<Root>>,
    out: Vec<ParamAccess>,
    // Local reference bindings known to be shared: what is passed through
    // one is only read.
    shared: std::collections::HashSet<String>,
}

fn extend(roots: Vec<Root>, field: Option<&str>) -> Vec<Root> {
    roots
        .into_iter()
        .map(|(i, mut p, cut)| match field {
            Some(f) if !cut => {
                p.push(f.to_string());
                (i, p, false)
            }
            Some(_) => (i, p, true),
            None => (i, p, true),
        })
        .collect()
}

impl<'a> Walk<'a> {
    fn read(&mut self, roots: &[Root]) {
        for (i, p, _) in roots {
            self.out[*i].read(p.clone());
        }
    }

    fn write(&mut self, roots: &[Root]) {
        for (i, p, _) in roots {
            self.out[*i].write(p.clone());
        }
    }

    // The parameter-rooted places a place expression denotes.
    fn place(&self, e: &Expr) -> Vec<Root> {
        match &e.kind {
            ExprKind::Paren(x) => self.place(x),
            // A reference binding used as a place (auto-deref in `r.f`,
            // `*r`): its referent.
            ExprKind::Path(segs, _) if segs.len() == 1 => self.env.get(&segs[0]).cloned().unwrap_or_default(),
            ExprKind::Deref(x) => self.reference(x),
            ExprKind::Field(b, f) => extend(self.place(b), Some(f)),
            ExprKind::Index(b, _) => extend(self.place(b), None),
            ExprKind::SliceOf(_, b, _, _) => extend(self.place(b), None),
            ExprKind::Call(..) => extend(self.reference(e), None),
            _ => Vec::new(),
        }
    }

    // The parameter-rooted places a reference-valued expression reaches.
    fn reference(&self, e: &Expr) -> Vec<Root> {
        match &e.kind {
            ExprKind::Paren(x) => self.reference(x),
            ExprKind::Path(segs, _) if segs.len() == 1 => self.env.get(&segs[0]).cloned().unwrap_or_default(),
            ExprKind::Borrow(_, x) => self.place(x),
            ExprKind::SliceOf(_, b, _, _) => extend(self.place(b), None),
            // A call returning a reference with one reference parameter
            // (D-0011) reaches inside what that argument reaches.
            ExprKind::Call(c, args) => {
                let Some(f) = self.callee(c) else { return Vec::new() };
                if is_ref_param(&f.ret).is_none() && !matches!(&f.ret, Type::Named(n, _) if n == "std::Option") {
                    return Vec::new();
                }
                let mut out = Vec::new();
                for (p, a) in f.params.iter().zip(args.iter()) {
                    if is_ref_param(&p.ty).is_some() {
                        out.extend(extend(self.reference(a), None));
                    }
                }
                out
            }
            ExprKind::If(_, t, e2) => {
                let mut out = t.tail.as_ref().map(|x| self.reference(x)).unwrap_or_default();
                if let Some(x) = e2 {
                    out.extend(self.reference(x));
                }
                out
            }
            ExprKind::Block(b) => b.tail.as_ref().map(|x| self.reference(x)).unwrap_or_default(),
            ExprKind::Match(_, arms) => arms.iter().flat_map(|a| self.reference(&a.body)).collect(),
            _ => Vec::new(),
        }
    }

    fn callee(&self, c: &Expr) -> Option<Arc<FnDecl>> {
        if let ExprKind::Path(segs, _) = &c.kind {
            return self.items.fns.get(&segs.join("::")).cloned();
        }
        None
    }

    fn block(&mut self, b: &Block) {
        for s in &b.stmts {
            self.stmt(s);
        }
        if let Some(t) = &b.tail {
            self.expr(t);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Let { name, init, ty, .. } => {
                if let Some(e) = init {
                    self.expr(e);
                    let r = self.reference(e);
                    let is_ref = ty.as_ref().map_or(true, |t| is_ref_param(t).is_some() || matches!(t, Type::Named(n, _) if n == "std::Option"));
                    if !r.is_empty() && is_ref {
                        self.env.insert(name.clone(), r);
                    } else {
                        self.env.remove(name);
                    }
                    let shared_ty = ty.as_ref().map_or(false, |t| matches!(is_ref_param(t), Some(Mode::Shared)));
                    let shared_init = matches!(&e.kind, ExprKind::Borrow(Mode::Shared, _) | ExprKind::SliceOf(Mode::Shared, ..));
                    if shared_ty || (ty.is_none() && shared_init) {
                        self.shared.insert(name.clone());
                    } else {
                        self.shared.remove(name);
                    }
                } else {
                    self.env.remove(name);
                }
            }
            Stmt::Destructure { init, names, .. } => {
                self.expr(init);
                for n in names {
                    self.env.remove(n);
                }
            }
            Stmt::Expr(e) | Stmt::BlockLike(e) => self.expr(e),
        }
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Paren(x) | ExprKind::Unary(_, x) | ExprKind::Propagate(x) => self.expr(x),
            ExprKind::Binary(_, a, b) => {
                self.expr(a);
                self.expr(b);
            }
            ExprKind::Assign(l, r) => {
                self.expr(r);
                self.subexprs_of_place(l);
                let w = self.place(l);
                self.write(&w);
                // `r = e` for a reference binding `r` rebinds it.
                if let ExprKind::Path(segs, _) = &l.kind {
                    if segs.len() == 1 && self.env.contains_key(&segs[0]) {
                        let nr = self.reference(r);
                        let entry = self.env.entry(segs[0].clone()).or_default();
                        for x in nr {
                            if !entry.contains(&x) {
                                entry.push(x);
                            }
                        }
                    }
                }
            }
            ExprKind::Borrow(m, x) => {
                self.subexprs_of_place(x);
                let p = self.place(x);
                match m {
                    Mode::Exclusive => self.write(&p),
                    Mode::Shared => self.read(&p),
                }
            }
            ExprKind::SliceOf(m, b, lo, hi) => {
                self.expr(lo);
                self.expr(hi);
                self.subexprs_of_place(b);
                let p = extend(self.place(b), None);
                match m {
                    Mode::Exclusive => self.write(&p),
                    Mode::Shared => self.read(&p),
                }
            }
            ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_) => {
                self.subexprs_of_place(e);
                let p = self.place(e);
                self.read(&p);
            }
            ExprKind::Path(segs, _) if segs.len() == 1 => {
                // A reference binding read whole (passed on, copied): what
                // it reaches is read; a call's own rule refines that.
                let p = self.env.get(&segs[0]).cloned().unwrap_or_default();
                self.read(&p);
            }
            ExprKind::Call(c, args) => self.call(c, args),
            ExprKind::StructLit(_, _, fs) => {
                for (_, x) in fs {
                    self.expr(x);
                }
            }
            ExprKind::ArrayLit(xs) => {
                for x in xs {
                    self.expr(x);
                }
            }
            ExprKind::ArrayRepeat(x, _) => self.expr(x),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => self.block(b),
            ExprKind::If(c, t, e2) => {
                self.expr(c);
                self.block(t);
                if let Some(x) = e2 {
                    self.expr(x);
                }
            }
            ExprKind::While(c, b, step) => {
                // Twice, so a rebinding in the body reaches its start.
                for _ in 0..2 {
                    self.expr(c);
                    self.block(b);
                    if let Some(s) = step {
                        self.expr(s);
                    }
                }
            }
            ExprKind::Match(scrut, arms) => {
                self.expr(scrut);
                let r = self.reference(scrut);
                let r = if r.is_empty() { extend(self.place(scrut), None) } else { extend(r, None) };
                for a in arms {
                    let saved = a.binder.as_ref().map(|b| (b.clone(), self.env.get(b).cloned()));
                    let excl_scrut = matches!(&strip(scrut).kind, ExprKind::Borrow(Mode::Exclusive, _) | ExprKind::SliceOf(Mode::Exclusive, ..))
                        || matches!(&strip(scrut).kind, ExprKind::Path(segs, _) if segs.len() == 1 && self.env.contains_key(&segs[0]) && !self.shared.contains(&segs[0]));
                    let was_shared = a.binder.as_ref().map_or(false, |b| self.shared.contains(b));
                    if let Some(b) = &a.binder {
                        if r.is_empty() {
                            self.env.remove(b);
                        } else {
                            self.env.insert(b.clone(), r.clone());
                        }
                        // Only a match on an exclusive borrow binds an
                        // exclusive reference; any other binder is read
                        // through (or is a copy).
                        if excl_scrut {
                            self.shared.remove(b);
                        } else {
                            self.shared.insert(b.clone());
                        }
                    }
                    self.expr(&a.body);
                    if let Some(b) = &a.binder {
                        if was_shared {
                            self.shared.insert(b.clone());
                        } else {
                            self.shared.remove(b);
                        }
                    }
                    if let Some((b, old)) = saved {
                        match old {
                            Some(o) => {
                                self.env.insert(b, o);
                            }
                            None => {
                                self.env.remove(&b);
                            }
                        }
                    }
                }
            }
            ExprKind::Return(Some(x)) => self.expr(x),
            ExprKind::Closure { captures, body, .. } => {
                // A closure that captures a reference rooted in a parameter
                // may do anything through it.
                for c in captures {
                    if let Some(p) = self.env.get(c).cloned() {
                        self.write(&p);
                        self.read(&p);
                    }
                }
                let saved = std::mem::take(&mut self.env);
                self.block(body);
                self.env = saved;
            }
            _ => {}
        }
    }

    // The index and argument expressions inside a place, evaluated.
    fn subexprs_of_place(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Paren(x) | ExprKind::Deref(x) | ExprKind::Field(x, _) => self.subexprs_of_place(x),
            ExprKind::Index(b, i) => {
                self.subexprs_of_place(b);
                self.expr(i);
            }
            ExprKind::SliceOf(_, b, lo, hi) => {
                self.subexprs_of_place(b);
                self.expr(lo);
                self.expr(hi);
            }
            ExprKind::Call(..) => self.expr(e),
            _ => {}
        }
    }

    fn call(&mut self, c: &Expr, args: &[Expr]) {
        let f = self.callee(c);
        for (j, a) in args.iter().enumerate() {
            let pmode = f.as_ref().and_then(|f| f.params.get(j)).and_then(|p| is_ref_param(&p.ty));
            match pmode {
                Some(m) => {
                    // The argument's own subexpressions, then what the
                    // callee does through the reference it is given.
                    match &a.kind {
                        ExprKind::Borrow(_, x) => self.subexprs_of_place(x),
                        ExprKind::SliceOf(_, b, lo, hi) => {
                            self.subexprs_of_place(b);
                            self.expr(lo);
                            self.expr(hi);
                        }
                        ExprKind::Path(..) => {}
                        _ => self.expr(a),
                    }
                    let roots = self.reference(a);
                    if roots.is_empty() {
                        continue;
                    }
                    let key = match &c.kind {
                        ExprKind::Path(segs, _) => segs.join("::"),
                        _ => String::new(),
                    };
                    // A function with a body, an empty one included, by its
                    // summary; `std`'s are whole by their declared modes.
                    match self.sums.param(&key, j).filter(|_| f.is_some()).cloned() {
                        Some(acc) => {
                            for (i, base, cut) in &roots {
                                for r in &acc.reads {
                                    let mut p = base.clone();
                                    if !cut {
                                        p.extend(r.iter().cloned());
                                    }
                                    self.out[*i].read(p);
                                }
                                for w in &acc.writes {
                                    let mut p = base.clone();
                                    if !cut {
                                        p.extend(w.iter().cloned());
                                    }
                                    self.write(&[(*i, p, false)]);
                                }
                            }
                        }
                        None => {
                            self.read(&roots);
                            if m == Mode::Exclusive {
                                self.write(&roots);
                            }
                        }
                    }
                }
                None => {
                    self.expr(a);
                    // An argument to an unknown callee (a `fn` value) or a
                    // type parameter: a reference passed may be used whole.
                    if f.is_none() || matches!(f.as_ref().and_then(|f| f.params.get(j)).map(|p| &p.ty), Some(Type::Named(_, a)) if a.is_empty()) {
                        let roots = self.reference(a);
                        self.read(&roots);
                        // A shared borrow handed to an unknown callee (`foreach`'s
                        // helpers, a `fn` value) can only be read through.
                        let shared = matches!(&a.kind, ExprKind::Borrow(Mode::Shared, _) | ExprKind::SliceOf(Mode::Shared, ..))
                            || matches!(&a.kind, ExprKind::Path(segs, _) if segs.len() == 1 && self.shared.contains(&segs[0]));
                        if f.is_none() && !shared {
                            self.write(&roots);
                        }
                    }
                }
            }
        }
        if f.is_none() {
            self.expr(c);
        }
    }
}

fn has_unsafe(b: &Block) -> bool {
    fn e(x: &Expr) -> bool {
        match &x.kind {
            ExprKind::Unsafe(_) => true,
            ExprKind::Paren(a) | ExprKind::Unary(_, a) | ExprKind::Propagate(a) | ExprKind::Deref(a) | ExprKind::Borrow(_, a) | ExprKind::Field(a, _) | ExprKind::ArrayRepeat(a, _) => e(a),
            ExprKind::Binary(_, a, c) | ExprKind::Index(a, c) | ExprKind::Assign(a, c) => e(a) || e(c),
            ExprKind::SliceOf(_, a, c, d) => e(a) || e(c) || e(d),
            ExprKind::Call(c, xs) => e(c) || xs.iter().any(e),
            ExprKind::StructLit(_, _, fs) => fs.iter().any(|(_, x)| e(x)),
            ExprKind::ArrayLit(xs) => xs.iter().any(e),
            ExprKind::Block(b) => has_unsafe(b),
            ExprKind::If(c, t, x) => e(c) || has_unsafe(t) || x.as_ref().map_or(false, |x| e(x)),
            ExprKind::While(c, t, x) => e(c) || has_unsafe(t) || x.as_ref().map_or(false, |x| e(x)),
            ExprKind::Match(c, arms) => e(c) || arms.iter().any(|a| e(&a.body)),
            ExprKind::Return(Some(x)) => e(x),
            ExprKind::Closure { body, .. } => has_unsafe(body),
            _ => false,
        }
    }
    b.stmts.iter().any(|s| match s {
        Stmt::Let { init, .. } => init.as_ref().map_or(false, |x| e(x)),
        Stmt::Destructure { init, .. } => e(init),
        Stmt::Expr(x) | Stmt::BlockLike(x) => e(x),
    }) || b.tail.as_ref().map_or(false, |x| e(x))
}

fn strip(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Paren(x) => strip(x),
        _ => e,
    }
}
