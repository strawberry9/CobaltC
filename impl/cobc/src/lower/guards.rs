// D-0320: the length guard of a loop nest. A loop whose variables are kept
// below bounds known before the program runs (a literal, a `const`, a
// `foreach` range's literal end, or an enclosing such variable), and whose
// body indexes slices that the loop never rebinds, is emitted twice: under
// `if (s.len >= N && …)`, N one past the largest index the body can form
// into `s`, and otherwise. Both copies keep every check (`version_loops`
// copies the text as it is, its labels renamed), so the guard cannot change
// what the program does, whatever it says: it only tells the C compiler the
// lengths, which lets it prove the copy's index checks and drop them, as an
// `assert(slice_len(s) >= N)` written before the loop does.

use super::*;
use std::sync::Arc;

// Upper bounds of the integers in a loop nest, as the scan reaches them.
pub(super) struct GuardScan<'s> {
    // The innermost binding last: a name and the largest value it holds
    // wherever the scan reads it.
    env: Vec<(String, u128)>,
    // The candidate slices (bound outside the loop, never written, borrowed
    // or declared again in it) and the count each needs: the largest index
    // formed into it, plus one.
    pub(super) need: Vec<(String, u128)>,
    slices: &'s [String],
    // The program's functions, its `const` items among them.
    fns: &'s coby::fxhash::HashMap<String, Arc<FnDecl>>,
    // Locals bound before the loop to values known before the run
    // (`Fx::const_locals`).
    ends: &'s HashMap<String, u128>,
    // The vectors bound in the nest with a written `simd<T, N>` type, and N.
    lanes: Vec<(String, u128)>,
    // CHG-0449: each candidate slice indexed where the index has no bound
    // known, first time only, with the first name in the index that has
    // none (empty when there is no such name).
    pub(super) loose: Vec<(String, String)>,
}

// What a nest may bind with an upper bound: a name the whole nest never
// writes, borrows or declares a second time (`NameScan`).
fn steady(name: &str, stmts: &[Stmt], tail: Option<&Expr>, step: Option<&Expr>, declared_once: bool) -> bool {
    let mut s = NameScan::new(name);
    s.block(stmts, tail);
    if let Some(st) = step {
        s.expr(st);
    }
    s.writes == 0 && !s.borrows && (!s.declares || declared_once)
}

impl<'s> GuardScan<'s> {
    pub(super) fn new(slices: &'s [String], fns: &'s coby::fxhash::HashMap<String, Arc<FnDecl>>, ends: &'s HashMap<String, u128>) -> Self {
        GuardScan { env: Vec::new(), need: Vec::new(), slices, fns, ends, lanes: Vec::new(), loose: Vec::new() }
    }

    // A `const` item's value: an integer of no parameters whose value is a
    // constant expression.
    fn konst(&self, name: &str, depth: usize) -> Option<u128> {
        if depth > 8 {
            return None;
        }
        let f = self.fns.get(name).filter(|f| f.is_const && f.params.is_empty() && f.type_params.is_empty() && f.body.stmts.is_empty())?;
        if !matches!(f.ret, Type::Int(_)) {
            return None;
        }
        self.exact(f.body.tail.as_deref()?, depth + 1)
    }

    fn lookup(&self, n: &str) -> Option<u128> {
        if let Some((_, v)) = self.env.iter().rev().find(|(k, _)| k == n) {
            return Some(*v);
        }
        self.ends.get(n).copied()
    }

    // The value of a constant expression, when it is one.
    fn exact(&self, e: &Expr, depth: usize) -> Option<u128> {
        match &e.kind {
            ExprKind::IntLit(v, _) => Some(*v),
            ExprKind::Paren(a) => self.exact(a, depth),
            ExprKind::Call(c, args) if args.is_empty() => match &c.kind {
                ExprKind::Path(segs, targs) if targs.is_empty() => self.konst(&segs.join("::"), depth),
                _ => None,
            },
            ExprKind::Binary(op, a, b) => {
                let (x, y) = (self.exact(a, depth)?, self.exact(b, depth)?);
                match op {
                    BinOp::Add => x.checked_add(y),
                    BinOp::Sub => x.checked_sub(y),
                    BinOp::Mul => x.checked_mul(y),
                    BinOp::Div => x.checked_div(y),
                    BinOp::Rem => x.checked_rem(y),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    // The largest value `e` can have where the scan reads it, an index's
    // arithmetic being unsigned (every operand of an index is a `usize`).
    pub(super) fn upper(&self, e: &Expr, depth: usize) -> Option<u128> {
        if depth > 16 {
            return None;
        }
        if let Some(v) = self.exact(e, depth) {
            return Some(v);
        }
        match &e.kind {
            ExprKind::Paren(a) => self.upper(a, depth + 1),
            ExprKind::Path(segs, _) if segs.len() == 1 => self.lookup(&segs[0]),
            ExprKind::Call(c, args) if args.len() == 1 => match &c.kind {
                ExprKind::Path(segs, _) if segs.len() == 1 && segs[0] == "widen" => self.upper(&args[0], depth + 1),
                _ => None,
            },
            ExprKind::Binary(op, a, b) => match op {
                BinOp::Add => self.upper(a, depth + 1)?.checked_add(self.upper(b, depth + 1)?),
                BinOp::Mul => self.upper(a, depth + 1)?.checked_mul(self.upper(b, depth + 1)?),
                // A subtraction that would go below zero faults instead.
                BinOp::Sub => Some(self.upper(a, depth + 1)?.saturating_sub(self.exact(b, depth).unwrap_or(0))),
                BinOp::Div => {
                    let d = self.exact(b, depth).filter(|d| *d > 0)?;
                    Some(self.upper(a, depth + 1)? / d)
                }
                BinOp::Rem => {
                    let d = self.exact(b, depth).filter(|d| *d > 0)?;
                    Some(match self.upper(a, depth + 1) {
                        Some(u) => u.min(d - 1),
                        None => d - 1,
                    })
                }
                BinOp::BitAnd => match (self.upper(a, depth + 1), self.upper(b, depth + 1)) {
                    (Some(x), Some(y)) => Some(x.min(y)),
                    (Some(x), None) | (None, Some(x)) => Some(x),
                    _ => None,
                },
                BinOp::Shr => Some(self.upper(a, depth + 1)? >> u32::try_from(self.exact(b, depth)?).ok().filter(|s| *s < 128)?),
                _ => None,
            },
            _ => None,
        }
    }

    // CHG-0449: an index into the candidate slice `s` with no known bound.
    fn note_loose(&mut self, s: &str, idx: &Expr) {
        if self.loose.iter().any(|(k, _)| k == s) {
            return;
        }
        let name = self.unbounded(idx).unwrap_or_default();
        self.loose.push((s.to_string(), name));
    }

    // The first name in `e` (left to right) with no known bound: not a
    // variable the scan bounds, nor a constant.
    fn unbounded(&self, e: &Expr) -> Option<String> {
        match &e.kind {
            ExprKind::Path(segs, _) if segs.len() == 1 => (self.lookup(&segs[0]).is_none() && self.konst(&segs[0], 0).is_none()).then(|| segs[0].clone()),
            ExprKind::Paren(a) | ExprKind::Unary(_, a) => self.unbounded(a),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => self.unbounded(a).or_else(|| self.unbounded(b)),
            ExprKind::Call(_, args) => args.iter().find_map(|a| self.unbounded(a)),
            _ => None,
        }
    }

    fn record(&mut self, s: &str, n: u128) {
        match self.need.iter_mut().find(|(k, _)| k == s) {
            Some((_, m)) => *m = (*m).max(n),
            None => self.need.push((s.to_string(), n)),
        }
    }

    // The bound a loop's test keeps its variable under, in its body: the
    // variable, and the largest value it has there.
    fn test_bound(&self, c: &Expr) -> Option<(String, u128)> {
        let c = strip_parens(c);
        let (x, k, b, incl) = match &c.kind {
            ExprKind::Binary(BinOp::And, a, _) => return self.test_bound(a),
            ExprKind::Binary(BinOp::Lt, a, b) => (&**a, 0u128, &**b, false),
            ExprKind::Binary(BinOp::Le, a, b) => (&**a, 0u128, &**b, true),
            ExprKind::Binary(BinOp::Gt, b, a) => (&**a, 0u128, &**b, false),
            ExprKind::Binary(BinOp::Ge, b, a) => (&**a, 0u128, &**b, true),
            _ => return None,
        };
        // `x + k < b`: `x < b - k`.
        let (x, k) = match &strip_parens(x).kind {
            ExprKind::Binary(BinOp::Add, l, r) => match self.exact(r, 0) {
                Some(k2) => (strip_parens(l), k + k2),
                None => (strip_parens(x), k),
            },
            _ => (strip_parens(x), k),
        };
        let ExprKind::Path(segs, _) = &x.kind else { return None };
        if segs.len() != 1 {
            return None;
        }
        let hi = self.upper(b, 0)?.checked_sub(k)?;
        let hi = if incl { hi } else { hi.checked_sub(1)? };
        Some((segs[0].clone(), hi))
    }

    // A loop of the nest: its variable bounded in its body when its test
    // keeps it there and nothing but the step writes it.
    pub(super) fn while_loop(&mut self, c: &Expr, b: &Block, step: Option<&Expr>) {
        let bound = self.test_bound(c).filter(|(x, _)| steady(x, &b.stmts, b.tail.as_deref(), None, false));
        let at = self.env.len();
        if let Some(xb) = bound {
            self.env.push(xb);
        }
        self.block(&b.stmts, b.tail.as_deref());
        self.env.truncate(at);
        let _ = step;
    }

    fn block(&mut self, stmts: &[Stmt], tail: Option<&Expr>) {
        let at = self.env.len();
        for (i, st) in stmts.iter().enumerate() {
            match st {
                Stmt::Let { name, init: Some(e), ty, .. } => {
                    self.expr(e);
                    if let Some(Type::Simd(_, n)) = ty {
                        self.lanes.push((name.clone(), *n));
                    }
                    // Bound for the rest of the block when the rest never
                    // writes, borrows or declares it again.
                    if let Some(u) = self.upper(e, 0) {
                        if steady(name, &stmts[i + 1..], tail, None, false) {
                            self.env.push((name.clone(), u));
                        }
                    }
                }
                Stmt::Let { init: None, .. } => {}
                Stmt::Destructure { init, .. } => self.expr(init),
                Stmt::Expr(e) | Stmt::BlockLike(e) => self.expr(e),
            }
        }
        if let Some(t) = tail {
            self.expr(t);
        }
        self.env.truncate(at);
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Index(base, idx) => {
                if let ExprKind::Path(segs, _) = &strip_parens(base).kind {
                    if segs.len() == 1 && self.slices.contains(&segs[0]) {
                        match self.upper(idx, 0).and_then(|u| u.checked_add(1)) {
                            Some(u) => self.record(&segs[0].clone(), u),
                            None => self.note_loose(&segs[0].clone(), idx),
                        }
                    }
                }
                self.expr(base);
                self.expr(idx);
            }
            // `simd_load<simd<T, N>>(s, i)` reads `s[i .. i + N]`.
            ExprKind::Call(c, args) => {
                if let ExprKind::Path(segs, targs) = &c.kind {
                    // `simd_store(s, i, x)` writes `s[i .. i + N]`, N the lanes of
                    // `x` (a vector whose type the nest writes, or a load).
                    if segs.len() == 1 && segs[0] == "simd_store" && args.len() == 3 {
                        let n = match &strip_parens(&args[2]).kind {
                            ExprKind::Path(x, _) if x.len() == 1 => self.lanes.iter().rev().find(|(k, _)| *k == x[0]).map(|(_, n)| *n),
                            ExprKind::Call(f, _) => match &f.kind {
                                ExprKind::Path(g, t) if g.len() == 1 && g[0] == "simd_load" => match t.first() {
                                    Some(Type::Simd(_, n)) => Some(*n),
                                    _ => None,
                                },
                                _ => None,
                            },
                            _ => None,
                        };
                        if let (Some(n), ExprKind::Path(s, _)) = (n, &strip_parens(&args[0]).kind) {
                            if s.len() == 1 && self.slices.contains(&s[0]) {
                                match self.upper(&args[1], 0).and_then(|u| u.checked_add(n)) {
                                    Some(u) => self.record(&s[0].clone(), u),
                                    None => self.note_loose(&s[0].clone(), &args[1]),
                                }
                            }
                        }
                    }
                    if segs.len() == 1 && segs[0] == "simd_load" && args.len() == 2 {
                        if let (Some(Type::Simd(_, n)), ExprKind::Path(s, _)) = (targs.first(), &strip_parens(&args[0]).kind) {
                            if s.len() == 1 && self.slices.contains(&s[0]) {
                                match self.upper(&args[1], 0).and_then(|u| u.checked_add(*n as u128)) {
                                    Some(u) => self.record(&s[0].clone(), u),
                                    None => self.note_loose(&s[0].clone(), &args[1]),
                                }
                            }
                        }
                    }
                }
                self.expr(c);
                args.iter().for_each(|a| self.expr(a));
            }
            ExprKind::While(c, b, st) => self.while_loop(c, b, st.as_deref()),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => self.block(&b.stmts, b.tail.as_deref()),
            ExprKind::If(c, t, f) => {
                self.expr(c);
                self.block(&t.stmts, t.tail.as_deref());
                if let Some(f) = f {
                    self.expr(f);
                }
            }
            ExprKind::Match(sc, arms) => {
                self.expr(sc);
                for a in arms {
                    self.expr(&a.body);
                }
            }
            ExprKind::Unary(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::Borrow(_, a) => self.expr(a),
            ExprKind::Binary(_, a, b) | ExprKind::Assign(a, b) => {
                self.expr(a);
                self.expr(b);
            }
            ExprKind::SliceOf(_, a, lo, hi) => {
                self.expr(a);
                self.expr(lo);
                self.expr(hi);
            }
            ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| self.expr(a)),
            ExprKind::ArrayLit(es) => es.iter().for_each(|a| self.expr(a)),
            ExprKind::Return(Some(a)) => self.expr(a),
            // A closure's body runs when it is called: not scanned.
            _ => {}
        }
    }
}

// The value a local is bound to, when it is a constant expression: for a
// `foreach` range's hidden end (`$hN`, `parser::foreach_range`), the `hi` of
// `$each_range($v, hi, n)`, which gives `hi`.
pub(super) fn known_value(name: &str, e: &Expr, fns: &coby::fxhash::HashMap<String, Arc<FnDecl>>) -> Option<u128> {
    let ends = HashMap::new();
    let scan = GuardScan::new(&[], fns, &ends);
    let range_end = name.len() > 2 && name.starts_with("$h") && name[2..].bytes().all(|c| c.is_ascii_digit());
    match &e.kind {
        ExprKind::Call(c, args) if range_end && args.len() == 3 && matches!(&c.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == "$each_range") => scan.exact(&args[1], 0),
        _ => scan.exact(e, 0),
    }
}

impl<'g, 'a> Fx<'g, 'a> {
    // D-0320: the guard of the loop `while (c) b` (with its step): one test
    // `(S).len >= N` for each slice in scope that the loop never writes,
    // borrows or declares again and whose largest index in it is known
    // (`GuardScan`), joined with `&&`; None when there is none.
    pub(super) fn loop_guard(&mut self, c: &Expr, b: &Block, step: Option<&Expr>) -> Option<String> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut slices: Vec<String> = Vec::new();
        for scope in self.scopes.iter().rev() {
            for (n, bnd) in scope {
                if seen.insert(n.clone()) && matches!(bnd.ty, Type::Slice(..)) {
                    let mut s = NameScan::new(n);
                    s.expr(c);
                    s.block(&b.stmts, b.tail.as_deref());
                    if let Some(st) = step {
                        s.expr(st);
                    }
                    if s.writes == 0 && !s.borrows && !s.declares {
                        slices.push(n.clone());
                    }
                }
            }
        }
        if slices.is_empty() {
            return None;
        }
        let mut scan = GuardScan::new(&slices, &self.g.items.fns, &self.const_locals);
        scan.while_loop(c, b, step);
        let mut tests = Vec::new();
        let mut shown = Vec::new();
        for (s, n) in &scan.need {
            let (Some(bnd), Ok(n)) = (self.lookup(s), u64::try_from(*n)) else { continue };
            tests.push(format!("({}).len >= {}u", bnd.c, n));
            shown.push(format!("slice_len({}) >= {}", s, n));
        }
        // CHG-0449: what `--report-guards` says of the loop; the caller adds
        // whether the guard is used.
        if self.g.report_guards {
            let mut note = String::new();
            if !shown.is_empty() {
                note.push_str(&format!("  guard: {}\n", shown.join(" && ")));
            }
            for (s, name) in &scan.loose {
                if name.is_empty() {
                    note.push_str(&format!("  no bound for an index of `{}`: its checks stay\n", s));
                } else {
                    note.push_str(&format!("  no bound for an index of `{}`: `{}` is not kept below a value known before the run, so its checks stay\n", s, name));
                }
            }
            self.pending_note = Some(note);
        }
        (!tests.is_empty()).then(|| tests.join(" && "))
    }
}
