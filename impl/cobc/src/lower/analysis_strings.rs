// Per-function analyses: objectless String locals and pure direct calls.

use super::*;

impl<'a> Gen<'a> {
    // The `String` locals of a function body that need no runtime object:
    // declared once, by `let` with an initializer; no parameter, binder,
    // capture or destructured field of the name; and every mention of the
    // name is `&s` handed to a reader parameter (`reader_param`) of a named
    // function, as no other argument of that call names it
    // (`Fx::reader_passthrough`). No path to `s` but its root is ever
    // formed, nothing writes it or moves it, and a reader given token 0
    // makes no check (`cb_read`'s note): the object would only be created,
    // bound and ended. `Fx::bind_value` checks the type. By name, across
    // scopes, like `confined_vecs`.
    // D-0196: by binding, not by name: each `let` with an initializer is a
    // candidate of its own (identified by its initializer's address), and
    // a use resolves to the innermost binding of its name, so the same name
    // declared in two blocks (or shadowed) no longer disqualifies both.
    pub(super) fn objectless_strings(&self, body: &Block, params: &[String]) -> HashSet<usize> {
        struct W<'s, 'a> {
            g: &'s Gen<'a>,
            bound: HashSet<String>,
            scopes: Vec<HashMap<String, usize>>,
            bad: HashSet<usize>,
            seen: HashSet<usize>,
            // D-0200: the candidates moved out (`move_out`): only a `String`
            // knows how to be left empty there (`read_place`).
            moved: HashSet<usize>,
            compared: HashSet<usize>,
            indexed: HashSet<usize>,
            names: HashMap<usize, String>,
            depth: usize,
        }
        // `std`'s `String` builders, which change the `String` they are lent
        // and keep no reference to it (each is native or a few `Vec` calls
        // that take an unchecked reference, token 0, as any reader does).
        const BUILDERS: &[&str] = &["std::String::append", "std::String::push_ascii", "std::String::push_code_point", "std::String::clear", "std::String::truncate"];
        impl W<'_, '_> {
            // The binding a name resolves to here: 0 for one that is not a
            // candidate (a parameter, a binder, a `let` with no initializer).
            fn id(&self, n: &str) -> Option<usize> {
                self.scopes.iter().rev().find_map(|m| m.get(n).copied())
            }
            fn mark_bad(&mut self, n: &str) {
                if let Some(id) = self.id(n) {
                    if id != 0 {
                        if std::env::var("COBC_DEBUG_W").is_ok() && !self.bad.contains(&id) {
                            eprintln!("W bad {}", n);
                        }
                        self.bad.insert(id);
                    }
                }
            }
            fn reader_arg(&self, callee: &Expr, args: &[Expr], j: usize) -> Option<String> {
                let ExprKind::Borrow(Mode::Shared, x) = &strip_parens(&args[j]).kind else { return None };
                let s = path_name(x)?.to_string();
                let ExprKind::Path(segs, _) = &callee.kind else { return None };
                if segs.len() < 2 && segs.first().map_or(true, |n| self.bound.contains(n)) {
                    return None;
                }
                let k = segs.join("::");
                let k = if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
                let f = self.g.items.fns.get(&k)?;
                if !(appended_text(&k, f, j) || (matches!(f.params.get(j).map(|p| &p.ty), Some(Type::Ref(_, Mode::Shared))) && self.g.reader_param(&k, j))) {
                    return None;
                }
                if args.iter().enumerate().any(|(i, o)| i != j && mentions(o, &s)) {
                    return None;
                }
                Some(s)
            }
            // `&mut s` handed to a builder as its first argument, or (D-0200)
            // to any writer parameter (`Gen::writer_param`), no other
            // argument naming `s`.
            fn builder_arg(&self, callee: &Expr, args: &[Expr], j: usize) -> Option<String> {
                let ExprKind::Borrow(Mode::Exclusive, x) = &strip_parens(&args[j]).kind else { return None };
                let s = path_name(x)?.to_string();
                let ExprKind::Path(segs, _) = &callee.kind else { return None };
                if segs.len() < 2 && segs.first().map_or(true, |n| self.bound.contains(n)) {
                    return None;
                }
                let k = segs.join("::");
                let k = if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
                let builder = j == 0 && BUILDERS.contains(&k.as_str()) && self.g.items.fns.get(&k).map_or(false, |f| in_prelude(f));
                if !builder && !self.g.writer_param(&k, j) {
                    return None;
                }
                if args.iter().enumerate().any(|(i, o)| i != j && mentions(o, &s)) {
                    return None;
                }
                Some(s)
            }
            // D-0200: `x` alone as argument `j`, moved: the parameter a value
            // (not a reference, a slice or a `fn`), or the callee a variant
            // (no function of that name); no other argument naming `x`.
            fn moved_arg(&self, callee: &Expr, args: &[Expr], j: usize) -> Option<String> {
                let s = path_name(&args[j])?.to_string();
                let ExprKind::Path(segs, _) = &callee.kind else { return None };
                if segs.len() == 1 && self.bound.contains(&segs[0]) {
                    return None;
                }
                if segs.len() == 1 && segs[0].starts_with('$') {
                    return None;
                }
                let k = segs.join("::");
                let k = if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
                match self.g.items.fns.get(&k) {
                    Some(f) => {
                        let pt = &f.params.get(j)?.ty;
                        if matches!(pt, Type::Ref(..) | Type::Slice(..) | Type::Fn(..)) {
                            return None;
                        }
                    }
                    None => {
                        // A variant constructor: a name that is no function
                        // (an intrinsic's name is not a variant's).
                        if segs.len() == 1 && !segs[0].starts_with(|ch: char| ch.is_ascii_uppercase()) {
                            return None;
                        }
                    }
                }
                if args.iter().enumerate().any(|(i, o)| i != j && mentions(o, &s)) {
                    return None;
                }
                Some(s)
            }
            // `return s` (control leaves, so nothing uses `s` after) or the
            // function body's last expression `s`: the value moves out,
            // given an object there (`read_place`).
            fn move_out(&mut self, e: &Expr) -> bool {
                if let ExprKind::Path(segs, t) = &strip_parens(e).kind {
                    if segs.len() == 1 && t.is_empty() {
                        if let Some(id) = self.id(&segs[0]) {
                            if id != 0 {
                                self.seen.insert(id);
                                self.moved.insert(id);
                                return true;
                            }
                        }
                    }
                }
                false
            }
            // A block inside the body: its last expression is not the
            // function's.
            fn nested(&mut self, b: &Block) {
                self.depth += 1;
                self.block(b);
                self.depth -= 1;
            }
            fn block(&mut self, b: &Block) {
                self.depth += 1;
                self.scopes.push(HashMap::new());
                for st in &b.stmts {
                    match st {
                        Stmt::Let { name, init, .. } => match init {
                            Some(e) => {
                                // D-0200: `let y = x;` moves `x` (into a binding
                                // of its own type, or a copy of plain data,
                                // which `read_place` gives no object either way).
                                if !self.move_out(e) {
                                    self.expr(e);
                                }
                                let id = &**e as *const Expr as usize;
                                self.names.insert(id, name.clone());
                                self.scopes.last_mut().unwrap().insert(name.clone(), id);
                            }
                            None => {
                                self.scopes.last_mut().unwrap().insert(name.clone(), 0);
                            }
                        },
                        Stmt::Destructure { names, init, .. } => {
                            self.expr(init);
                            for n in names {
                                self.scopes.last_mut().unwrap().insert(n.clone(), 0);
                            }
                        }
                        Stmt::Expr(e) | Stmt::BlockLike(e) => self.expr(e),
                    }
                }
                if let Some(t) = &b.tail {
                    if !(self.depth == 1 && self.move_out(t)) {
                        self.expr(t);
                    }
                }
                self.scopes.pop();
                self.depth -= 1;
            }
            fn expr(&mut self, e: &Expr) {
                match &e.kind {
                    ExprKind::Call(c, args) => {
                        let mut handled = vec![false; args.len()];
                        let fmt = matches!(&c.kind, ExprKind::Path(segs, _) if segs.len() == 1 && segs[0] == "$fmt");
                        // D-0200: a candidate moved away by value -- an
                        // argument of a by-value parameter, or of a variant
                        // constructor -- no other argument naming it.
                        let moves: Vec<Option<String>> = (0..args.len()).map(|j| self.moved_arg(c, args, j)).collect();
                        for j in 0..args.len() {
                            if let Some(s) = &moves[j] {
                                if let Some(id) = self.id(s) {
                                    if id != 0 {
                                        self.seen.insert(id);
                                        self.moved.insert(id);
                                        handled[j] = true;
                                        continue;
                                    }
                                }
                            }
                            // `printf`'s `&s` (`$fmt`): read where it is
                            // formatted (`lower_intrinsic`'s `$fmt`).
                            let f = (fmt && j > 0)
                                .then(|| match &strip_parens(&args[j]).kind {
                                    ExprKind::Borrow(Mode::Shared, x) => path_name(x).map(|n| n.to_string()),
                                    _ => None,
                                })
                                .flatten()
                                .filter(|n| args[1..].iter().filter(|o| mentions(o, n)).count() == 1);
                            if let Some(s) = f.or_else(|| self.reader_arg(c, args, j)).or_else(|| self.builder_arg(c, args, j)) {
                                if let Some(id) = self.id(&s) {
                                    if id != 0 {
                                        self.seen.insert(id);
                                    }
                                }
                                handled[j] = true;
                            }
                        }
                        self.expr(c);
                        for (j, a) in args.iter().enumerate() {
                            if !handled[j] {
                                self.expr(a);
                            }
                        }
                    }
                    ExprKind::Path(segs, _) => {
                        if segs.len() == 1 {
                            self.mark_bad(&segs[0]);
                        }
                    }
                    ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Unit | ExprKind::Dollar | ExprKind::Break | ExprKind::Continue | ExprKind::Return(None) => {}
                    ExprKind::StructLit(_, _, fs) => {
                        // D-0200: a candidate as a field's value moves in,
                        // no other field naming it.
                        for (i, (_, a)) in fs.iter().enumerate() {
                            let moved = path_name(a).filter(|x| !fs.iter().enumerate().any(|(k, (_, o))| k != i && mentions(o, x))).and_then(|x| self.id(x)).filter(|&id| id != 0);
                            match moved {
                                Some(id) => {
                                    self.seen.insert(id);
                                    self.moved.insert(id);
                                }
                                None => self.expr(a),
                            }
                        }
                    }
                    ExprKind::ArrayLit(es) => es.iter().for_each(|a| self.expr(a)),
                    ExprKind::Return(Some(a)) => {
                        if !self.move_out(a) {
                            self.expr(a);
                        }
                    }
                    ExprKind::Borrow(_, x) if matches!(&strip_parens(x).kind, ExprKind::Index(a, _) if path_name(a).is_some()) => {
                        let ExprKind::Index(a, i) = &strip_parens(x).kind else { unreachable!() };
                        self.mark_bad(path_name(a).unwrap());
                        self.expr(i);
                    }
                    ExprKind::ArrayRepeat(a, _) | ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => self.expr(a),
                    // D-0200: a candidate compared (`==`, `<`, …): read where it
                    // stands, for a `String` (`lower_text_operand`); marked
                    // (`id | 1`) so a binding of another type stays tracked.
                    ExprKind::Binary(op, a, b) if matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge) => {
                        for x in [a, b] {
                            match path_name(x).and_then(|n| self.id(n)).filter(|&id| id != 0) {
                                Some(id) => {
                                    self.seen.insert(id);
                                    self.compared.insert(id);
                                }
                                None => self.expr(x),
                            }
                        }
                    }
                    // D-0200: `x[i]` read, or written whole (`x[i] = v`), for a
                    // candidate: a bounds-checked element access of a vector
                    // nothing else reaches; marked (`id | 2`) so only a `Vec`
                    // of plain data stays objectless. A borrow of an element
                    // (`&x[i]`) is not this: it falls to `Borrow` below.
                    ExprKind::Index(a, i) if path_name(a).and_then(|n| self.id(n)).map_or(false, |id| id != 0) => {
                        let id = self.id(path_name(a).unwrap()).unwrap();
                        self.seen.insert(id);
                        self.indexed.insert(id);
                        self.expr(i);
                    }
                    ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
                        self.expr(a);
                        self.expr(b);
                    }
                    ExprKind::SliceOf(_, a, lo, hi) => {
                        self.expr(a);
                        self.expr(lo);
                        self.expr(hi);
                    }
                    ExprKind::Block(b) | ExprKind::Unsafe(b) => {
                        // D-0200: a fused text loop (`Gen::fusion_plan`) over
                        // `&x`: a read of `x` (`lower_fused_text_loop` lends
                        // it as the source only); its body as any block.
                        let scopes = &self.scopes;
                        if let Some(plan) = self.g.fusion_plan(b, &|n| scopes.iter().any(|m| m.contains_key(n))) {
                            if let ExprKind::Borrow(Mode::Shared, x) = &strip_parens(plan.source).kind {
                                if let Some(id) = path_name(x).and_then(|n| self.id(n)).filter(|&id| id != 0) {
                                    self.seen.insert(id);
                                    let ExprKind::Block(body) = &plan.be.kind else { unreachable!() };
                                    self.depth += 1;
                                    let mut sc = HashMap::new();
                                    sc.insert(plan.cname.clone(), 0);
                                    self.scopes.push(sc);
                                    self.nested(body);
                                    self.scopes.pop();
                                    self.depth -= 1;
                                    return;
                                }
                            }
                        }
                        self.nested(b)
                    }
                    ExprKind::If(c, t, e2) => {
                        self.expr(c);
                        self.nested(t);
                        if let Some(e2) = e2 {
                            self.expr(e2);
                        }
                    }
                    ExprKind::While(c, b, step) => {
                        self.expr(c);
                        self.nested(b);
                        if let Some(x) = step {
                            self.expr(x);
                        }
                    }
                    ExprKind::Match(sc, arms) => {
                        self.expr(sc);
                        for a in arms {
                            self.depth += 1;
                            self.scopes.push(HashMap::new());
                            // D-0200: an arm's binder is a candidate as a `let`
                            // is, by the arm's address (`Fx::lower_match`
                            // binds it objectless when it takes a payload
                            // of such a type by value).
                            if let Some(b) = &a.binder {
                                let id = a as *const Arm as usize;
                                self.names.insert(id, b.clone());
                                self.scopes.last_mut().unwrap().insert(b.clone(), id);
                            }
                            if let Some(l) = &a.lit {
                                self.expr(l);
                            }
                            self.expr(&a.body);
                            self.scopes.pop();
                            self.depth -= 1;
                        }
                    }
                    ExprKind::Closure { captures, params, body, .. } => {
                        for c in captures {
                            self.mark_bad(c);
                        }
                        // Every name the body uses, resolved out here: a
                        // closure's own parameters shadow nothing it sees.
                        let pnames: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
                        let mut used: Vec<String> = Vec::new();
                        visit_exprs(body, &mut |x| {
                            if let ExprKind::Path(s, _) = &x.kind {
                                if s.len() == 1 && !pnames.contains(&s[0]) {
                                    used.push(s[0].clone());
                                }
                            }
                        });
                        for n in used {
                            self.mark_bad(&n);
                        }
                    }
                }
            }
        }
        let bound: HashSet<String> = bound_names(body).into_iter().chain(params.iter().cloned()).collect();
        let top: HashMap<String, usize> = params.iter().map(|p| (p.clone(), 0)).collect();
        let mut w = W { g: self, bound, scopes: vec![top], bad: HashSet::new(), seen: HashSet::new(), moved: HashSet::new(), compared: HashSet::new(), indexed: HashSet::new(), names: HashMap::new(), depth: 0 };
        w.block(body);
        // A candidate moved away is recorded twice (`id` and `!id`): its
        // binding gets a moved flag (`Fx::objectless_bound`).
        w.seen
            .iter()
            .filter(|x| !w.bad.contains(*x))
            .flat_map(|&x| {
                let mut v = vec![x];
                if w.moved.contains(&x) {
                    v.push(!x);
                }
                if w.compared.contains(&x) {
                    v.push(x | 1);
                }
                if w.indexed.contains(&x) {
                    v.push(x | 2);
                }
                v
            })
            .collect()
    }

    // D-0200: a type whose value an objectless local may hold: built only
    // from `std`'s containers (`String`, `Vec`, `Queue`, `HashMap`,
    // `HashSet`) and structs that are not declared `resource`, over plain
    // data -- no reference inside (its slots would need an object to end
    // with), no `fn` value, no handle, mutex or guard, no destructor of
    // the program's own. Its end is `cb_drop_local`'s `[Destroy]`.
    pub(super) fn objectless_type(&self, t: &Type) -> bool {
        self.objectless_type_in(t, &mut Vec::new())
    }

    // `objectless_type`, `seen` the struct types being answered: a type
    // holding itself (`struct N { Vec<N> kids; }`) is answered no there.
    pub(super) fn objectless_type_in(&self, t: &Type, seen: &mut Vec<Type>) -> bool {
        if seen.contains(t) {
            return false;
        }
        let elem = |x: &Type, seen: &mut Vec<Type>| (self.plain_data(x) && !self.has_refs(x) && !self.holds_fn(x)) || self.objectless_type_in(x, seen);
        match t {
            Type::Named(n, a) if n == "std::String" && a.is_empty() => true,
            Type::Named(n, a) if (n == "std::Vec" || n == "std::Queue" || n == "std::HashSet") && a.len() == 1 => elem(&a[0], seen),
            Type::Named(n, a) if n == "std::HashMap" && a.len() == 2 => elem(&a[0], seen) && elem(&a[1], seen),
            Type::Named(n, a) if !n.starts_with("std::") => match self.struct_decl(n) {
                Some(sd) if !sd.resource && sd.bits.is_none() => {
                    let sub = subst_of(&sd.type_params, a);
                    seen.push(t.clone());
                    let ok = sd.fields.iter().all(|f| elem(&apply(&f.ty, &sub), seen));
                    seen.pop();
                    ok
                }
                _ => false,
            },
            _ => false,
        }
    }

    // D-0196: whether `key` gets a `__raw` form: a function of the program
    // (not `std`'s, not generic, not a dual body) returning a `String` whose
    // every result -- each `return x` and its last expression -- is a local
    // declared once that the analysis makes objectless.
    pub(super) fn raw_ret_ok(&self, key: &str) -> bool {
        if self.boot(4) {
            return false;
        }
        let Some(f) = self.items.fns.get(key) else { return false };
        // D-0200: any non-generic function of the program returning a value
        // an objectless local may hold (`objectless_type`): its `__raw`
        // form ends its result's record where it returns (`emit_return`)
        // rather than sending it, and the caller binds the value objectless.
        // And, since D-0200's later round, `std`'s own non-generic functions
        // (a native body never reaches here: it returns before).
        if !f.type_params.is_empty() || key == "main" || !self.objectless_type(&f.ret) {
            return false;
        }
        if self.dual_eligible(f, &HashMap::new()).ok().flatten().is_some() || self.field_dual_eligible(f, &HashMap::new()).ok().flatten().is_some() {
            return false;
        }
        true
    }

    pub(super) fn pure_direct(&self, key: &str, pos: usize) -> bool {
        if self.all_checks {
            return false;
        }
        let visiting = std::cell::RefCell::new(Vec::new());
        self.pure_direct_in(key, pos, &visiting)
    }

    // A call `segs(…)` whose argument `pos` is a pure direct reference
    // parameter (`pure_direct`), named so that no local can be what it
    // calls (two segments or more: `Type::f`, `m::f`; or one that no
    // binding of the function, `bound`, has). `&x` passed there mints no
    // path on `x` (`lower_user_call`), so it does not pin `x`
    // (`pinned_names`).
    pub(super) fn pure_direct_ref_arg(&self, segs: &[String], pos: usize, bound: &HashSet<String>) -> bool {
        if segs.len() < 2 && segs.first().map_or(true, |s| bound.contains(s)) {
            return false;
        }
        let key = segs.join("::");
        let Some(f) = self.items.fns.get(&key) else { return false };
        matches!(f.params.get(pos).map(|p| &p.ty), Some(Type::Ref(..))) && self.pure_direct(&key, pos)
    }
    pub(super) fn pure_direct_in(&self, key: &str, pos: usize, visiting: &std::cell::RefCell<Vec<(String, usize)>>) -> bool {
        // D-0192: the key of `std`'s searches is only read (`==`, or
        // `key_before`'s `<`), never kept, in their bodies and their native
        // forms alike (a generic body, which the rule below cannot answer
        // for). Not `binary_search_by`, whose key goes to the caller's own
        // function.
        if pos == 1 && matches!(key, "std::Vec::contains" | "std::Vec::index_of" | "std::Vec::binary_search") {
            return self.items.fns.get(key).map_or(false, |f| in_prelude(f));
        }
        let id = (key.to_string(), pos);
        if let Some(&b) = self.pure_memo.borrow().get(&id) {
            return b;
        }
        if visiting.borrow().contains(&id) {
            return false;
        }
        let Some(f) = self.items.fns.get(key) else { return false };
        // A generic function qualifies when no parameter's type names a
        // type parameter: then which parameters are direct is the same in
        // every instance. A slice parameter, which the caller keeps
        // instead of sending, also needs every call to be one the
        // compiler sees: the function never used as a `fn` value.
        if pos >= f.params.len() || !f.params.iter().all(|p| resolved(&p.ty, &f.type_params)) {
            return false;
        }
        match &f.params[pos].ty {
            Type::Ref(..) => {}
            Type::Slice(..) if !self.used_as_value(key) => {}
            _ => return false,
        }
        let name = f.params[pos].name.clone();
        if !self.direct_ref_params(f, &HashMap::new()).map_or(false, |d| d.contains(&name)) {
            self.pure_memo.borrow_mut().insert(id, false);
            return false;
        }
        // A struct reached through `Box`es (`box_rec_uses`): never handed
        // on, so pure.
        if box_rec_uses(&f.body, &name) {
            self.pure_memo.borrow_mut().insert(id, true);
            return true;
        }
        visiting.borrow_mut().push(id.clone());
        let my_mode = match &f.params[pos].ty {
            Type::Ref(_, m) | Type::Slice(_, m) => Some(m.clone()),
            _ => None,
        };
        let ok = direct_uses(&f.body, &name, &|segs: &[String], at: usize| {
            let k = segs.join("::");
            let k = if self.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
            // Handed on in the same mode only: an exclusive reference
            // passed where a shared one is wanted is reborrowed from its
            // token, and a pure parameter's token may be none at all.
            let same_mode = self.items.fns.get(&k).and_then(|g| g.params.get(at)).map_or(false, |q| match &q.ty {
                Type::Ref(_, m) | Type::Slice(_, m) => Some(m.clone()) == my_mode,
                _ => false,
            });
            same_mode && self.pure_direct_in(&k, at, visiting)
        });
        visiting.borrow_mut().pop();
        if visiting.borrow().is_empty() || ok {
            self.pure_memo.borrow_mut().insert(id, ok);
        }
        ok
    }
}
