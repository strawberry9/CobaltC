// The static half of the model, written from the specification:
//
// - typing for the fragment (`spec/12`), including the mode rules:
//   `[Write-Not-Exclusive]`, `[Borrow-Exceeds-Source]` and D-0218's
//   `[T-Read-Through-Shared]`;
// - `rule.control.flow-analysis` (`spec/14` §6): the facts `valid`,
//   `deriv` and `escaped`, the transfer table and the discharge table, as
//   written, for the constructs of the fragment;
// - `rule.temporal.ref-escape` (`spec/10`);
// - `[Ref-Binding-Last-Use]` (`spec/14` §1, D-0111, D-0133): which
//   statement each local holding a reference ends after;
// - `rule.control.frame-references` (`spec/14` §6, D-0282): inherited
//   facts, the call-site rule with the access summaries of `frames.rs`,
//   element accesses, evaluation order and moving out through a reference,
//   over the same state, after the flow analysis; it only refutes.
//
// Where two static diagnostics apply, the first in source order is the one
// reported (D-0110). The analysis is intraprocedural; each function body
// is analysed on its own, helpers included.

use crate::ast::*;
use crate::frames::{self, Positions, Summaries, PK};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticErr {
    pub line: usize,
    pub diag: &'static str,
    pub why: String,
}

// Where a binding ends early: (function, block number in pre-order, the
// statement index) -> the bindings that end after that statement.
pub type Ends = HashMap<(String, usize, usize), Vec<String>>;

pub struct Statics {
    pub errors: Vec<StaticErr>,
    pub ends: Ends,
    // Every binding's type, per function.
    pub types: HashMap<String, HashMap<String, Ty>>,
    // D-0241: (function, line, binding) -> whether every `¬clash` the
    // discharge table resolves there for an access rooted at the binding
    // is *proven* (no run-time check licensed).
    pub proven: Proven,
}

pub type Proven = HashMap<(String, usize, String), bool>;

// `cobmodel check --pre-d0236`: the table as it stood before D-0236, a
// forgotten fact leaving its root unescaped (evidence for the finding).
pub static PRE_D0236: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// `--pre-d0243`: the closure row as it stood before D-0243, a captured
// reference binding left unescaped.
pub static PRE_D0243: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// `--pre-d0282`: without `rule.control.frame-references`.
pub static PRE_D0282: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Statics {
    pub fn first(&self) -> Option<&StaticErr> {
        self.errors.iter().min_by_key(|e| e.line)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tri {
    F,
    T,
    Q, // `?`: the join of T and F
}

fn join_tri(a: Tri, b: Tri) -> Tri {
    if a == b {
        a
    } else {
        Tri::Q
    }
}

// A syntactic place's root (`spec/14` §6: a binding x, or the referent
// `*q` of a tracked binding q of reference type, D-0071).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Root {
    Var(String),
    Deref(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StepK {
    Field,
    // A literal index place: only an array has them (D-0221: a `Vec`'s
    // elements are not places), and the fragment has no arrays.
    #[allow(dead_code)]
    Idx(i64),
    // A slice's step, its range [lo, hi) of literal bounds (D-0070).
    Range(i64, i64),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SPlace {
    pub root: Root,
    pub steps: Vec<StepK>,
}

// π1, π2 overlap iff one is a prefix of the other, literal indices
// compared by value; a range meets a literal index inside it or a literal
// range it intersects (an empty range meets neither).
fn overlap(a: &[StepK], b: &[StepK]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| step_meet(x, y))
}

fn step_meet(a: &StepK, b: &StepK) -> bool {
    match (a, b) {
        (StepK::Field, StepK::Field) => true,
        (StepK::Idx(x), StepK::Idx(y)) => x == y,
        (StepK::Range(i, j), StepK::Idx(k)) | (StepK::Idx(k), StepK::Range(i, j)) => i <= k && k < j,
        (StepK::Range(i, j), StepK::Range(i2, j2)) => i.max(i2) < j.min(j2),
        _ => false,
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Flow {
    valid: BTreeMap<String, Tri>,
    escaped: BTreeMap<Root, Tri>,
    deriv: BTreeMap<(String, SPlace, Mode), Tri>,
    // D-0282: the facts a frame-held reference inherits where the rows
    // above form none. They join and end as `deriv` facts do; no discharge
    // of the flow analysis reads them.
    inh: BTreeMap<(String, SPlace, Mode), Tri>,
    // No path reaches this point: it follows a `break` or `continue` in its
    // block, and a join ignores it.
    dead: bool,
}

impl Flow {
    fn join(&self, o: &Flow) -> Flow {
        if self.dead {
            return o.clone();
        }
        if o.dead {
            return self.clone();
        }
        let mut r = Flow::default();
        let keys: BTreeSet<_> = self.valid.keys().chain(o.valid.keys()).cloned().collect();
        for k in keys {
            r.valid.insert(k.clone(), join_tri(*self.valid.get(&k).unwrap_or(&Tri::F), *o.valid.get(&k).unwrap_or(&Tri::F)));
        }
        let keys: BTreeSet<_> = self.escaped.keys().chain(o.escaped.keys()).cloned().collect();
        for k in keys {
            r.escaped.insert(k.clone(), join_tri(*self.escaped.get(&k).unwrap_or(&Tri::F), *o.escaped.get(&k).unwrap_or(&Tri::F)));
        }
        let keys: BTreeSet<_> = self.deriv.keys().chain(o.deriv.keys()).cloned().collect();
        for k in keys {
            r.deriv.insert(k.clone(), join_tri(*self.deriv.get(&k).unwrap_or(&Tri::F), *o.deriv.get(&k).unwrap_or(&Tri::F)));
        }
        let keys: BTreeSet<_> = self.inh.keys().chain(o.inh.keys()).cloned().collect();
        for k in keys {
            r.inh.insert(k.clone(), join_tri(*self.inh.get(&k).unwrap_or(&Tri::F), *o.inh.get(&k).unwrap_or(&Tri::F)));
        }
        r
    }
    // D-0236 "Forgetting": a holder's facts cleared because it ends, is
    // reassigned or is moved leave their roots escaped -- the reference it
    // held may live on in a copy the analysis does not follow.
    fn forget_holder(&mut self, h: &str) {
        if PRE_D0236.load(std::sync::atomic::Ordering::Relaxed) {
            self.clear_holder(h);
            return;
        }
        let roots: Vec<String> = self
            .deriv
            .iter()
            .filter(|(k, v)| k.0 == h && **v != Tri::F)
            .filter_map(|(k, _)| match &k.1.root {
                Root::Var(x) => Some(x.clone()),
                Root::Deref(_) => None,
            })
            .collect();
        for x in roots {
            self.escaped.insert(Root::Var(x), Tri::T);
        }
        self.clear_holder(h);
    }
    fn clear_holder(&mut self, h: &str) {
        for (k, v) in self.deriv.iter_mut().chain(self.inh.iter_mut()) {
            if k.0 == h {
                *v = Tri::F;
            }
        }
    }
    fn clear_referent(&mut self, q: &str) {
        for (k, v) in self.deriv.iter_mut().chain(self.inh.iter_mut()) {
            if k.1.root == Root::Deref(q.to_string()) {
                *v = Tri::F;
            }
        }
    }
    // A holder's `deriv` facts, each with its value where not F (D-0288).
    fn deriv_facts(&self, h: &str) -> Vec<(SPlace, Mode, Tri)> {
        self.deriv.iter().filter(|(k, v)| k.0 == h && **v != Tri::F).map(|(k, v)| (k.1.clone(), k.2, *v)).collect()
    }
    // A holder's facts for D-0282: its `deriv` facts and those it inherited,
    // each with its value where not F.
    fn holder_facts(&self, h: &str) -> Vec<(SPlace, Mode, Tri)> {
        self.deriv.iter().chain(self.inh.iter()).filter(|(k, v)| k.0 == h && **v != Tri::F).map(|(k, v)| (k.1.clone(), k.2, *v)).collect()
    }
}

struct Binding {
    ty: Ty,
    block: usize, // the declaring block's number
    id: usize,    // its number in `Positions` (usize::MAX: none)
}

struct Cx<'a> {
    prog: &'a Program,
    fname: String,
    errors: Vec<StaticErr>,
    binds: HashMap<String, Vec<Binding>>, // innermost last: a nested block may shadow
    regions: Vec<BTreeSet<String>>,        // the names each open block declared
    // block number -> its parent's number (None for the body)
    parents: HashMap<usize, Option<usize>>,
    next_block: usize,
    // The referent block of each reference binding, where a visible borrow
    // gave it one (`rule.temporal.ref-escape`).
    referent: HashMap<String, Option<usize>>,
    ends: Ends,
    all_types: HashMap<String, Ty>,
    proven: Proven,
    // The body's result type (a function's, or a closure's).
    ret: Ty,
    // In a closure body: the captures. A capture is not a binding of the
    // body the analysis tracks: an access through it is never proven
    // (its root is the closure's field, or `*a_i` for a borrow capture).
    caps: BTreeSet<String>,
    // A binding holding a closure literal: (move, writes-self).
    clo_info: HashMap<String, (bool, bool)>,
    // D-0289, CHG-0384: a spawn pushed onto a `Vec` of handles adds its
    // facts to the `Vec`'s (`Vec::push(&mut hs, spawn(…))`).
    accumulate: bool,
    // A binding holding a `move` closure literal that captured a resource:
    // resource-typed by derivation (`rule.type.is-resource`).
    clo_owns: BTreeSet<String>,
    // The closure bodies analysed inside this one: their binding types.
    sub_types: HashMap<String, HashMap<String, Ty>>,
    // The open loops, innermost last: the number of regions open at the
    // body's entry, and the facts at each `break` and `continue` joined
    // (§5's edges to the loop's exit and its test).
    loops: Vec<(usize, Flow, Flow)>,
    // D-0282: the program's access summaries, this body's source positions,
    // and each frame-held reference bound as a copy of another: the one it
    // copies, followed back to the first (`a copy of a reference carries the
    // same path`).
    sums: &'a Summaries,
    pos: Positions,
    copies: HashMap<String, String>,
}

// How an expression is used: read for its value, or as a place to write
// or borrow.
#[derive(Clone, Copy, PartialEq)]
enum Use {
    Read,
    Write,
    Borrow(Mode),
}

impl<'a> Cx<'a> {
    fn err(&mut self, line: usize, diag: &'static str, why: String) {
        self.errors.push(StaticErr { line, diag, why });
    }

    fn ty_of(&self, n: &str) -> Option<Ty> {
        // D-0258: a static's name, where no binding shadows it.
        self.binds.get(n).and_then(|v| v.last()).map(|b| b.ty.clone()).or_else(|| if self.prog.is_static(n) { Some(Ty::Atomic) } else { None })
    }
    fn is_static_var(&self, e: &Expr) -> bool {
        matches!(e, Expr::Var(n) if self.prog.is_static(n) && self.binds.get(n).map_or(true, |v| v.is_empty()))
    }
    fn block_of(&self, n: &str) -> Option<usize> {
        self.binds.get(n).and_then(|v| v.last()).map(|b| b.block)
    }

    // The static type of a place expression, and whether it is reached
    // through a shared path (`via-shared`, D-0218; a binding's own path is
    // exclusive, `[Binding-Form]`).
    fn place_ty(&self, e: &Expr, u: Use) -> Option<(Ty, Mode)> {
        match e {
            Expr::Var(n) => Some((self.ty_of(n)?, Mode::Excl)),
            // `a[k]` of an array: a projection of the binding's path (`[Index-Checked]`).
            Expr::Index(v, _) if self.ty_of(v) == Some(Ty::Arr) => Some((Ty::I32, Mode::Excl)),
            // `s[k]` of a slice: an element, in the slice's mode (`[Slice-Index]`).
            Expr::Index(v, _) if matches!(self.ty_of(v), Some(Ty::Slice(_))) => match self.ty_of(v)? {
                Ty::Slice(m) => Some((Ty::I32, m)),
                _ => None,
            },
            // `*v[k] = e`, `&mut *v[k]` with v a Vec of exclusive references:
            // the element is reached by index_exclusive (`[Index-Vec]` "where
            // the place is written", read so that it covers the place under
            // `*`; the tools' reading, D-0219 finding 6).
            Expr::Deref(a) if matches!(u, Use::Write | Use::Borrow(Mode::Excl)) && matches!(&**a, Expr::Index(v, _) if self.ty_of(v) == Some(Ty::VecR)) => Some((Ty::I32, Mode::Excl)),
            Expr::Deref(a) => match self.value_ty(a)? {
                Ty::Ref(t, m) => Some((*t, m)),
                // `[Guard-Deref]`: the lock path, exclusive.
                Ty::Guard => Some((Ty::I32, Mode::Excl)),
                _ => None,
            },
            Expr::Field(a) => {
                // `e.r` of a Hold place, or of a reference to one
                // (`[Field-Access-Auto-Deref]`).
                if let Some((Ty::Hold, m)) = self.place_ty(a, Use::Read) {
                    return Some((Ty::refi(Mode::Excl), m));
                }
                match self.value_ty(a)? {
                    Ty::Ref(t, m) if *t == Ty::Hold => Some((Ty::refi(Mode::Excl), m)),
                    _ => None,
                }
            }
            // `[Index-Vec]`: index_exclusive where the place is written or
            // borrowed mut, index_shared elsewhere.
            Expr::Index(v, _) => {
                let et = match self.ty_of(v)? {
                    Ty::Vec => Ty::I32,
                    Ty::VecR => Ty::refi(Mode::Excl),
                    _ => return None,
                };
                let m = match u {
                    Use::Write | Use::Borrow(Mode::Excl) => Mode::Excl,
                    _ => Mode::Shared,
                };
                Some((et, m))
            }
            _ => None,
        }
    }

    // The type of an expression read as a value. A reference read through
    // a shared path is a shared reference (`[T-Read-Through-Shared]`).
    fn value_ty(&self, e: &Expr) -> Option<Ty> {
        match e {
            Expr::Int(_) => Some(Ty::I32),
            Expr::Var(_) | Expr::Deref(_) | Expr::Field(_) | Expr::Index(..) => {
                let (t, m) = self.place_ty(e, Use::Read)?;
                match t {
                    Ty::Ref(inner, Mode::Excl) if m == Mode::Shared => Some(Ty::Ref(inner, Mode::Shared)),
                    t => Some(t),
                }
            }
            Expr::Borrow(m, a) => Some(Ty::Ref(Box::new(self.place_ty(a, Use::Borrow(*m))?.0), *m)),
            Expr::Add(..) => Some(Ty::I32),
            Expr::Gt(..) => Some(Ty::Bool),
            Expr::Call(f, _) => match f.as_str() {
                "Mutex::new" => Some(Ty::Mutex),
                "lock" => Some(Ty::Guard),
                "Atomic::new" => Some(Ty::Atomic),
                "Atomic::load" | "Atomic::fetch_add" => Some(Ty::I32),
                "Atomic::store" | "Atomic::wait" | "Atomic::notify_all" => Some(Ty::Unit),
                "Vec::new" => None, // fixed by the declaration
                "Vec::push" => Some(Ty::Unit),
                "Vec::pop" => Some(Ty::Opt(Box::new(self.vec_elem(e)?))),
                "Vec::index_exclusive" => Some(Ty::Ref(Box::new(self.vec_elem(e)?), Mode::Excl)),
                "Channel::new" => None, // fixed by the declaration
                "Channel::send" => Some(Ty::Res(Box::new(self.chan_elem(e)?))),
                "Channel::recv" => Some(Ty::Opt(Box::new(self.chan_elem(e)?))),
                "Channel::close" => Some(Ty::Unit),
                "swap" => Some(Ty::Unit),
                "replace" => Some(Ty::I32),
                _ => Some(self.prog.func(f)?.ret.clone()),
            },
            Expr::Spawn(f, _) => Some(Ty::Handle(Box::new(self.prog.func(f)?.ret.clone()))),
            Expr::Slice(m, ..) => Some(Ty::Slice(*m)),
            Expr::Closure(c) => Some(Ty::Fn(Box::new(c.ret.clone()))),
            Expr::CallVal(a) => match self.place_ty(a, Use::Read)?.0 {
                Ty::Fn(r) => Some(*r),
                _ => None,
            },
            Expr::SpawnClo(a) => match self.value_ty(a)? {
                Ty::Fn(r) => Some(Ty::Handle(r)),
                _ => None,
            },
            Expr::JobLit(_) => Some(Ty::Job),
            Expr::ArrLit(_) => Some(Ty::Arr),
            Expr::Join(t) => match self.ty_of(t)? {
                Ty::Handle(r) => Some(*r),
                _ => None,
            },
            Expr::HoldLit(_) => Some(Ty::Hold),
        }
    }

    // The element type of a `Vec::…(&_m v, …)` call's vector.
    fn vec_elem(&self, e: &Expr) -> Option<Ty> {
        let Expr::Call(_, args) = e else { return None };
        match self.value_ty(args.first()?)? {
            Ty::Ref(t, _) => match *t {
                Ty::Vec => Some(Ty::I32),
                Ty::VecR => Some(Ty::refi(Mode::Excl)),
                Ty::VecH => Some(Ty::Handle(Box::new(Ty::Unit))),
                _ => None,
            },
            _ => None,
        }
    }

    // T of a `Channel::…(&c, …)` call: c's type is Channel<T>.
    fn chan_elem(&self, e: &Expr) -> Option<Ty> {
        let Expr::Call(_, args) = e else { return None };
        match self.value_ty(args.first()?)? {
            Ty::Ref(t, _) => match *t {
                Ty::Chan(x) => Some(*x),
                _ => None,
            },
            _ => None,
        }
    }

    // The flow analysis' place for a syntactic place, when it has one.
    fn splace(&self, e: &Expr) -> Option<SPlace> {
        let mut names = Vec::new();
        expr_names(e, &mut names);
        if names.iter().any(|n| self.caps.contains(n)) {
            return None; // a capture's place: not tracked
        }
        match e {
            Expr::Var(n) => Some(SPlace { root: Root::Var(n.clone()), steps: vec![] }),
            // An array's element is a place, at its literal index.
            Expr::Index(v, k) if self.ty_of(v) == Some(Ty::Arr) => Some(SPlace { root: Root::Var(v.clone()), steps: vec![StepK::Idx(*k)] }),
            // A `Vec` element is not a place of the analysis (D-0219 finding
            // 10, the implementations' reading): its accesses are checked
            // where they run; a borrow of one counts as a borrow of the vector
            // (`deriv_place`).
            Expr::Index(..) => None,
            Expr::Field(a) => {
                // h.r for a Hold binding; s.r = (*s).r for a tracked reference s.
                if let Expr::Var(n) = &**a {
                    return match self.ty_of(n)? {
                        Ty::Hold => Some(SPlace { root: Root::Var(n.clone()), steps: vec![StepK::Field] }),
                        Ty::Ref(..) => Some(SPlace { root: Root::Deref(n.clone()), steps: vec![StepK::Field] }),
                        _ => None,
                    };
                }
                if let Expr::Deref(b) = &**a {
                    if let Expr::Var(q) = &**b {
                        return Some(SPlace { root: Root::Deref(q.clone()), steps: vec![StepK::Field] });
                    }
                }
                None
            }
            Expr::Deref(a) => match &**a {
                Expr::Var(q) if matches!(self.ty_of(q), Some(Ty::Ref(..)) | Some(Ty::Guard) | Some(Ty::Slice(_))) => Some(SPlace { root: Root::Deref(q.clone()), steps: vec![] }),
                _ => None, // `*r` for r not a tracked binding: neither T nor F
            },
            _ => None,
        }
    }

    // The place a borrow derives from: `v[k]` of a `Vec` counts as `v` itself.
    fn deriv_place(&self, e: &Expr) -> Option<SPlace> {
        match e {
            Expr::Index(v, _) if self.caps.contains(v) => None,
            Expr::Index(v, _) if matches!(self.ty_of(v), Some(Ty::Slice(_))) => Some(SPlace { root: Root::Deref(v.clone()), steps: vec![] }),
            Expr::Index(v, _) if self.ty_of(v) == Some(Ty::Arr) => self.splace(e),
            Expr::Index(v, _) => Some(SPlace { root: Root::Var(v.clone()), steps: vec![] }),
            _ => self.splace(e),
        }
    }

    // A slice's place: `v.[lo, hi)` of a `Vec` binding, `*s.[lo, hi)` of a
    // slice binding (its positions in that slice; such places are
    // compared only with other places rooted at `*s`).
    fn slice_place(&self, v: &str, lo: i64, hi: i64) -> Option<SPlace> {
        if self.caps.contains(v) {
            return None;
        }
        let root = match self.ty_of(v)? {
            Ty::Slice(_) => Root::Deref(v.to_string()),
            _ => Root::Var(v.to_string()),
        };
        Some(SPlace { root, steps: vec![StepK::Range(lo, hi)] })
    }

    // An access whose mode the analysis does not know (a call of a
    // closure it did not see formed: shared, or exclusive if the closure
    // writes itself, D-0239): refuted only as a shared access would be,
    // proven only as an exclusive one would be.
    fn check_access_unknown(&mut self, fl: &Flow, p: &SPlace, line: usize) {
        let before = self.errors.len();
        self.check_access(fl, p, Mode::Excl, line);
        self.errors.truncate(before);
        self.check_access_refute(fl, p, Mode::Shared, line);
    }

    fn check_access_refute(&mut self, fl: &Flow, p: &SPlace, m: Mode, line: usize) {
        for ((h, q, m2), v) in &fl.deriv {
            if *v == Tri::T && q.root == p.root && overlap(&q.steps, &p.steps) && !(m == Mode::Shared && *m2 == Mode::Shared) {
                let why = format!("`{}` holds a {} borrow of this place", h, m2.word());
                self.err(line, "diag.aliasing-conflict", why);
                return;
            }
        }
    }

    // The discharge table's `¬clash` row for an access at `p` in mode `m`:
    // refuted when some overlapping, non-permitted deriv fact is T.
    fn check_access(&mut self, fl: &Flow, p: &SPlace, m: Mode, line: usize) {
        // The *proven* outcome (D-0241): escaped(x) = F and every
        // overlapping, non-permitted deriv fact F. Recorded per statement
        // and binding, false if any access there is not proven.
        if let Root::Var(x) = &p.root {
            let proven = fl.escaped.get(&p.root).copied().unwrap_or(Tri::F) == Tri::F
                && fl.deriv.iter().all(|((_, q, m2), v)| *v == Tri::F || q.root != p.root || !overlap(&q.steps, &p.steps) || (m == Mode::Shared && *m2 == Mode::Shared));
            let e = self.proven.entry((self.fname.clone(), line, x.clone())).or_insert(true);
            *e = *e && proven;
        }
        for ((h, q, m2), v) in &fl.deriv {
            if *v == Tri::T && q.root == p.root && overlap(&q.steps, &p.steps) && !(m == Mode::Shared && *m2 == Mode::Shared) {
                let why = format!("`{}` holds a {} borrow of this place", h, m2.word());
                self.err(line, "diag.aliasing-conflict", why);
                return;
            }
        }
    }

    // A use of binding `n` (`[Binding-Lookup]`): refuted when valid(n) is F,
    // and (D-0087) when it is `?`.
    fn use_binding(&mut self, fl: &Flow, n: &str, line: usize) {
        match fl.valid.get(n) {
            Some(Tri::T) => {}
            _ => self.err(line, "diag.stale-binding", format!("`{}` may have been moved or ended", n)),
        }
    }

    // Walks a place expression: binding uses, the reads of references it
    // goes through, and the access itself.
    fn place(&mut self, fl: &mut Flow, e: &Expr, u: Use, line: usize) {
        match e {
            Expr::Var(n) => self.use_binding(fl, n, line),
            // `s[k]` reads the slice binding `s`, then reaches the element
            // through its borrow (`[Slice-Index]`).
            Expr::Index(v, _) if matches!(self.ty_of(v), Some(Ty::Slice(_))) => self.place(fl, &Expr::Var(v.clone()), Use::Read, line),
            // `a[k]` with a literal `k` outside the array: rejected
            // (`[Index-Checked]`'s bound, static for a literal).
            Expr::Index(v, k) if self.ty_of(v) == Some(Ty::Arr) => {
                self.use_binding(fl, v, line);
                if *k < 0 || *k >= 2 {
                    self.err(line, "diag.index-out-of-bounds", "a literal index outside the array".into());
                }
            }
            // `v[k]` is not a call (D-0238): an element access, which the
            // flow analysis does not check (D-0221); the binding is used.
            // D-0282 checks it against the frame-held references.
            Expr::Index(v, k) => {
                self.use_binding(fl, v, line);
                self.element_access(fl, e, v, *k, u, line);
            }
            Expr::Deref(a) => self.rvalue(fl, a, line),
            Expr::Field(a) => {
                if matches!(self.place_ty(a, Use::Read), Some((Ty::Hold, _))) && !matches!(&**a, Expr::Deref(_)) {
                    self.place(fl, a, Use::Read, line);
                } else {
                    // auto-deref: the reference is read
                    match &**a {
                        Expr::Deref(b) => self.rvalue(fl, b, line),
                        _ => self.rvalue(fl, a, line),
                    }
                }
            }
            _ => {}
        }
        let m = match u {
            Use::Read => Mode::Shared,
            Use::Write => Mode::Excl,
            Use::Borrow(m) => m,
        };
        if let Some(p) = self.splace(e) {
            self.check_access(fl, &p, m, line);
        }
    }

    // An expression evaluated for its value, in evaluation order; `holder`
    // is the binding a `let`/assignment stores it in (for the deriv lines).
    fn rvalue(&mut self, fl: &mut Flow, e: &Expr, line: usize) {
        self.rvalue_into(fl, e, line, None);
    }

    fn rvalue_into(&mut self, fl: &mut Flow, e: &Expr, line: usize, holder: Option<&str>) {
        match e {
            Expr::Int(_) => {}
            // D-0258 `[Static-Use]`: a static is never read whole.
            Expr::Var(n) if self.is_static_var(e) => self.err(line, "diag.read-of-resource", format!("`{}` is a static: it is borrowed (`&{}`), never read or moved whole", n, n)),
            Expr::Var(_) | Expr::Deref(_) | Expr::Field(_) | Expr::Index(..) => self.place(fl, e, Use::Read, line),
            Expr::Add(a, b) | Expr::Gt(a, b) => {
                self.rvalue(fl, a, line);
                self.rvalue(fl, b, line);
            }
            // D-0258 `[Ref-Form-Static]`: `&S` is a shared reference with
            // program extent, nothing to track; `&mut S` is refused.
            Expr::Borrow(m, a) if self.is_static_var(a) => {
                if *m == Mode::Excl {
                    let Expr::Var(n) = &**a else { unreachable!() };
                    self.err(line, "diag.write-through-shared", format!("`{}` is a static, shared by every thread: borrow it `&{}`", n, n));
                }
            }
            Expr::Borrow(m, a) => {
                self.check_borrow_mode(*m, a, line);
                self.place(fl, a, Use::Borrow(*m), line);
                let p = self.deriv_place(a);
                match (holder, p) {
                    (Some(h), Some(p)) => {
                        // `auto r = &_m x.π;` / `r = &_m x.π`
                        fl.forget_holder(h);
                        fl.deriv.insert((h.to_string(), p, *m), Tri::T);
                    }
                    (_, Some(p)) => {
                        fl.escaped.insert(p.root, Tri::T);
                    }
                    _ => {}
                }
            }
            Expr::Call(f, args) => {
                let params: Vec<Ty> = match f.as_str() {
                    "Mutex::new" => vec![Ty::I32],
                    "lock" => vec![Ty::Ref(Box::new(Ty::Mutex), Mode::Shared)],
                    // D-0249, D-0253: every operation takes the atomic by
                    // shared reference.
                    "Atomic::new" => vec![Ty::I32],
                    "Atomic::load" | "Atomic::notify_all" => vec![Ty::Ref(Box::new(Ty::Atomic), Mode::Shared)],
                    "Atomic::store" | "Atomic::fetch_add" | "Atomic::wait" => vec![Ty::Ref(Box::new(Ty::Atomic), Mode::Shared), Ty::I32],
                    "Vec::new" => vec![],
                    "Vec::push" | "Vec::pop" | "Vec::index_exclusive" => {
                        let et = self.vec_elem(e).unwrap_or(Ty::I32);
                        let vt = match &et {
                            Ty::I32 => Ty::Vec,
                            Ty::Handle(_) => Ty::VecH,
                            _ => Ty::VecR,
                        };
                        let mut v = vec![Ty::Ref(Box::new(vt), Mode::Excl)];
                        if f == "Vec::push" {
                            v.push(et);
                        }
                        if f == "Vec::index_exclusive" {
                            v.push(Ty::I32);
                        }
                        v
                    }
                    "Channel::new" => vec![Ty::I32],
                    // `std::swap`, `std::replace` at T = i32 (D-0235)
                    "swap" => vec![Ty::refi(Mode::Excl), Ty::refi(Mode::Excl)],
                    "replace" => vec![Ty::refi(Mode::Excl), Ty::I32],
                    "Channel::send" | "Channel::recv" | "Channel::close" => {
                        let t = self.chan_elem(e).unwrap_or(Ty::Unit);
                        let mut v = vec![Ty::Ref(Box::new(Ty::Chan(Box::new(t.clone()))), Mode::Shared)];
                        if f == "Channel::send" {
                            v.push(t);
                        }
                        v
                    }
                    _ => match self.prog.func(f) {
                        Some(func) => func.params.iter().map(|p| p.0.clone()).collect(),
                        None => {
                            self.err(line, "diag.unbound-name", format!("no function `{}`", f));
                            return;
                        }
                    },
                };
                // The D-0011 elided call: `auto r = f(&_m x…)` with a
                // reference result — as `auto r = &_m x`.
                let elided = holder.is_some()
                    && matches!(self.prog.func(f).map(|x| &x.ret), Some(Ty::Ref(..)))
                    && args.first().map_or(false, |a| matches!(a, Expr::Borrow(..)));
                // D-0289, CHG-0384: `Vec::push(&mut hs, spawn(…))` into a `Vec`
                // of handles of this frame binds the spawn: `hs` holds its
                // facts besides what it held.
                let pool = match (f.as_str(), args.first(), args.get(1)) {
                    ("Vec::push", Some(Expr::Borrow(Mode::Excl, v)), Some(Expr::Spawn(..) | Expr::SpawnClo(_))) => match &**v {
                        Expr::Var(hs) if matches!(self.ty_of(hs), Some(Ty::VecH)) => Some(hs.clone()),
                        _ => None,
                    },
                    _ => None,
                };
                // And a handle binding holding facts pushed there gives them
                // to the `Vec` as it moves in.
                let pushed: Option<(String, Vec<(SPlace, Mode, Tri)>)> = match (f.as_str(), args.first(), args.get(1)) {
                    ("Vec::push", Some(Expr::Borrow(Mode::Excl, v)), Some(Expr::Var(t))) if matches!(self.ty_of(t), Some(Ty::Handle(_))) => match &**v {
                        Expr::Var(hs) if matches!(self.ty_of(hs), Some(Ty::VecH)) => Some((hs.clone(), fl.holder_facts(t))),
                        _ => None,
                    },
                    _ => None,
                };
                for (i, a) in args.iter().enumerate() {
                    if i == 0 && elided {
                        self.rvalue_into(fl, a, line, holder);
                    } else if let (1, Some(hs)) = (i, &pool) {
                        let saved = std::mem::replace(&mut self.accumulate, true);
                        self.rvalue_into(fl, a, line, Some(hs));
                        self.accumulate = saved;
                    } else {
                        self.rvalue(fl, a, line);
                    }
                    if let Some(pt) = params.get(i) {
                        self.check_arg(pt, a, line);
                        self.consume_resource(fl, pt, a, line);
                        if pt.is_resource() {
                            self.move_out(a, line);
                        }
                    }
                }
                self.call_site(fl, e, f, args, line, holder);
                if let Some((hs, facts)) = pushed {
                    for (sp, m, v) in facts {
                        fl.deriv.insert((hs.clone(), sp, m), v);
                    }
                }
            }
            Expr::Spawn(f, args) => {
                let Some(func) = self.prog.func(f) else {
                    self.err(line, "diag.unbound-name", format!("no function `{}`", f));
                    return;
                };
                let params: Vec<Ty> = func.params.iter().map(|p| p.0.clone()).collect();
                let mut facts = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    let pm = match params.get(i) {
                        Some(Ty::Ref(_, m)) => Some(*m),
                        _ => None,
                    };
                    match a {
                        Expr::Borrow(m, p) => {
                            // `spawn(…, &_m x …)`: escaped(x) := T; and the
                            // handle derives the place (D-0071).
                            self.check_borrow_mode(*m, p, line);
                            self.place(fl, p, Use::Borrow(*m), line);
                            if let Some(sp) = self.deriv_place(p) {
                                // D-0289: a handle bound whole holds the
                                // lend; only an unbound one escapes it.
                                if holder.is_none() {
                                    fl.escaped.insert(sp.root.clone(), Tri::T);
                                }
                                facts.push((sp, *m));
                            }
                        }
                        Expr::Slice(m, v, lo, hi) => {
                            // A slice, as a visible borrow of its range.
                            self.rvalue(fl, a, line);
                            if let Some(sp) = self.slice_place(v, *lo, *hi) {
                                facts.push((sp, *m));
                            }
                        }
                        // A slice binding passed whole is lent as a value: the
                        // row names a binding of reference type (D-0247).
                        Expr::Var(r) if matches!(self.ty_of(r), Some(Ty::Ref(..))) => {
                            // D-0213: the lend is a borrow of *r in the
                            // parameter's mode.
                            self.use_binding(fl, r, line);
                            let m = pm.unwrap_or(Mode::Shared);
                            let sp = SPlace { root: Root::Deref(r.clone()), steps: vec![] };
                            self.check_access(fl, &sp, m, line);
                            facts.push((sp, m));
                        }
                        _ => self.rvalue(fl, a, line),
                    }
                    if let Some(pt) = params.get(i) {
                        self.check_arg(pt, a, line);
                        self.consume_resource(fl, pt, a, line);
                        if pt.is_resource() {
                            self.move_out(a, line);
                        }
                    }
                }
                if let Some(h) = holder {
                    if !self.accumulate {
                        fl.forget_holder(h);
                    }
                    for (sp, m) in facts {
                        fl.deriv.insert((h.to_string(), sp, m), Tri::T);
                    }
                }
            }
            Expr::Join(t) => {
                // A resource binding consumed as an argument.
                self.use_binding(fl, t, line);
                fl.valid.insert(t.clone(), Tri::F);
                fl.clear_holder(t);
            }
            Expr::HoldLit(a) => {
                self.rvalue(fl, a, line);
                self.check_arg(&Ty::refi(Mode::Excl), a, line);
            }
            Expr::ArrLit(xs) => {
                for x in xs {
                    self.rvalue(fl, x, line);
                    self.check_arg(&Ty::I32, x, line);
                }
            }
            Expr::JobLit(a) => {
                // A resource-typed whole binding consumed as a field initializer.
                let ht = Ty::Handle(Box::new(Ty::Unit));
                self.rvalue(fl, a, line);
                self.check_arg(&ht, a, line);
                self.consume_resource(fl, &ht, a, line);
                self.move_out(a, line);
            }
            Expr::Slice(m, v, lo, hi) => {
                // `[Slice-Form]`: `&_m v[lo .. hi]` borrows v's range; through
                // a slice binding, its range of the slice's source.
                if let Some(Ty::Slice(sm)) = self.ty_of(v) {
                    if *m == Mode::Excl && sm == Mode::Shared {
                        self.err(line, "diag.borrow-exceeds-source", "an exclusive slice of a shared one".into());
                    }
                    self.place(fl, &Expr::Var(v.clone()), Use::Read, line);
                } else {
                    self.use_binding(fl, v, line);
                }
                if self.ty_of(v) == Some(Ty::Arr) && !(0 <= *lo && lo <= hi && *hi <= 2) {
                    self.err(line, "diag.index-out-of-bounds", "a literal range outside the array".into());
                }
                let p = self.slice_place(v, *lo, *hi);
                if let Some(p) = &p {
                    self.check_access(fl, p, *m, line);
                }
                match (holder, p) {
                    (Some(h), Some(p)) => {
                        fl.forget_holder(h);
                        fl.deriv.insert((h.to_string(), p, *m), Tri::T);
                    }
                    (_, Some(p)) => {
                        fl.escaped.insert(p.root, Tri::T);
                    }
                    _ => {}
                }
            }
            Expr::Closure(c) => self.closure_lit(fl, c, line, holder),
            Expr::CallVal(a) => {
                // `[Closure-Call]`: the closure's place is borrowed, shared,
                // or exclusive for one that writes itself (D-0239).
                match self.place_ty(a, Use::Read) {
                    Some((Ty::Fn(_), _)) => {}
                    Some((t, _)) => self.err(line, "diag.type-mismatch", format!("a call of {}", t.text())),
                    None => {}
                }
                match &**a {
                    Expr::Var(n) => {
                        self.use_binding(fl, n, line);
                        if let Some(p) = self.splace(a) {
                            match self.clo_info.get(n).copied() {
                                Some((_, ws)) => self.check_access(fl, &p, if ws { Mode::Excl } else { Mode::Shared }, line),
                                None => self.check_access_unknown(fl, &p, line),
                            }
                        }
                    }
                    other => {
                        self.place(fl, other, Use::Read, line);
                    }
                }
            }
            Expr::SpawnClo(a) => {
                match &**a {
                    Expr::Closure(c) if !c.mv => self.err(line, "diag.spawn-borrow-closure", "a closure passed to spawn must be a move closure".into()),
                    Expr::Var(n) if matches!(self.clo_info.get(n), Some((false, _))) => self.err(line, "diag.spawn-borrow-closure", "a closure passed to spawn must be a move closure".into()),
                    _ => {}
                }
                // D-0289: the handle holds what the closure row gives the
                // closure it runs: a literal's facts, a binding's copied.
                let copied = match (&**a, holder) {
                    (Expr::Var(n), Some(_)) => fl.deriv_facts(n),
                    _ => Vec::new(),
                };
                match (&**a, holder) {
                    (Expr::Closure(c), Some(h)) => self.closure_lit(fl, c, line, Some(h)),
                    _ => self.rvalue(fl, a, line),
                }
                // A closure that owns a resource moves into the thread.
                self.consume_resource(fl, &Ty::Unit, a, line);
                if let (Some(h), false) = (holder, matches!(&**a, Expr::Closure(_))) {
                    if !self.accumulate {
                        fl.forget_holder(h);
                    }
                    for (p, m, v) in copied {
                        fl.deriv.insert((h.to_string(), p, m), v);
                    }
                }
            }
        }
    }

    // A closure literal (`rule.fn.closure`, `spec/14` §6's closure row).
    fn closure_lit(&mut self, fl: &mut Flow, c: &Clo, line: usize, holder: Option<&str>) {
        let free = free_names(c, self.prog);
        let listed: BTreeSet<String> = c.caps.iter().cloned().collect();
        if free != listed {
            self.err(line, "diag.capture-list-mismatch", "the capture list is not the body's free variables".into());
        }
        let mut facts = Vec::new();
        // D-0288: what the captures reach, for a closure with or without
        // `move`: `*r` for a captured reference or slice, and the facts of
        // another capture that may hold a reference.
        let mut reach: Vec<(SPlace, Mode, Tri)> = Vec::new();
        for x in &c.caps {
            let Some(t) = self.ty_of(x) else {
                self.err(line, "diag.unbound-name", format!("no binding `{}`", x));
                continue;
            };
            let m = if writes_root(&c.body, x) { Mode::Excl } else { Mode::Shared };
            let xe = Expr::Var(x.clone());
            match &t {
                Ty::Ref(_, rm) | Ty::Slice(rm) => reach.push((SPlace { root: Root::Deref(x.clone()), steps: vec![] }, *rm, Tri::T)),
                _ if may_hold_ref(&t) => reach.extend(fl.deriv_facts(x)),
                _ => {}
            }
            if c.mv {
                // `[Closure-Form-Move]`: the capture's value is stored (read).
                self.rvalue(fl, &xe, line);
                if t.is_resource() {
                    self.consume_resource(fl, &t, &xe, line);
                }
            } else {
                // `[Closure-Form-Borrow]`: `&_m x` stored in a field.
                self.use_binding(fl, x, line);
                if let Some(p) = self.splace(&xe) {
                    self.check_access(fl, &p, m, line);
                    if !matches!(t, Ty::Ref(..)) {
                        facts.push((p, m));
                    } else if !PRE_D0243.load(std::sync::atomic::Ordering::Relaxed) {
                        // D-0243: the closure holds a borrow of the reference
                        // binding itself; since D-0288 a fact describes it.
                        fl.escaped.insert(p.root.clone(), Tri::T);
                        reach.push((p, m, Tri::T));
                    }
                }
            }
        }
        match holder {
            Some(h) => {
                if !self.accumulate {
                    fl.forget_holder(h);
                }
                for (p, m) in facts {
                    fl.deriv.insert((h.to_string(), p, m), Tri::T);
                }
                for (p, m, v) in reach {
                    fl.deriv.insert((h.to_string(), p, m), v);
                }
            }
            // `&x…` in any other position.
            None => {
                for (p, _) in facts {
                    fl.escaped.insert(p.root, Tri::T);
                }
            }
        }
        self.analyse_closure(c);
    }

    // D-0288's `fn` value row: a copy of a binding holds its facts; a call's
    // result may hold what any argument that may hold a reference formed or
    // held, which the run decides (`?`).
    fn fn_value_facts(&self, fl: &Flow, e: &Expr) -> Vec<(SPlace, Mode, Tri)> {
        match e {
            Expr::Var(x) => fl.deriv_facts(x),
            Expr::Call(_, args) => {
                let mut out = Vec::new();
                for a in args {
                    match a {
                        Expr::Borrow(m, p) => {
                            if let Some(sp) = self.deriv_place(p) {
                                out.push((sp, *m, Tri::Q));
                            }
                        }
                        Expr::Var(y) if self.ty_of(y).map_or(false, |t| may_hold_ref(&t)) => {
                            if let Some(Ty::Ref(_, rm) | Ty::Slice(rm)) = self.ty_of(y) {
                                out.push((SPlace { root: Root::Deref(y.clone()), steps: vec![] }, rm, Tri::Q));
                            }
                            out.extend(fl.deriv_facts(y).into_iter().map(|(p, m, _)| (p, m, Tri::Q)));
                        }
                        _ => {}
                    }
                }
                out
            }
            _ => Vec::new(),
        }
    }

    // A closure's body, analysed as a body of its own: the captures bound
    // with their types, untracked.
    fn analyse_closure(&mut self, c: &Clo) {
        let mut parents = HashMap::new();
        let mut next = 0;
        number_blocks(&c.body, None, &mut next, &mut parents);
        let pos = Positions::of(&c.caps, &c.body);
        let mut cx = Cx::new(self.prog, self.sums, pos, format!("$clo{}", c.id), parents, c.ret.clone());
        let mut fl = Flow::default();
        for x in &c.caps {
            let Some(t) = self.ty_of(x) else { continue };
            if let Ty::Ref(..) = t {
                cx.referent.insert(x.clone(), None);
            }
            let id = cx.binds.len();
            cx.binds.entry(x.clone()).or_default().push(Binding { ty: t, block: 0, id });
            cx.regions[0].insert(x.clone());
            cx.caps.insert(x.clone());
            fl.valid.insert(x.clone(), Tri::T);
        }
        cx.block(&mut fl, &c.body, None);
        self.errors.extend(cx.errors);
        self.ends.extend(cx.ends);
        self.proven.extend(cx.proven);
        self.sub_types.extend(cx.sub_types);
        self.sub_types.insert(format!("$clo{}", c.id), cx.all_types);
    }

    // "resource-typed whole binding x consumed … as an argument": valid(x)
    // := F and every deriv(·, x, ·) := F; `solitary(a_x)` is refuted when
    // some deriv(·, x.·, ·) is T (`diag.move-while-aliased`).
    fn consume_resource(&mut self, fl: &mut Flow, pt: &Ty, a: &Expr, line: usize) {
        let Expr::Var(x) = a else { return };
        if !pt.is_resource() && !self.clo_owns.contains(x) {
            return;
        }
        let root = Root::Var(x.clone());
        if fl.deriv.iter().any(|(k, v)| *v == Tri::T && k.1.root == root) {
            self.err(line, "diag.move-while-aliased", format!("`{}` is moved while borrowed", x));
        }
        for (k, v) in fl.deriv.iter_mut().chain(fl.inh.iter_mut()) {
            if k.1.root == root {
                *v = Tri::F;
            }
        }
        // D-0244: a consumed binding that held borrows (a handle moved into
        // a `Vec` or a struct) moves them on with it: its own facts are
        // forgotten, as for a moved reference.
        fl.forget_holder(x);
        fl.valid.insert(x.clone(), Tri::F);
    }

    fn check_borrow_mode(&mut self, m: Mode, place: &Expr, line: usize) {
        if m == Mode::Excl {
            if let Some((_, Mode::Shared)) = self.place_ty(place, Use::Borrow(m)) {
                self.err(line, "diag.borrow-exceeds-source", "an exclusive borrow through a shared path".into());
            }
        }
    }

    // An argument against its parameter type (`[Call]`, `rule.fn.bind-param`).
    fn check_arg(&mut self, pt: &Ty, a: &Expr, line: usize) {
        let Some(at) = self.value_ty(a) else { return };
        match (pt, &at) {
            (Ty::Ref(t1, Mode::Excl), Ty::Ref(t2, Mode::Shared)) if t1 == t2 => {
                // A reference read through a shared path where an exclusive
                // one is expected is `diag.borrow-exceeds-source` (D-0218);
                // a shared one otherwise is a type mismatch.
                let through = matches!(a, Expr::Var(_) | Expr::Deref(_) | Expr::Field(_)) && self.place_ty(a, Use::Read).map_or(false, |(t, _)| t == Ty::refi(Mode::Excl));
                if through {
                    self.err(line, "diag.borrow-exceeds-source", "an exclusive reference read through a shared one".into());
                } else {
                    self.err(line, "diag.type-mismatch", format!("{} where {} is expected", at.text(), pt.text()));
                }
            }
            (Ty::Ref(t1, Mode::Shared), Ty::Ref(t2, _)) if t1 == t2 => {}
            _ if *pt == at => {}
            _ => self.err(line, "diag.type-mismatch", format!("{} where {} is expected", at.text(), pt.text())),
        }
    }

    fn stmt(&mut self, fl: &mut Flow, s: &Stmt, block: usize) {
        let line = s.line;
        match &s.kind {
            StmtKind::Let { name, ty, init } => {
                if self.regions.last().map_or(false, |r| r.contains(name)) {
                    self.err(line, "diag.duplicate-local", format!("`{}` is declared twice in one block", name));
                }
                self.rvalue_into(fl, init, line, Some(name));
                self.move_out(init, line);
                let it = self.value_ty(init);
                let t = match (ty, it) {
                    (Some(t), Some(it)) => {
                        let ok = *t == it || matches!((t, &it), (Ty::Ref(a, Mode::Shared), Ty::Ref(b, _)) if a == b) || matches!((t, &it), (Ty::Slice(Mode::Shared), Ty::Slice(_)));
                        if !ok {
                            self.err(line, "diag.type-mismatch", format!("`{}` is declared {} but its initializer is {}", name, t.text(), it.text()));
                        }
                        t.clone()
                    }
                    (Some(t), None) => t.clone(),
                    (None, Some(it)) => it,
                    (None, None) => Ty::Unit,
                };
                if let Ty::Ref(..) | Ty::Slice(_) = t {
                    let r = self.referent_of_value(init);
                    self.referent.insert(name.clone(), r);
                }
                // "as a local-declaration initializer (`auto y = x`)": a
                // resource-typed whole binding is consumed.
                if let Expr::Var(x) = init {
                    let xt = self.ty_of(x).unwrap_or(Ty::Unit);
                    self.consume_resource(fl, &xt, init, line);
                }
                self.clo_owns.remove(name);
                match init {
                    Expr::Closure(c) => {
                        self.clo_info.insert(name.clone(), (c.mv, c.mv && c.caps.iter().any(|x| writes_root(&c.body, x))));
                        if c.mv && c.caps.iter().any(|x| self.ty_of(x).map_or(false, |t| t.is_resource()) || self.clo_owns.contains(x)) {
                            self.clo_owns.insert(name.clone());
                        }
                    }
                    _ => {
                        self.clo_info.remove(name);
                    }
                }
                // A reference copied from anything but a visible borrow: the
                // referents the analysis cannot exclude escape.
                if matches!(t, Ty::Ref(..) | Ty::Slice(_)) && !matches!(init, Expr::Borrow(..) | Expr::Slice(..)) && !self.is_elided(init) {
                    self.escape_copied(fl, init);
                }
                let id = self.pos.decl.get(&(s as *const Stmt)).copied().unwrap_or(usize::MAX);
                if self.d0282() {
                    let inh = self.inherit(fl, init);
                    let end = self.pos.stmt_end.get(&(s as *const Stmt)).copied().unwrap_or(0);
                    self.bind_frame_held(fl, name, &t, init, inh, id, end, line);
                }
                if let (Ty::Fn(_), false) = (&t, matches!(init, Expr::Closure(_))) {
                    for (p, m, v) in self.fn_value_facts(fl, init) {
                        fl.deriv.insert((name.clone(), p, m), v);
                    }
                }
                self.binds.entry(name.clone()).or_default().push(Binding { ty: t, block, id });
                if let Some(r) = self.regions.last_mut() {
                    r.insert(name.clone());
                }
                fl.valid.insert(name.clone(), Tri::T);
            }
            StmtKind::Assign { place, value } => {
                // target place first, then value (`spec/13` §1)
                let pty = self.place_ty(place, Use::Write);
                if let Some((_, Mode::Shared)) = pty {
                    self.err(line, "diag.write-through-shared", "a write through a shared path".into());
                }
                let whole = match place {
                    Expr::Var(n) => Some(n.clone()),
                    _ => None,
                };
                if whole.is_none() {
                    self.place(fl, place, Use::Write, line);
                }
                self.rvalue_into(fl, value, line, whole.as_deref());
                self.move_out(value, line);
                let inh = if self.d0282() && whole.is_some() { Some(self.inherit(fl, value)) } else { None };
                if let (Some((pt, _)), Some(vt)) = (&pty, self.value_ty(value)) {
                    let ok = *pt == vt || matches!((pt, &vt), (Ty::Ref(a, Mode::Shared), Ty::Ref(b, _)) if a == b) || matches!((pt, &vt), (Ty::Slice(Mode::Shared), Ty::Slice(_)));
                    if !ok {
                        self.err(line, "diag.type-mismatch", format!("{} assigned to a place of type {}", vt.text(), pt.text()));
                    }
                }
                if let Some(n) = whole {
                    // `x = e`: a whole-object write (`spec/11`).
                    if let Some(p) = self.splace(place) {
                        self.check_access(fl, &p, Mode::Excl, line);
                    }
                    if let Some(Ty::Fn(_)) = self.ty_of(&n) {
                        if let Expr::Closure(c) = value {
                            // `rule.temporal.ref-escape`: a closure without
                            // `move` is an aggregate of borrows of its
                            // captures (CHG-0113).
                            if !c.mv {
                                for x in &c.caps {
                                    if let (Some(rb), Some(b)) = (self.referent_block(&Expr::Var(x.clone())), self.block_of(&n)) {
                                        if rb != b && self.encloses(b, rb) {
                                            self.err(line, "diag.reference-escapes-scope", format!("`{}` outlives `{}`, which it borrows", n, x));
                                        }
                                    }
                                }
                            }
                            self.clo_info.insert(n.clone(), (c.mv, c.mv && c.caps.iter().any(|x| writes_root(&c.body, x))));
                        } else {
                            self.clo_info.remove(&n);
                            let facts = self.fn_value_facts(fl, value);
                            fl.forget_holder(&n);
                            for (p, m, v) in facts {
                                fl.deriv.insert((n.clone(), p, m), v);
                            }
                        }
                    }
                    if matches!(self.ty_of(&n), Some(Ty::Ref(..)) | Some(Ty::Slice(_))) {
                        // `r = e`: a reference reassigned; what derived from *r
                        // before is gone.
                        fl.clear_referent(&n);
                        match value {
                            Expr::Borrow(..) | Expr::Slice(..) => {
                                let rb = self.referent_of_value(value);
                                if let (Some(rb), Some(b)) = (rb, self.block_of(&n)) {
                                    if rb != b && self.encloses(b, rb) {
                                        self.err(line, "diag.reference-escapes-scope", format!("`{}` outlives what it would refer to", n));
                                    }
                                }
                            }
                            _ if self.is_elided(value) => {}
                            _ => {
                                fl.forget_holder(&n);
                                self.escape_copied(fl, value);
                            }
                        }
                    }
                    if let (Some(inh), Some(t)) = (inh, self.ty_of(&n)) {
                        let id = self.binds.get(&n).and_then(|v| v.last()).map_or(usize::MAX, |b| b.id);
                        let end = self.pos.stmt_end.get(&(s as *const Stmt)).copied().unwrap_or(0);
                        self.bind_frame_held(fl, &n, &t, value, inh, id, end, line);
                    }
                    fl.valid.insert(n, Tri::T);
                }
                if self.d0282() {
                    self.eval_order(fl, place, value, line);
                }
            }
            StmtKind::Expr(e) | StmtKind::Print(e) => {
                if let StmtKind::Print(_) = s.kind {
                    if let Some(t) = self.value_ty(e) {
                        if t != Ty::I32 {
                            self.err(line, "diag.type-mismatch", format!("printf of {}", t.text()));
                        }
                    }
                }
                self.rvalue(fl, e, line);
            }
            StmtKind::Block(b) => self.block(fl, b, None),
            StmtKind::Discard(e) => self.rvalue(fl, e, line),
            StmtKind::Match { scrut, binder, some, none } => {
                self.rvalue(fl, scrut, line);
                let bt = match self.value_ty(scrut) {
                    Some(Ty::Opt(t)) => *t,
                    _ => {
                        self.err(line, "diag.type-mismatch", "match on a non-Option".into());
                        Ty::Unit
                    }
                };
                // The arm's binder, when the scrutinee is a call whose result
                // holds a reference without being one: deriv(b, x.π, m) for the
                // first reference parameter's argument, when it is a visible
                // borrow (D-0088, D-0071).
                let mut a = fl.clone();
                let mut facts = Vec::new();
                if bt.holds_ref() {
                    if let Expr::Call(_, args) = scrut {
                        if let Some(Expr::Borrow(m, p)) = args.first() {
                            if let Some(sp) = self.splace(p) {
                                facts.push((sp, *m));
                            }
                        }
                    }
                }
                // D-0282: a frame-held binder the row forms no fact for
                // inherits the scrutinee's ("a `match` arm's binder stands for
                // the scrutinee").
                let mut inh = Vec::new();
                self.copies.remove(binder);
                if self.d0282() && facts.is_empty() && frame_held(&bt) {
                    inh = self.inherit(fl, scrut).0;
                    if let Expr::Var(o) = scrut {
                        let root = self.copy_root(o);
                        self.copies.insert(binder.clone(), root);
                    }
                }
                // D-0289, CHG-0384: a handle taken out of a `Vec` of them holds
                // what the `Vec` holds.
                if let (Ty::Handle(_), Expr::Call(f, args)) = (&bt, scrut) {
                    if let (true, Some(Expr::Borrow(_, v))) = (f == "Vec::pop", args.first()) {
                        if let Expr::Var(hs) = &**v {
                            inh = fl.holder_facts(hs);
                        }
                    }
                }
                let id = self.pos.decl.get(&(s as *const Stmt)).copied().unwrap_or(usize::MAX);
                self.block(&mut a, some, Some(Pre { name: binder.clone(), ty: bt, facts, inh, id }));
                let mut b = fl.clone();
                self.block(&mut b, none, None);
                *fl = a.join(&b);
            }
            StmtKind::While { cond, body } => {
                // `rule.control.flow-analysis`: the loop's head joins its
                // entry with each back edge, iterated to the least fixed
                // point; the diagnostics, proofs and ends are those of a
                // last pass at the fixed point.
                let start = self.next_block;
                let (saved_err, saved_proven, saved_ends) = (self.errors.len(), self.proven.clone(), self.ends.clone());
                // A `continue` joins the head as the back edge does; a
                // `break` joins the loop's exit.
                let dead = Flow { dead: true, ..Flow::default() };
                let depth = self.regions.len();
                let mut head = fl.clone();
                for _ in 0..64 {
                    self.next_block = start;
                    let mut b = head.clone();
                    self.rvalue(&mut b, cond, line);
                    self.loops.push((depth, dead.clone(), dead.clone()));
                    self.block(&mut b, body, None);
                    let (_, _, cont) = self.loops.pop().unwrap();
                    let j = head.join(&b).join(&cont);
                    if j == head {
                        break;
                    }
                    head = j;
                }
                self.errors.truncate(saved_err);
                self.proven = saved_proven;
                self.ends = saved_ends;
                self.next_block = start;
                self.rvalue(&mut head, cond, line);
                let mut b = head.clone();
                self.loops.push((depth, dead.clone(), dead));
                self.block(&mut b, body, None);
                let (_, brk, _) = self.loops.pop().unwrap();
                *fl = head.join(&brk);
            }
            StmtKind::Break | StmtKind::Continue => {
                if self.loops.is_empty() {
                    self.err(line, "diag.break-outside-loop", "break or continue outside a loop".into());
                    return;
                }
                // `[Break]`: every block up to and including the loop's body
                // is exited, its bindings ended.
                let depth = self.loops.last().unwrap().0;
                let mut f = fl.clone();
                let names: Vec<String> = self.regions[depth..].iter().flat_map(|r| r.iter().cloned()).collect();
                for n in &names {
                    end_binding(&mut f, n);
                }
                let l = self.loops.last_mut().unwrap();
                if matches!(s.kind, StmtKind::Break) {
                    l.1 = l.1.join(&f);
                } else {
                    l.2 = l.2.join(&f);
                }
                fl.dead = true;
            }
            StmtKind::If { cond, then, els } => {
                self.rvalue(fl, cond, line);
                let mut a = fl.clone();
                self.block(&mut a, then, None);
                let mut b = fl.clone();
                if let Some(e) = els {
                    self.block(&mut b, e, None);
                }
                *fl = a.join(&b);
            }
        }
    }

    fn is_elided(&self, e: &Expr) -> bool {
        match e {
            Expr::Call(f, args) => matches!(self.prog.func(f).map(|x| &x.ret), Some(Ty::Ref(..))) && args.first().map_or(false, |a| matches!(a, Expr::Borrow(..))),
            _ => false,
        }
    }

    // `r = e` from anything but a visible borrow: `escaped(x) := T for the x
    // the analysis cannot exclude` — read here as every place some
    // reference binding mentioned in `e` is known to derive from.
    fn escape_copied(&mut self, fl: &mut Flow, e: &Expr) {
        let mut names = Vec::new();
        expr_names(e, &mut names);
        let keys: Vec<_> = fl.deriv.iter().filter(|(k, v)| **v != Tri::F && names.contains(&k.0)).map(|(k, _)| k.1.root.clone()).collect();
        for r in keys {
            fl.escaped.insert(r, Tri::T);
        }
    }

    // The referent block of a reference or slice value: a visible borrow's,
    // a slice's source's.
    fn referent_of_value(&self, e: &Expr) -> Option<usize> {
        match e {
            Expr::Borrow(_, p) => self.referent_block(p),
            Expr::Slice(_, v, ..) => self.referent_block(&Expr::Var(v.clone())),
            _ => None,
        }
    }

    fn referent_block(&self, p: &Expr) -> Option<usize> {
        match p {
            Expr::Var(n) | Expr::Index(n, _) => match self.ty_of(n)? {
                Ty::Ref(..) => None,
                Ty::Slice(_) => *self.referent.get(n)?,
                _ => self.block_of(n),
            },
            Expr::Deref(a) => match &**a {
                Expr::Var(q) => *self.referent.get(q)?,
                _ => None,
            },
            Expr::Field(_) => None,
            _ => None,
        }
    }

    // Does block `outer` strictly enclose block `inner`?
    fn encloses(&self, outer: usize, inner: usize) -> bool {
        let mut b = self.parents.get(&inner).copied().flatten();
        while let Some(x) = b {
            if x == outer {
                return true;
            }
            b = self.parents.get(&x).copied().flatten();
        }
        false
    }

    // A block; `pre` is a pattern binder the block's region declares (a
    // `match` arm's), with the deriv facts it starts with.
    fn block(&mut self, fl: &mut Flow, b: &Block, pre: Option<Pre>) {
        let me = self.next_block;
        self.next_block += 1;
        self.block_in(fl, b, me, pre);
    }

    fn block_in(&mut self, fl: &mut Flow, b: &Block, me: usize, pre: Option<Pre>) {
        self.regions.push(BTreeSet::new());
        let mut declared = Vec::new();
        if let Some(Pre { name: n, ty: t, facts, inh, id }) = pre {
            self.binds.entry(n.clone()).or_default().push(Binding { ty: t, block: me, id });
            self.regions.last_mut().unwrap().insert(n.clone());
            fl.valid.insert(n.clone(), Tri::T);
            for (sp, m) in facts {
                fl.deriv.insert((n.clone(), sp, m), Tri::T);
            }
            for (sp, m, v) in inh {
                fl.inh.insert((n.clone(), sp, m), v);
            }
            declared.push(n);
        }
        // `[Ref-Binding-Last-Use]` (D-0221): each candidate waits from its
        // last mention until no binding that may hold a reference and shares
        // a statement with it is mentioned later; it then ends, unless the
        // flow facts root another binding's borrow in it.
        let (plan, mentions, tail_names) = end_plan(self, b);
        let mut waiting: Vec<(String, usize)> = Vec::new();
        let mut plan = plan;
        for p in plan.iter_mut() {
            p.sort();
        }
        for (i, s) in b.stmts.iter().enumerate() {
            // Nested blocks get their numbers in pre-order as they are met.
            self.enter_children(s, me);
            self.stmt(fl, s, me);
            if let StmtKind::Let { name, .. } = &s.kind {
                declared.push(name.clone());
            }
            let mut now: Vec<String> = Vec::new();
            waiting.extend(plan[i].iter().cloned());
            let mut still = Vec::new();
            let mut free = Vec::new();
            for (x, d) in std::mem::take(&mut waiting) {
                if self.end_blocked(b, &x, d, i, &mentions, &tail_names) {
                    still.push((x, d));
                } else {
                    free.push((x, d));
                }
            }
            waiting = still;
            // One at a time, in order: a binding ended frees what was
            // rooted in it for the next.
            // One whose facts still root another binding's reference waits
            // for a later statement (D-0288).
            for (x, d) in free {
                let rooted = fl.deriv.iter().any(|(k, v)| *v != Tri::F && k.0 != x && (k.1.root == Root::Var(x.clone()) || k.1.root == Root::Deref(x.clone())));
                if !rooted {
                    end_binding(fl, &x);
                    now.push(x);
                } else {
                    waiting.push((x, d));
                }
            }
            if !now.is_empty() {
                for n in &now {
                    // `[Ref-Binding-Last-Use]`: the binding ends here as at
                    // its block's exit (the flow table's row, D-0219).
                    end_binding(fl, n);
                }
                self.ends.insert((self.fname.clone(), me, i), now);
            }
        }
        if let Some(t) = &b.tail {
            self.rvalue(fl, t, b.tail_line);
            if me == 0 {
                // a function's (or closure's) result
                self.move_out(t, b.tail_line);
            }
            if let Some(tt) = self.value_ty(t) {
                let want = self.ret.clone();
                let ok = tt == want || matches!((&want, &tt), (Ty::Ref(a, Mode::Shared), Ty::Ref(b, _)) if a == b);
                if me == 0 && !ok {
                    self.err(b.tail_line, "diag.type-mismatch", format!("the body's value is {}, the result type {}", tt.text(), want.text()));
                }
            }
        }
        // block exit
        for n in &declared {
            end_binding(fl, n);
            if let Some(v) = self.binds.get_mut(n) {
                if let Some(bd) = v.pop() {
                    self.all_types.insert(n.clone(), bd.ty);
                }
            }
        }
        self.regions.pop();
    }

    fn enter_children(&mut self, _s: &Stmt, _me: usize) {}

    // Whether `x` (declared by statement `d`) must outlive statement `i`: a
    // binding that may hold a reference shares a statement with it since its
    // declaration and is mentioned after `i`.
    fn end_blocked(&self, b: &Block, x: &str, d: usize, i: usize, mentions: &[Vec<String>], tail: &[String]) -> bool {
        // D-0288: `m` stops counting at a later statement that assigns it
        // whole from a value that does not read it, with no `break` or
        // `continue` on the way: what `m` got in statement `j` ends there.
        let later = |m: &str, j: usize| {
            for k in j + 1..b.stmts.len() {
                if crate::ast::stmt_may_leave(&b.stmts[k - 1]) {
                    break;
                }
                if crate::ast::stmt_overwrites(&b.stmts[k], m) {
                    return mentions[i + 1..k.max(i + 1)].iter().any(|ms| ms.iter().any(|n| n == m));
                }
            }
            tail.iter().any(|t| t == m) || mentions[i + 1..].iter().any(|ms| ms.iter().any(|n| n == m))
        };
        (d..=i).filter(|&j| j == d || mentions[j].iter().any(|n| n == x)).any(|j| {
            let declared = match &b.stmts[j].kind {
                StmtKind::Let { name, .. } => Some(name.clone()),
                _ => None,
            };
            mentions[j].iter().cloned().chain(declared.clone()).any(|m| {
                m != x
                    && self.ty_of(&m).or_else(|| decl_ty_in(self, b, &m)).map_or(false, |t| {
                        may_hold_ref(&t) && (declared.as_deref() == Some(m.as_str()) || !inert_ref(&t) || crate::ast::stmt_rebinds(&b.stmts[j], &m))
                    })
                    && later(&m, j)
            })
        })
    }
}

fn end_binding(fl: &mut Flow, n: &str) {
    fl.forget_holder(n);
    fl.clear_referent(n);
    fl.valid.insert(n.to_string(), Tri::F);
}

// D-0275: a reference or slice whose referent holds no reference: it can
// take another binding's path only by being rebound.
fn inert_ref(t: &Ty) -> bool {
    match t {
        Ty::Ref(inner, _) => !may_hold_ref(inner),
        Ty::Slice(_) => true,
        _ => false,
    }
}

fn may_hold_ref(t: &Ty) -> bool {
    match t {
        Ty::Ref(..) | Ty::Hold | Ty::VecR | Ty::Slice(_) | Ty::Fn(_) => true,
        Ty::Opt(x) | Ty::Res(x) | Ty::Chan(x) | Ty::Handle(x) => may_hold_ref(x),
        _ => false,
    }
}

// Each candidate (a `let` of a type that holds a reference and no resource)
// at the statement of its last mention; each statement's mentions; the
// trailing expression's.
fn end_plan(cx: &Cx, b: &Block) -> (Vec<Vec<(String, usize)>>, Vec<Vec<String>>, Vec<String>) {
    let n = b.stmts.len();
    let mut plan = vec![Vec::new(); n];
    let mentions: Vec<Vec<String>> = b
        .stmts
        .iter()
        .map(|s| {
            let mut v = Vec::new();
            stmt_names(s, &mut v);
            v
        })
        .collect();
    let mut tail = Vec::new();
    if let Some(t) = &b.tail {
        expr_names(t, &mut tail);
    }
    // "no closure in the block captures x"
    let mut clos = Vec::new();
    block_closures(b, &mut clos);
    // D-0288: but a `move` closure bound whole (its binding holds the facts)
    let mut whole = BTreeSet::new();
    crate::ast::bound_whole_move(b, &mut whole);
    let captured: BTreeSet<String> = clos.iter().filter(|c| !whole.contains(&c.id)).flat_map(|c| c.caps.iter().cloned()).collect();
    for (d, s) in b.stmts.iter().enumerate() {
        let StmtKind::Let { name: x, ty, init } = &s.kind else { continue };
        let Some(t) = ty.clone().or_else(|| guess_ty(cx, init)) else { continue };
        if !t.holds_ref() || t.is_resource() || tail.contains(x) || captured.contains(x) {
            continue;
        }
        let last = (d + 1..n).filter(|&j| mentions[j].contains(x)).last().unwrap_or(d);
        if last + 1 < n || b.tail.is_some() {
            plan[last].push((x.clone(), d));
        }
    }
    (plan, mentions, tail)
}


fn decl_ty_in(cx: &Cx, b: &Block, y: &str) -> Option<Ty> {
    for s in &b.stmts {
        if let StmtKind::Let { name, ty, init } = &s.kind {
            if name == y {
                return ty.clone().or_else(|| guess_ty(cx, init));
            }
        }
    }
    None
}

fn guess_ty(cx: &Cx, init: &Expr) -> Option<Ty> {
    match init {
        // Any borrow is a reference, whatever its referent's type.
        Expr::Borrow(m, _) => Some(Ty::Ref(Box::new(Ty::I32), *m)),
        Expr::HoldLit(_) => Some(Ty::Hold),
        Expr::Call(f, _) => match f.as_str() {
            "Vec::new" => Some(Ty::Vec),
            "Mutex::new" => Some(Ty::Mutex),
            "lock" => Some(Ty::Guard),
            "Atomic::new" => Some(Ty::Atomic),
            "Atomic::load" | "Atomic::fetch_add" => Some(Ty::I32),
            _ => cx.prog.func(f).map(|f| f.ret.clone()),
        },
        Expr::Spawn(..) | Expr::SpawnClo(_) => Some(Ty::Handle(Box::new(Ty::Unit))),
        Expr::Slice(m, ..) => Some(Ty::Slice(*m)),
        Expr::Closure(c) => Some(Ty::Fn(Box::new(c.ret.clone()))),
        Expr::JobLit(_) => Some(Ty::Job),
        Expr::ArrLit(_) => Some(Ty::Arr),
        Expr::Int(_) => Some(Ty::I32),
        Expr::Var(n) => decl_or_bound(cx, n),
        _ => None,
    }
}

fn decl_or_bound(cx: &Cx, n: &str) -> Option<Ty> {
    cx.ty_of(n)
}

// Numbers every block of a body in pre-order and records its parent.
fn number_blocks(b: &Block, parent: Option<usize>, next: &mut usize, parents: &mut HashMap<usize, Option<usize>>) {
    let me = *next;
    *next += 1;
    parents.insert(me, parent);
    for s in &b.stmts {
        match &s.kind {
            StmtKind::Block(c) => number_blocks(c, Some(me), next, parents),
            StmtKind::Match { some, none, .. } => {
                number_blocks(some, Some(me), next, parents);
                number_blocks(none, Some(me), next, parents);
            }
            StmtKind::If { then, els, .. } => {
                number_blocks(then, Some(me), next, parents);
                if let Some(e) = els {
                    number_blocks(e, Some(me), next, parents);
                }
            }
            StmtKind::While { body, .. } => number_blocks(body, Some(me), next, parents),
            _ => {}
        }
    }
}

impl<'a> Cx<'a> {
    fn new(prog: &'a Program, sums: &'a Summaries, pos: Positions, fname: String, parents: HashMap<usize, Option<usize>>, ret: Ty) -> Cx<'a> {
        Cx {
            prog,
            fname,
            errors: Vec::new(),
            binds: HashMap::new(),
            regions: vec![BTreeSet::new()],
            parents,
            next_block: 0,
            referent: HashMap::new(),
            ends: HashMap::new(),
            all_types: HashMap::new(),
            proven: HashMap::new(),
            ret,
            caps: BTreeSet::new(),
            clo_info: HashMap::new(),
            accumulate: false,
            clo_owns: BTreeSet::new(),
            sub_types: HashMap::new(),
            loops: Vec::new(),
            sums,
            pos,
            copies: HashMap::new(),
        }
    }
}

// A closure body's free variables: the names it mentions that it does not
// declare and that name no function.
fn free_names(c: &Clo, prog: &Program) -> BTreeSet<String> {
    fn declared(b: &Block, out: &mut BTreeSet<String>) {
        for s in &b.stmts {
            match &s.kind {
                StmtKind::Let { name, .. } => {
                    out.insert(name.clone());
                }
                StmtKind::Match { binder, some, none, .. } => {
                    out.insert(binder.clone());
                    declared(some, out);
                    declared(none, out);
                }
                StmtKind::Block(b) => declared(b, out),
                StmtKind::If { then, els, .. } => {
                    declared(then, out);
                    if let Some(e) = els {
                        declared(e, out);
                    }
                }
                StmtKind::While { body, .. } => declared(body, out),
                _ => {}
            }
        }
    }
    let mut names = Vec::new();
    block_names(&c.body, &mut names);
    let mut decl = BTreeSet::new();
    declared(&c.body, &mut decl);
    // A static is every thread's by name: no capture (D-0258).
    names.into_iter().filter(|n| !decl.contains(n) && prog.func(n).is_none() && !prog.is_static(n)).collect()
}

pub fn check(prog: &Program) -> Statics {
    let mut all = Statics { errors: Vec::new(), ends: HashMap::new(), types: HashMap::new(), proven: HashMap::new() };
    let sums = frames::summaries(prog);
    for f in &prog.funcs {
        let mut parents = HashMap::new();
        let mut next = 0;
        number_blocks(&f.body, None, &mut next, &mut parents);
        let names: Vec<String> = f.params.iter().map(|p| p.1.clone()).collect();
        let pos = Positions::of(&names, &f.body);
        let mut cx = Cx::new(prog, &sums, pos, f.name.clone(), parents, f.ret.clone());
        let mut fl = Flow::default();
        for (t, n) in &f.params {
            let id = cx.binds.len();
            cx.binds.entry(n.clone()).or_default().push(Binding { ty: t.clone(), block: 0, id });
            cx.regions[0].insert(n.clone());
            fl.valid.insert(n.clone(), Tri::T);
            if let Ty::Ref(..) = t {
                cx.referent.insert(n.clone(), None);
            }
        }
        cx.block(&mut fl, &f.body, None);
        all.errors.extend(cx.errors);
        all.ends.extend(cx.ends);
        all.types.insert(f.name.clone(), cx.all_types);
        all.types.extend(cx.sub_types);
        all.proven.extend(cx.proven);
    }
    all
}

// A pattern binder a block's region declares (a `match` arm's), with the
// deriv facts it starts with, those it inherits (D-0282), and its number in
// `Positions`.
struct Pre {
    name: String,
    ty: Ty,
    facts: Vec<(SPlace, Mode)>,
    inh: Vec<(SPlace, Mode, Tri)>,
    id: usize,
}

// D-0282 "Holders": a binding of a reference, slice or `StringView` type, of
// `Option`/`Result` of one, or of a closure or `fn` type. (The other bindings
// §6 gives `deriv` facts, a spawn's handle among them, are holders with those
// facts: `Cx::is_holder`.) Any other binding that holds a reference (a `Hold`,
// a `Vec<ref<…>>`, a `Job`) keeps it in data.
fn frame_held(t: &Ty) -> bool {
    match t {
        Ty::Ref(..) | Ty::Slice(_) | Ty::Fn(_) => true,
        Ty::Opt(x) | Ty::Res(x) => matches!(**x, Ty::Ref(..) | Ty::Slice(_)),
        _ => false,
    }
}

fn permitted(a: Mode, b: Mode) -> bool {
    a == Mode::Shared && b == Mode::Shared
}

fn extend(p: &SPlace, steps: &[StepK]) -> SPlace {
    let mut q = p.clone();
    q.steps.extend(steps.iter().cloned());
    q
}

// Inherited facts: the facts themselves, and whether they name the
// statement's temporary.
type Inh = (Vec<(SPlace, Mode, Tri)>, bool);

// `rule.control.frame-references` (`spec/14` §6, D-0282).
impl<'a> Cx<'a> {
    fn d0282(&self) -> bool {
        !PRE_D0282.load(std::sync::atomic::Ordering::Relaxed)
    }

    // A holder: a frame-held reference, or another binding §6 gives `deriv`
    // facts (a spawn's handle).
    fn is_holder(&self, fl: &Flow, n: &str, t: &Ty) -> bool {
        frame_held(t) || fl.deriv.iter().any(|(k, v)| k.0 == n && *v != Tri::F)
    }

    fn copy_root(&self, n: &str) -> String {
        self.copies.get(n).cloned().unwrap_or_else(|| n.to_string())
    }

    // "Inherited facts": the facts of `e`, by the rule's table.
    fn inherit(&self, fl: &Flow, e: &Expr) -> Inh {
        match e {
            // A capture's place is the closure's field, a static's is static
            // data: neither is a place of this frame.
            Expr::Var(q) if self.caps.contains(q) || self.is_static_var(e) => (vec![], false),
            Expr::Var(q) => match self.ty_of(q) {
                // "a frame-held reference q: q's facts; for a reference or slice
                // q with none, *q whole, in q's mode"
                Some(t) if frame_held(&t) => {
                    let f = fl.holder_facts(q);
                    match t {
                        Ty::Ref(_, m) | Ty::Slice(m) if f.is_empty() => (vec![(SPlace { root: Root::Deref(q.clone()), steps: vec![] }, m, Tri::T)], false),
                        _ => (f, false),
                    }
                }
                // "an owner x of this frame: x whole, shared"
                Some(_) => (vec![(SPlace { root: Root::Var(q.clone()), steps: vec![] }, Mode::Shared, Tri::T)], false),
                None => (vec![], false),
            },
            // "an element of a container, or a reference read out of a field of
            // an aggregate that holds references: tier D". The fragment's one
            // field is `Hold.r`, a reference in an aggregate that holds
            // references, so a value read out of it is tier D; "`b.f` for a
            // frame-held b" counts as a place's projection (under `&_m`, in
            // `place_base`).
            Expr::Field(_) | Expr::Index(..) => (vec![], false),
            // "`*b`: the facts of b"
            Expr::Deref(b) => self.inherit(fl, b),
            // "`&_m p` or a slice of p: the facts of p's root, extended by p's
            // fields up to its first index, in mode m"
            // where it forms one; otherwise …": the fact §6 forms for it (`p`'s
            // place, literal indices and ranges included) comes first.
            Expr::Borrow(m, p) => {
                if let Some(sp) = self.deriv_place(p) {
                    return (vec![(sp, *m, Tri::T)], false);
                }
                let (fs, temp) = self.place_base(fl, p);
                (fs.into_iter().map(|(sp, v)| (sp, *m, v)).collect(), temp)
            }
            Expr::Slice(m, v, lo, hi) => {
                if let Some(sp) = self.slice_place(v, *lo, *hi) {
                    return (vec![(sp, *m, Tri::T)], false);
                }
                let (fs, temp) = self.place_base(fl, &Expr::Var(v.clone()));
                (fs.into_iter().map(|(sp, t)| (sp, *m, t)).collect(), temp)
            }
            Expr::Call(f, args) => {
                // "a call f(a1, …, an) whose result is or holds a reference:
                // the facts of the first argument whose parameter is or holds a
                // reference; none when no parameter is; tier D when the result
                // holds a reference only through a type argument, or when that
                // argument reaches storage that keeps references"
                if !self.value_ty(e).map_or(false, |t| frame_held(&t)) {
                    return (vec![], false);
                }
                if matches!(f.as_str(), "Vec::pop" | "Channel::recv" | "Channel::send") {
                    return (vec![], false);
                }
                let pks = self.param_kinds(f);
                let Some(i) = (0..args.len()).find(|&i| matches!(pks.get(i), Some(PK::Ref(_)) | Some(PK::Holds) | Some(PK::TyParam))) else { return (vec![], false) };
                if let Some(Ty::Ref(inner, _)) = self.value_ty(&args[i]) {
                    if may_hold_ref(&inner) {
                        return (vec![], false);
                    }
                }
                self.inherit(fl, &args[i])
            }
            // a literal, an arithmetic result, and what holds no reference
            _ => (vec![], false),
        }
    }

    // A place's root facts, extended by its fields up to its first index;
    // and whether the root is a temporary ("the result of a call that is not
    // a reference, holds none and is not a constant").
    fn place_base(&self, fl: &Flow, p: &Expr) -> (Vec<(SPlace, Tri)>, bool) {
        let (fs, _, temp) = self.place_base_cut(fl, p);
        (fs, temp)
    }

    fn place_base_cut(&self, fl: &Flow, p: &Expr) -> (Vec<(SPlace, Tri)>, bool, bool) {
        match p {
            Expr::Var(x) if self.caps.contains(x) || self.is_static_var(p) => (vec![], false, false),
            Expr::Var(x) => match self.ty_of(x) {
                Some(_) => (vec![(SPlace { root: Root::Var(x.clone()), steps: vec![] }, Tri::T)], false, false),
                None => (vec![], false, false),
            },
            Expr::Field(a) => {
                // `h.r` of a Hold binding, or of a reference to one
                // (auto-deref: the facts of the reference).
                let (fs, cut, temp) = match &**a {
                    Expr::Var(q) if matches!(self.ty_of(q), Some(Ty::Ref(..))) => {
                        let (f, t) = self.inherit(fl, a);
                        (f.into_iter().map(|(sp, _, v)| (sp, v)).collect(), false, t)
                    }
                    other => self.place_base_cut(fl, other),
                };
                if cut {
                    (fs, cut, temp)
                } else {
                    (fs.into_iter().map(|(sp, v)| (extend(&sp, &[StepK::Field]), v)).collect(), false, temp)
                }
            }
            // An index cuts the path: the facts of the indexed binding's root
            // (a slice's: the slice's facts).
            Expr::Index(v, _) => {
                let ve = Expr::Var(v.clone());
                let (fs, _, temp) = match self.ty_of(v) {
                    Some(Ty::Slice(_)) => {
                        let (f, t) = self.inherit(fl, &ve);
                        (f.into_iter().map(|(sp, _, v)| (sp, v)).collect(), false, t)
                    }
                    _ => self.place_base_cut(fl, &ve),
                };
                (fs, true, temp)
            }
            Expr::Deref(b) => {
                let (f, t) = self.inherit(fl, b);
                (f.into_iter().map(|(sp, _, v)| (sp, v)).collect(), false, t)
            }
            Expr::Call(..) => {
                let r = self.value_ty(p);
                let temp = !matches!(r, Some(Ty::Ref(..)) | Some(Ty::Slice(_))) && !r.map_or(false, |t| t.holds_ref());
                (vec![], false, temp)
            }
            _ => (vec![], false, false),
        }
    }

    // A frame-held binding `n` bound (declared or assigned) from `e`: where
    // the flow analysis' rows formed no fact, it inherits `e`'s. A binding
    // whose inherited facts name a temporary and which is live after its
    // statement is refuted.
    #[allow(clippy::too_many_arguments)]
    fn bind_frame_held(&mut self, fl: &mut Flow, n: &str, t: &Ty, e: &Expr, inh: Inh, id: usize, end: usize, line: usize) {
        self.copies.remove(n);
        if !frame_held(t) {
            return;
        }
        for (k, v) in fl.inh.iter_mut() {
            if k.0 == n {
                *v = Tri::F;
            }
        }
        if fl.deriv.iter().any(|(k, v)| k.0 == n && *v != Tri::F) {
            return;
        }
        let (facts, temp) = inh;
        for (sp, m, v) in facts {
            fl.inh.insert((n.to_string(), sp, m), v);
        }
        if let Expr::Var(q) = e {
            if self.ty_of(q).map_or(false, |t| frame_held(&t)) {
                let root = self.copy_root(q);
                self.copies.insert(n.to_string(), root);
            }
        }
        if temp && self.pos.live_after(id, end) {
            self.err(line, "diag.destroy-while-aliased", format!("`{}` borrows a temporary that ends with its statement", n));
        }
    }

    fn param_kinds(&self, f: &str) -> Vec<PK> {
        match self.prog.func(f) {
            Some(func) => func.params.iter().map(|p| frames::ty_pk(&p.0)).collect(),
            None => frames::std_params(f).unwrap_or_default(),
        }
    }

    // The frame-held bindings in scope that are live at position `p`, other
    // than those in `skip`: by mention, and "while another live binding
    // holds a fact rooted at *h".
    fn live_holders(&self, fl: &Flow, p: usize, skip: &BTreeSet<String>) -> Vec<String> {
        let held: Vec<(String, usize)> = self
            .binds
            .iter()
            .filter_map(|(n, v)| v.last().map(|b| (n, b)))
            .filter(|(n, b)| self.is_holder(fl, n, &b.ty) && !skip.contains(*n) && !self.caps.contains(*n))
            .map(|(n, b)| (n.clone(), b.id))
            .collect();
        // A handle or a `Vec` of handles is not ended at its last mention:
        // its end waits for its threads (CHG-0384), so it holds its facts to
        // its scope's end.
        let waits = |n: &str| matches!(self.ty_of(n), Some(Ty::Handle(_)) | Some(Ty::VecH));
        let mut live: BTreeSet<String> = held.iter().filter(|(n, id)| self.pos.live_after(*id, p) || waits(n)).map(|(n, _)| n.clone()).collect();
        loop {
            let more: Vec<String> = held
                .iter()
                .filter(|(h, _)| !live.contains(h))
                .filter(|(h, _)| live.iter().any(|g| fl.holder_facts(g).iter().any(|(sp, _, _)| sp.root == Root::Deref(h.clone()))))
                .map(|(h, _)| h.clone())
                .collect();
            if more.is_empty() {
                break;
            }
            live.extend(more);
        }
        live.into_iter().collect()
    }

    // The bindings an argument is formed through: "the root of its place,
    // through fields, dereferences, indexes and the reference arguments of a
    // call on it".
    fn formed_through(&self, a: &Expr, out: &mut BTreeSet<String>) {
        match a {
            Expr::Var(n) | Expr::Index(n, _) | Expr::Slice(_, n, ..) => {
                out.insert(n.clone());
            }
            Expr::Deref(b) | Expr::Field(b) | Expr::Borrow(_, b) => self.formed_through(b, out),
            Expr::Call(g, args) => {
                let pks = self.param_kinds(g);
                for (i, x) in args.iter().enumerate() {
                    if matches!(pks.get(i), Some(PK::Ref(_))) {
                        self.formed_through(x, out);
                    }
                }
            }
            _ => {}
        }
    }

    // "The call-site rule": at a call of a named function, each argument
    // whose parameter is or holds a reference has a reach W and an effect E;
    // the call is refuted when a key of some E certainly overlaps a fact T
    // of a live binding other than the arguments' own (1), or a key of
    // another argument's W (2), in modes not permitted together.
    fn call_site(&mut self, fl: &Flow, e: &Expr, f: &str, args: &[Expr], line: usize, holder: Option<&str>) {
        if !self.d0282() {
            return;
        }
        let Some(&p) = self.pos.at.get(&(e as *const Expr)) else { return };
        let pks = self.param_kinds(f);
        let body = self.prog.func(f).is_some();
        // (argument index, W, E)
        let mut reach: Vec<(usize, Vec<(SPlace, Mode)>, Vec<(SPlace, Mode)>)> = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let m = match pks.get(i) {
                Some(PK::Ref(m)) => *m,
                Some(PK::Holds) => Mode::Shared,
                // "an argument of a type parameter's type counts, shared, when
                // its value is or holds a reference"
                Some(PK::TyParam) => match self.value_ty(a) {
                    Some(t) if t.holds_ref() || frame_held(&t) => Mode::Shared,
                    _ => continue,
                },
                _ => continue,
            };
            let mut w: Vec<(SPlace, Mode)> = Vec::new();
            match a {
                // "for a frame-held reference binding, its referent and its facts"
                Expr::Var(r) if self.ty_of(r).map_or(false, |t| frame_held(&t)) && !self.caps.contains(r) => {
                    if matches!(self.ty_of(r), Some(Ty::Ref(..)) | Some(Ty::Slice(_))) {
                        w.push((SPlace { root: Root::Deref(r.clone()), steps: vec![] }, m));
                    }
                    w.extend(fl.holder_facts(r).into_iter().filter(|x| x.2 == Tri::T).map(|(sp, _, _)| (sp, m)));
                }
                // "otherwise the facts ai inherits"
                _ => w.extend(self.inherit(fl, a).0.into_iter().filter(|x| x.2 == Tri::T).map(|(sp, _, _)| (sp, m))),
            }
            // "when F has a body and the parameter is a reference or slice,
            // W(ai) extended by each projection of reads(F, pi) (shared) and
            // of writes(F, pi) (exclusive for an exclusive parameter, shared
            // for a shared one); otherwise W(ai)". A function of `std` is not
            // in the program: its summary {ε} gives back W.
            let ef = match (body, pks.get(i), self.sums.get(f).and_then(|v| v.get(i).cloned().flatten())) {
                (true, Some(PK::Ref(_)), Some(s)) => {
                    let mut ef = Vec::new();
                    for (k, _) in &w {
                        for pi in &s.reads {
                            ef.push((extend(k, pi), Mode::Shared));
                        }
                        for pi in &s.writes {
                            ef.push((extend(k, pi), m));
                        }
                    }
                    ef
                }
                _ => w.clone(),
            };
            reach.push((i, w, ef));
        }
        if reach.is_empty() {
            return;
        }
        // 1. Against the live holders other than the arguments' own and the
        // copies of a reference argument.
        let mut own = BTreeSet::new();
        for (i, _, _) in &reach {
            self.formed_through(&args[*i], &mut own);
        }
        if let (Some(h), true) = (holder, self.is_elided(e)) {
            // Not a rule of the text but the model's order: §6's row for
            // `r = f(&_m x…)` (D-0011) is applied as the first argument is
            // evaluated, so at the call the facts under `r`'s name are the
            // result's, not those of the value `r` still holds (which §6
            // checked the borrow against). Without this, `r = touch(&mut x)`
            // would clash with itself. A `let`'s binding is not in scope at
            // the call and needs nothing.
            own.insert(h.to_string());
        }
        let roots: BTreeSet<String> = own.iter().map(|n| self.copy_root(n)).collect();
        let copies: Vec<String> = self.binds.keys().filter(|n| roots.contains(&self.copy_root(n))).cloned().collect();
        own.extend(copies);
        for h in self.live_holders(fl, p, &own) {
            for (q, m2, v) in fl.holder_facts(&h) {
                if v != Tri::T {
                    continue;
                }
                for (i, _, ef) in &reach {
                    if let Some((_, m)) = ef.iter().find(|(k, m)| k.root == q.root && frames::certain(&k.steps, &q.steps) && !permitted(*m, m2)) {
                        let why = format!("`{}` {} through argument {} while `{}` still holds a {} borrow of it", f, if *m == Mode::Excl { "writes" } else { "reads" }, i + 1, h, m2.word());
                        self.err(line, "diag.aliasing-conflict", why);
                        return;
                    }
                }
            }
        }
        // 2. Between two arguments.
        for (i, _, ef) in &reach {
            for (j, w, _) in &reach {
                if i == j || self.one_path(f, &args[*i], &args[*j]) {
                    continue;
                }
                for (k, m) in ef {
                    if w.iter().any(|(k2, m2)| k2.root == k.root && frames::certain(&k.steps, &k2.steps) && !permitted(*m, *m2)) {
                        let why = format!("`{}` {} through argument {} what argument {} also reaches", f, if *m == Mode::Excl { "writes" } else { "reads" }, i + 1, j + 1);
                        self.err(line, "diag.aliasing-conflict", why);
                        return;
                    }
                }
            }
        }
    }

    // "`swap` and `swap_places` of one place written the same way twice are
    // not refuted, nor is one reference binding passed twice, or beside a
    // copy of itself: one path."
    fn one_path(&self, f: &str, a: &Expr, b: &Expr) -> bool {
        if (f == "swap" || f == "swap_places") && a == b {
            return true;
        }
        match (a, b) {
            (Expr::Var(x), Expr::Var(y)) => self.copy_root(x) == self.copy_root(y),
            _ => false,
        }
    }

    // "Element accesses": a read or borrow of a `Vec`'s element `v[k]` is an
    // access to `v` at element `k`, refuted when a live frame-held reference
    // holds a fact T on `v` that certainly overlaps it in a mode not
    // permitted. (No frame-held reference reaches an element in the
    // fragment: `v` is always a binding.)
    fn element_access(&mut self, fl: &Flow, e: &Expr, v: &str, k: i64, u: Use, line: usize) {
        if !self.d0282() || self.caps.contains(v) || !matches!(self.ty_of(v), Some(Ty::Vec) | Some(Ty::VecR)) {
            return;
        }
        let m = match u {
            Use::Read => Mode::Shared,
            Use::Borrow(m) => m,
            // The text names a read or a borrow; a write `v[k] = e` is
            // neither (finding, see the report).
            Use::Write => return,
        };
        let Some(&p) = self.pos.at.get(&(e as *const Expr)) else { return };
        let key = [StepK::Idx(k)];
        for h in self.live_holders(fl, p, &BTreeSet::new()) {
            for (q, m2, val) in fl.holder_facts(&h) {
                if val == Tri::T && q.root == Root::Var(v.to_string()) && frames::certain(&key, &q.steps) && !permitted(m, m2) {
                    self.err(line, "diag.aliasing-conflict", format!("`{}` still holds a {} borrow of `{}`, whose element this {}", h, m2.word(), v, if m == Mode::Excl { "borrows exclusively" } else { "reads" }));
                    return;
                }
            }
        }
    }

    // "Evaluation order": an assignment whose place is reached through a
    // container's element, and whose value "contains an exclusive borrow
    // whose facts reach that container", may move the place before it is
    // written.
    fn eval_order(&mut self, fl: &Flow, place: &Expr, value: &Expr, line: usize) {
        // The container's keys: `v` for `v[k]`; for `*Vec::index_exclusive(a, k)`
        // the facts of `a` (and `*r` itself for a reference binding `r`).
        fn container(cx: &Cx, fl: &Flow, e: &Expr) -> Vec<SPlace> {
            match e {
                Expr::Index(v, _) if matches!(cx.ty_of(v), Some(Ty::Vec) | Some(Ty::VecR)) => vec![SPlace { root: Root::Var(v.clone()), steps: vec![] }],
                Expr::Deref(a) | Expr::Field(a) => container(cx, fl, a),
                Expr::Call(f, args) if f == "Vec::index_exclusive" => {
                    let Some(a) = args.first() else { return vec![] };
                    let mut keys: Vec<SPlace> = cx.inherit(fl, a).0.into_iter().filter(|x| x.2 == Tri::T).map(|x| x.0).collect();
                    if let Expr::Var(r) = a {
                        keys.push(SPlace { root: Root::Deref(r.clone()), steps: vec![] });
                    }
                    keys
                }
                _ => vec![],
            }
        }
        // The facts of every exclusive borrow (or exclusive slice) in `e`.
        fn borrows(cx: &Cx, fl: &Flow, e: &Expr, out: &mut Vec<SPlace>) {
            match e {
                Expr::Borrow(Mode::Excl, b) => {
                    out.extend(cx.inherit(fl, e).0.into_iter().filter(|x| x.2 == Tri::T).map(|x| x.0));
                    borrows(cx, fl, b, out);
                }
                Expr::Slice(Mode::Excl, ..) => out.extend(cx.inherit(fl, e).0.into_iter().filter(|x| x.2 == Tri::T).map(|x| x.0)),
                Expr::Call(_, xs) | Expr::Spawn(_, xs) | Expr::ArrLit(xs) => xs.iter().for_each(|a| borrows(cx, fl, a, out)),
                Expr::Deref(a) | Expr::Field(a) | Expr::Borrow(_, a) | Expr::HoldLit(a) | Expr::JobLit(a) | Expr::CallVal(a) | Expr::SpawnClo(a) => borrows(cx, fl, a, out),
                Expr::Add(a, b) | Expr::Gt(a, b) => {
                    borrows(cx, fl, a, out);
                    borrows(cx, fl, b, out);
                }
                _ => {}
            }
        }
        let keys = container(self, fl, place);
        if keys.is_empty() {
            return;
        }
        let mut facts = Vec::new();
        borrows(self, fl, value, &mut facts);
        if facts.iter().any(|f| keys.iter().any(|k| k.root == f.root && frames::certain(&k.steps, &f.steps))) {
            self.err(line, "diag.stale-binding", "the element this assigns to is found before the value, which borrows its container exclusively and may move it".into());
        }
    }

    // "Moving out through a reference": `*r`, for a reference binding `r`,
    // used where its value is moved (a resource: an argument, an
    // initializer, an assigned value, a function's result) is refuted, at
    // the `*r` (its statement's line in the fragment).
    fn move_out(&mut self, e: &Expr, line: usize) {
        if !self.d0282() {
            return;
        }
        if let Expr::Deref(a) = e {
            if let Expr::Var(r) = &**a {
                if let Some(Ty::Ref(t, _)) = self.ty_of(r) {
                    if t.is_resource() {
                        self.err(line, "diag.move-while-aliased", format!("`*{}` would move the value out of the place `{}` still refers to", r, r));
                    }
                }
            }
        }
    }
}
