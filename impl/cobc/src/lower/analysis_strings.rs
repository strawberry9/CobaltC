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
        self.objectless_strings_with(body, params, &[], &mut HashMap::new())
    }

    // `objectless_strings`, the parameters `cands` (name, id) candidates as
    // a `let` with an initializer is (D-0286: a decided program's by-value
    // parameters, which arrive with no object).
    // In a decided program (D-0286) a candidate may also be borrowed whole
    // or by a field anywhere (the borrow is its address, token 0), and its
    // fields read; `fields` gets, by candidate, the fields read by value,
    // which `Fx::objectless_binding` allows only of plain data (no field
    // is moved out of an objectless local).
    pub(super) fn objectless_strings_with(&self, body: &Block, params: &[String], cands: &[(String, usize)], fields: &mut HashMap<usize, HashSet<String>>) -> HashSet<usize> {
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
            // D-0294: the candidates assigned whole (`x = e`, decided).
            assigned: HashSet<usize>,
            names: HashMap<usize, String>,
            depth: usize,
            fields: HashMap<usize, HashSet<String>>,
            // D-0286: the next block walked is the function's last
            // expression (`unsafe { … x }` ending the body): its own last
            // expression is the function's result too.
            tail_next: bool,
            // D-0315 (step 4): every name the body mentions (a path or a
            // closure's capture): a `let` of none of them is unused.
            mentioned: HashSet<String>,
            // `COBC_DEBUG_W`: the expression walked, and the one it sits in.
            parent: String,
            outer: String,
            // `COBC_DEBUG_W`: the callee of the call being walked.
            callee: String,
            dbg: bool,
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
            // D-0286: `x`, `x.f`, `x.f.g`…: the candidate at its root (not
            // 0), and the first field named, if any.
            fn field_root(&self, e: &Expr) -> Option<(usize, Option<String>)> {
                let mut e = strip_parens(e);
                let mut first = None;
                while let ExprKind::Field(a, f) = &e.kind {
                    first = Some(f.clone());
                    e = strip_parens(a);
                }
                let ExprKind::Path(segs, t) = &e.kind else { return None };
                if segs.len() != 1 || !t.is_empty() {
                    return None;
                }
                self.id(&segs[0]).filter(|&id| id != 0).map(|id| (id, first))
            }
            // D-0315: `x.f`, `x.f.g`, `x.f[i].g`…: the candidate at the root,
            // the path from it (`f`, `f.g`, `f[].g`: a `Vec` field's element by
            // `[]`), and the index expressions on the way; None unless a field
            // comes first.
            fn place_chain<'e>(&self, e: &'e Expr) -> Option<(usize, String, Vec<&'e Expr>)> {
                let mut e = strip_parens(e);
                let mut segs: Vec<String> = Vec::new();
                let mut idx: Vec<&'e Expr> = Vec::new();
                loop {
                    match &e.kind {
                        ExprKind::Field(a, f) => {
                            segs.push(format!(".{}", f));
                            e = strip_parens(a);
                        }
                        ExprKind::Index(a, i) => {
                            segs.push("[]".to_string());
                            idx.push(i);
                            e = strip_parens(a);
                        }
                        _ => break,
                    }
                }
                let ExprKind::Path(p, t) = &e.kind else { return None };
                if p.len() != 1 || !t.is_empty() {
                    return None;
                }
                let id = self.id(&p[0]).filter(|&id| id != 0)?;
                segs.reverse();
                if !segs.first().map_or(false, |s| s.starts_with('.')) {
                    return None;
                }
                let chain = segs.concat();
                idx.reverse();
                Some((id, chain[1..].to_string(), idx))
            }
            // D-0315 (step 4): whether parameter `j` of the function `c` names
            // is a `fn` value.
            fn fn_param_at(&self, c: &Expr, j: usize) -> bool {
                let ExprKind::Path(segs, _) = &c.kind else { return false };
                if segs.len() == 1 && self.id(&segs[0]).is_some() {
                    return false;
                }
                let k = segs.join("::");
                let k = if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
                self.g.items.fns.get(&k).and_then(|f| f.params.get(j)).map_or(false, |p| matches!(p.ty, Type::Fn(..)))
            }
            fn mark_bad(&mut self, n: &str) {
                if let Some(id) = self.id(n) {
                    if id != 0 {
                        if self.dbg && !self.bad.contains(&id) {
                            let callee = if self.outer == "Call" { format!("({})", self.callee) } else { String::new() };
                            eprintln!("W bad {} in {}{} id {}", n, self.outer, callee, id);
                        }
                        self.bad.insert(id);
                    }
                }
            }
            fn reader_arg(&self, callee: &Expr, args: &[Expr], j: usize) -> Option<String> {
                // D-0302's whole-text view `&s[0..$]` (`&s` where a
                // `StringView` is declared), for the unminted view
                // `lower_direct_arg` makes of it (below).
                let (x, whole) = match &strip_parens(&args[j]).kind {
                    ExprKind::Borrow(Mode::Shared, x) => (x, false),
                    ExprKind::SliceOf(Mode::Shared, x, lo, hi) if matches!(&lo.kind, ExprKind::IntLit(0, _)) && matches!(&hi.kind, ExprKind::Dollar) => (x, true),
                    _ => return None,
                };
                let s = path_name(x)?.to_string();
                let ExprKind::Path(segs, _) = &callee.kind else { return None };
                if segs.len() < 2 && segs.first().map_or(true, |n| self.bound.contains(n)) {
                    return None;
                }
                let k = segs.join("::");
                let k = if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
                let f = self.g.items.fns.get(&k)?;
                // D-0308: `&s[0..$]` for a `StringView` parameter that keeps
                // nothing of the view (`pure_view`), the call's other
                // parameters views or plain data and its other arguments
                // inert (`inert_view_arg`): the view is unminted
                // (`lower_as_view_unminted`), `s` only read, as `String::len(&s)` was.
                if whole {
                    let plain = |t: &Type| matches!(t, Type::Named(n, a) if n == "std::StringView" && a.is_empty()) || self.g.plain_data(t);
                    if !(self.g.pure_view(&k, j) && f.params.iter().all(|p| plain(&p.ty)) && args.iter().enumerate().all(|(i, o)| i == j || inert_view_arg(o))) {
                        return None;
                    }
                } else if !(appended_text(&k, f, j) || (matches!(f.params.get(j).map(|p| &p.ty), Some(Type::Ref(_, Mode::Shared))) && self.g.reader_param(&k, j))) {
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
            // D-0315 (decided): a `foreach` holder `d` read or taken from by
            // `$each_len(d)`, `$each_take(d, …)` or `$each_take_key(d, …)`,
            // whose expansions borrow it (`each.rs`).
            fn each_holder_arg(&self, callee: &Expr, args: &[Expr], j: usize) -> Option<String> {
                let ExprKind::Path(segs, _) = &callee.kind else { return None };
                if !self.g.decided || j != 0 || segs.len() != 1 || !matches!(segs[0].as_str(), "$each_len" | "$each_take" | "$each_take_key") {
                    return None;
                }
                let s = path_name(&args[0])?.to_string();
                if args[1..].iter().any(|o| mentions(o, &s)) {
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
                // D-0315 (decided): `$each_drain(e, n)` moves `e` into the
                // holder `Vec::drain(e)` is.
                if segs.len() == 1 && segs[0] == "$each_drain" && j == 0 && self.g.decided && !args[1..].iter().any(|o| mentions(o, &s)) {
                    return Some(s);
                }
                // D-0315 (decided): `drop(x)` ends `x` where it is
                // (`lower_intrinsic_runtime`'s objectless case).
                if segs.len() == 1 && segs[0] == "drop" && args.len() == 1 && self.g.decided && self.g.items.fns.get("drop").is_none() && self.g.items.fns.get("std::drop").is_none() {
                    return Some(s);
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
            fn match_arms(&mut self, arms: &[Arm]) {
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
                    // D-0294 (decided): an arm whose body is a candidate
                    // alone moves it out, as a block's tail does.
                    if !(self.g.decided && self.move_out(&a.body)) {
                        self.expr(&a.body);
                    }
                    self.scopes.pop();
                    self.depth -= 1;
                }
            }
            // A block inside the body: its last expression is not the
            // function's.
            fn nested(&mut self, b: &Block) {
                self.depth += 1;
                self.block(b);
                self.depth -= 1;
            }
            fn block(&mut self, b: &Block) {
                let tail_here = std::mem::take(&mut self.tail_next);
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
                                // D-0315 (step 4): unused, it only ends.
                                if self.g.decided && !self.mentioned.contains(name) {
                                    self.seen.insert(id);
                                }
                            }
                            // D-0315 (step 4): with no value, a candidate by the
                            // statement's address in a decided program, moved
                            // (its flag) until assigned.
                            None if self.g.decided => {
                                let id = st as *const Stmt as usize;
                                self.names.insert(id, name.clone());
                                self.scopes.last_mut().unwrap().insert(name.clone(), id);
                                self.seen.insert(id);
                                self.moved.insert(id);
                            }
                            None => {
                                self.scopes.last_mut().unwrap().insert(name.clone(), 0);
                            }
                        },
                        // D-0315 (decided): as a `let` is, each binder (`fields`;
                        // `names` are the struct's fields) a candidate by its
                        // name's address (`destructure_key`).
                        Stmt::Destructure { fields, init, .. } if self.g.decided => {
                            if !self.move_out(init) {
                                self.expr(init);
                            }
                            for n in fields {
                                let id = destructure_key(n);
                                self.names.insert(id, n.clone());
                                self.scopes.last_mut().unwrap().insert(n.clone(), id);
                                // CHG-0440: unused, it only ends, as a `let`'s
                                // binding does (D-0315 step 4).
                                if !self.mentioned.contains(n) {
                                    self.seen.insert(id);
                                }
                            }
                        }
                        Stmt::Destructure { fields, init, .. } => {
                            self.expr(init);
                            for n in fields {
                                self.scopes.last_mut().unwrap().insert(n.clone(), 0);
                            }
                        }
                        Stmt::Expr(e) | Stmt::BlockLike(e) => self.expr(e),
                    }
                }
                if let Some(t) = &b.tail {
                    let at_tail = self.depth == 1 || (tail_here && self.g.decided);
                    let inner = match &strip_parens(t).kind {
                        ExprKind::Block(ib) | ExprKind::Unsafe(ib) if at_tail => {
                            let scopes = &self.scopes;
                            self.g.fusion_plan(ib, &|n| scopes.iter().any(|m| m.contains_key(n))).is_none().then_some(ib)
                        }
                        _ => None,
                    };
                    if let Some(ib) = inner.filter(|_| self.g.decided) {
                        self.tail_next = true;
                        self.nested(ib);
                    } else if !((at_tail || self.g.decided) && self.move_out(t)) {
                        // (D-0294: in a decided program any block's tail that
                        // is a candidate alone moves it out, `read_place`.)
                        self.expr(t);
                    }
                }
                self.scopes.pop();
                self.depth -= 1;
            }
            fn expr(&mut self, e: &Expr) {
                let outer = if self.dbg {
                    let d = format!("{:?}", e.kind);
                    let k = d.find(|c: char| c == '(' || c == ' ' || c == '{').unwrap_or(d.len());
                    let o = std::mem::replace(&mut self.parent, d[..k].to_string());
                    self.outer = o.clone();
                    Some(o)
                } else {
                    None
                };
                self.expr_in(e);
                if let Some(o) = outer {
                    self.parent = o;
                }
            }
            fn expr_in(&mut self, e: &Expr) {
                match &e.kind {
                    ExprKind::Call(c, args) => {
                        if self.dbg {
                            self.callee = match &c.kind {
                                ExprKind::Path(segs, _) => segs.join("::"),
                                _ => "?".to_string(),
                            };
                        }
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
                            if let Some(s) = f.or_else(|| self.reader_arg(c, args, j)).or_else(|| self.builder_arg(c, args, j)).or_else(|| self.each_holder_arg(c, args, j)) {
                                if let Some(id) = self.id(&s) {
                                    if id != 0 {
                                        self.seen.insert(id);
                                    }
                                }
                                handled[j] = true;
                            }
                        }
                        // D-0315 (step 4): `spawn`'s by-value arguments move into
                        // the thread, its borrowed ones are read (lent);
                        // `join` and `try_join` consume their handle (decided).
                        if self.g.decided {
                            if let ExprKind::Path(s, _) = &c.kind {
                                let name = if s.len() == 1 { s[0].as_str() } else { "" };
                                for (j, a) in args.iter().enumerate() {
                                    let moves = (name == "spawn" && j > 0) || ((name == "join" || name == "try_join") && j == 0) || (name == "lock" && j == 0);
                                    if !moves || handled[j] || self.id(name).is_some() {
                                        continue;
                                    }
                                    match &strip_parens(a).kind {
                                        ExprKind::Path(p, t) if name != "lock" && p.len() == 1 && t.is_empty() && self.id(&p[0]).map_or(false, |id| id != 0) => {
                                            let id = self.id(&p[0]).unwrap();
                                            self.seen.insert(id);
                                            self.moved.insert(id);
                                            handled[j] = true;
                                        }
                                        ExprKind::Borrow(_, x) if (name == "spawn" || name == "lock") && path_name(x).and_then(|n| self.id(n)).map_or(false, |id| id != 0) => {
                                            let id = self.id(path_name(x).unwrap()).unwrap();
                                            self.seen.insert(id);
                                            handled[j] = true;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        // D-0315 (step 4): a call of a closure held in a candidate
                        // reads it (decided).
                        match &c.kind {
                            ExprKind::Path(s, t) if self.g.decided && s.len() == 1 && t.is_empty() && self.id(&s[0]).map_or(false, |id| id != 0) => {
                                let id = self.id(&s[0]).unwrap();
                                self.seen.insert(id);
                            }
                            _ => self.expr(c),
                        }
                        for (j, a) in args.iter().enumerate() {
                            // D-0315 (step 4): a candidate given to a `fn`
                            // parameter is copied or moved into it (decided).
                            if !handled[j] && self.g.decided && self.fn_param_at(c, j) {
                                if let Some(id) = path_name(a).and_then(|n| self.id(n)).filter(|&id| id != 0) {
                                    self.seen.insert(id);
                                    self.moved.insert(id);
                                    continue;
                                }
                            }
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
                    // D-0294 (decided): `&x[i]` of a candidate is a borrow of
                    // `x` (its address, token 0) and a bounds check.
                    ExprKind::Borrow(_, x) if self.g.decided && matches!(&strip_parens(x).kind, ExprKind::Index(a, _) if path_name(a).and_then(|n| self.id(n)).map_or(false, |id| id != 0)) => {
                        let ExprKind::Index(a, i) = &strip_parens(x).kind else { unreachable!() };
                        let id = self.id(path_name(a).unwrap()).unwrap();
                        self.seen.insert(id);
                        self.expr(i);
                    }
                    ExprKind::Borrow(_, x) if matches!(&strip_parens(x).kind, ExprKind::Index(a, _) if path_name(a).is_some()) => {
                        let ExprKind::Index(a, i) = &strip_parens(x).kind else { unreachable!() };
                        self.mark_bad(path_name(a).unwrap());
                        self.expr(i);
                    }
                    // D-0315 (decided): a borrow of a place reached from a
                    // candidate through a field (`&x.f[i]`, `&x.f[i].g`): a
                    // borrow of the field (its address, token 0) and its indexes.
                    ExprKind::Borrow(_, x) if self.g.decided && self.place_chain(x).is_some() => {
                        let (id, _, idx) = self.place_chain(x).unwrap();
                        self.seen.insert(id);
                        for i in idx {
                            self.expr(i);
                        }
                    }
                    // D-0286: in a decided program, a borrow of a candidate or
                    // of a field of one (its address, token 0), and a field
                    // read (recorded: only plain data may be read so).
                    ExprKind::Borrow(_, x) if self.g.decided && self.field_root(x).is_some() => {
                        let (id, _) = self.field_root(x).unwrap();
                        self.seen.insert(id);
                    }
                    // D-0315: by its whole path (`f.g`, `f[].g`), what it reaches
                    // judged by `Fx::objectless_binding`.
                    ExprKind::Field(..) | ExprKind::Index(..) if self.g.decided && self.place_chain(e).is_some() => {
                        let (id, chain, idx) = self.place_chain(e).unwrap();
                        self.seen.insert(id);
                        self.fields.entry(id).or_default().insert(chain);
                        for i in idx {
                            self.expr(i);
                        }
                    }
                    // D-0294: in a decided program a field of a candidate
                    // written (recorded as `=f`: `Fx::objectless_binding`
                    // allows it of plain data and quiet types, whose old
                    // value the write destroys, `lower_assign`), or the
                    // candidate assigned whole (`id | 4`, a quiet type).
                    ExprKind::Assign(a, b) if self.g.decided && matches!(strip_parens(a).kind, ExprKind::Field(..)) && self.field_root(a).is_some() => {
                        let (id, f) = self.field_root(a).unwrap();
                        self.seen.insert(id);
                        if let Some(f) = f {
                            self.fields.entry(id).or_default().insert(format!("={}", f));
                        }
                        self.expr(b);
                    }
                    ExprKind::Assign(a, b) if self.g.decided && path_name(a).and_then(|n| self.id(n)).map_or(false, |id| id != 0) => {
                        let id = self.id(path_name(a).unwrap()).unwrap();
                        self.seen.insert(id);
                        self.assigned.insert(id);
                        if !(path_name(b).map_or(false, |n| Some(n) != path_name(a)) && self.move_out(b)) {
                            self.expr(b);
                        }
                    }
                    // D-0294 (decided): a candidate moved into a place
                    // (`p.f = x`), the place not naming it; `x?`; `&x[a..b]`, a
                    // read of `x` (its slice carries token 0).
                    ExprKind::Assign(a, b) if self.g.decided && path_name(b).map_or(false, |n| !mentions(a, n)) && self.move_out(b) => {
                        self.expr(a);
                    }
                    ExprKind::Propagate(a) if self.g.decided && self.move_out(a) => {}
                    ExprKind::SliceOf(_, a, lo, hi) if self.g.decided && path_name(a).and_then(|n| self.id(n)).map_or(false, |id| id != 0) => {
                        let id = self.id(path_name(a).unwrap()).unwrap();
                        self.seen.insert(id);
                        self.expr(lo);
                        self.expr(hi);
                    }
                    // D-0315 (decided): `&x.f[a..b]` (a view of a `String`
                    // field, a text coercion's): a borrow of the field.
                    ExprKind::SliceOf(_, a, lo, hi) if self.g.decided && self.place_chain(a).is_some() => {
                        let (id, _, idx) = self.place_chain(a).unwrap();
                        self.seen.insert(id);
                        for i in idx {
                            self.expr(i);
                        }
                        self.expr(lo);
                        self.expr(hi);
                    }
                    // D-0294 (decided): a candidate matched in place
                    // (`lower_match`'s place scrutinee): moved away when an arm
                    // binds a payload (the arm sets its flag).
                    // D-0315: an arm binding the whole value (`other : …`) is
                    // recorded `*whole`: an enum's match moves it into the
                    // binder as a payload; another type's (`lower_match_scalar`)
                    // needs the binding's object (`field_entry_ok`).
                    ExprKind::Match(sc, arms) if self.g.decided && path_name(sc).and_then(|n| self.id(n)).map_or(false, |id| id != 0) => {
                        let id = self.id(path_name(sc).unwrap()).unwrap();
                        if arms.iter().any(|a| a.variant.is_none() && a.binder.is_some()) {
                            self.fields.entry(id).or_default().insert("*whole".to_string());
                        }
                        self.seen.insert(id);
                        if arms.iter().any(|a| a.binder.is_some()) {
                            self.moved.insert(id);
                        }
                        self.match_arms(arms);
                    }
                    // D-0315 (step 4): `*x` of a candidate reads it (decided).
                    ExprKind::Deref(a) if self.g.decided && path_name(a).and_then(|n| self.id(n)).map_or(false, |id| id != 0) => {
                        let id = self.id(path_name(a).unwrap()).unwrap();
                        self.seen.insert(id);
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
                                // D-0315 (decided): a field compared, recorded `~f`
                                // (a `String` one is read where it stands).
                                None if self.g.decided && self.place_chain(x).is_some() => {
                                    let (id, chain, idx) = self.place_chain(x).unwrap();
                                    self.seen.insert(id);
                                    self.fields.entry(id).or_default().insert(format!("~{}", chain));
                                    for i in idx {
                                        self.expr(i);
                                    }
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
                        self.match_arms(arms);
                    }
                    ExprKind::Closure { captures, params, body, is_move, .. } => {
                        for c in captures {
                            // D-0315 (step 4): captured by reference in a
                            // decided program, read where it is (no path); by
                            // `move`, moved into the closure.
                            match self.id(c).filter(|&id| id != 0 && self.g.decided) {
                                Some(id) => {
                                    self.seen.insert(id);
                                    if *is_move {
                                        self.moved.insert(id);
                                    }
                                }
                                None => self.mark_bad(c),
                            }
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
                            // (A captured name's use is the capture's.)
                            if self.g.decided && captures.contains(&n) {
                                continue;
                            }
                            self.mark_bad(&n);
                        }
                    }
                }
            }
        }
        let bound: HashSet<String> = bound_names(body).into_iter().chain(params.iter().cloned()).collect();
        let top: HashMap<String, usize> = params.iter().map(|p| (p.clone(), cands.iter().find(|(n, _)| n == p).map_or(0, |c| c.1))).collect();
        let mut w = W { g: self, bound, scopes: vec![top], bad: HashSet::new(), seen: HashSet::new(), moved: HashSet::new(), compared: HashSet::new(), indexed: HashSet::new(), assigned: HashSet::new(), names: HashMap::new(), depth: 0, fields: HashMap::new(), tail_next: false, mentioned: if self.decided { mentioned_names(body) } else { HashSet::new() }, parent: String::new(), outer: String::new(), callee: String::new(), dbg: std::env::var_os("COBC_DEBUG_W").is_some() };
        w.block(body);
        *fields = std::mem::take(&mut w.fields);
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
                if w.assigned.contains(&x) {
                    v.push(x | 4);
                }
                v
            })
            .collect()
    }

    // D-0315: `std`'s types whose destructor frees memory alone: the elements
    // a drain has not given out, then its buffer.
    pub(super) const MEMORY_ONLY_DROPS: &'static [&'static str] = &["std::Drain", "std::HashDrain"];

    // D-0200: a type whose value an objectless local may hold: built only
    // from `std`'s containers (`String`, `Vec`, `Queue`, `HashMap`,
    // `HashSet`) and structs that are not declared `resource`, over plain
    // data -- no reference inside (its slots would need an object to end
    // with), no `fn` value, no handle, mutex or guard, no destructor of
    // the program's own. Its end is `cb_drop_local`'s `[Destroy]`.
    pub(super) fn objectless_type(&self, t: &Type) -> bool {
        self.objectless_type_in(t, &mut Vec::new())
    }

    // D-0315 (step 4): a type a noted value may hold: `objectless_type`'s,
    // with the `resource` structs and enums that have a destructor of their
    // own admitted, the program's and `std`'s (a fault must run it: the
    // value is noted).
    // D-0315 (step 4): whether the `fn` parameter `j` of the function `key`
    // does not escape its body: every mention of it is a call of it, or a
    // by-value argument to such a parameter of another function (a cycle
    // answers no).
    pub(super) fn fn_param_noescape(&self, key: &str, j: usize) -> bool {
        let k = (key.to_string(), j);
        if let Some(&r) = self.noescape.borrow().get(&k) {
            return r;
        }
        self.noescape.borrow_mut().insert(k.clone(), false);
        let r = self.fn_param_noescape_in(key, j);
        self.noescape.borrow_mut().insert(k, r);
        r
    }

    fn fn_param_noescape_in(&self, key: &str, j: usize) -> bool {
        let Some(f) = self.items.fns.get(key) else { return false };
        let Some(p) = f.params.get(j) else { return false };
        if !matches!(p.ty, Type::Fn(..)) {
            return false;
        }
        let x = p.name.clone();
        let mut mentions = 0usize;
        let mut ok = 0usize;
        let mut passes: Vec<(String, usize)> = Vec::new();
        visit_exprs(&f.body, &mut |e| match &e.kind {
            ExprKind::Path(s, _) if s.len() == 1 && s[0] == x => mentions += 1,
            ExprKind::Call(c, args) => {
                if matches!(&c.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == x) {
                    ok += 1;
                }
                if let ExprKind::Path(segs, _) = &c.kind {
                    for (i, a) in args.iter().enumerate() {
                        if matches!(&strip_parens(a).kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == x) {
                            let g = segs.join("::");
                            let g = if self.items.fns.contains_key(&g) { g } else { format!("std::{}", g) };
                            passes.push((g, i));
                        }
                    }
                }
            }
            ExprKind::Closure { captures, .. } => mentions += captures.iter().filter(|c| **c == x).count(),
            _ => {}
        });
        let passed_ok = passes.iter().all(|(g, i)| self.fn_param_noescape(g, *i));
        mentions > 0 && passed_ok && mentions == ok + passes.len()
    }

    pub(super) fn noted_type(&self, t: &Type) -> bool {
        self.noting.set(true);
        let r = self.objectless_type_in(t, &mut Vec::new());
        self.noting.set(false);
        r
    }

    // `objectless_type`, `seen` the struct and enum types being answered.
    // D-0294: a type met again on the walk (`struct N { Vec<N> kids; }`, a
    // syntax tree through `Box`) is answered yes there: the whole's answer
    // is its other parts' (coinductively; before D-0294 it was no, and a
    // recursive type never objectless).
    // CHG-0429: an answer is kept (`ol_memo`) where it holds wherever it is
    // asked: a no (made with every type still being answered taken as yes,
    // it is no under fewer of them too), and a yes that leaned on no type
    // being answered outside its own walk (`ol_low`). A type that names a
    // closure is answered afresh each time: the closures' table grows as
    // the lowering goes.
    pub(super) fn objectless_type_in(&self, t: &Type, seen: &mut Vec<Type>) -> bool {
        if let Some(i) = seen.iter().position(|x| x == t) {
            if i < self.ol_low.get() {
                self.ol_low.set(i);
            }
            return true;
        }
        if names_closure(t) {
            return self.objectless_type_walk(t, seen);
        }
        let key = (t.clone(), self.noting.get(), self.decided);
        if let Some(&r) = self.ol_memo.borrow().get(&key) {
            return r;
        }
        let depth = seen.len();
        let outer = self.ol_low.replace(usize::MAX);
        let r = self.objectless_type_walk(t, seen);
        let low = self.ol_low.get();
        if !r || low >= depth {
            self.ol_memo.borrow_mut().insert(key, r);
        }
        self.ol_low.set(if low < depth { outer.min(low) } else { outer });
        r
    }

    fn objectless_type_walk(&self, t: &Type, seen: &mut Vec<Type>) -> bool {
        // D-0294: in a decided program no reference inside a value is
        // tracked (`has_refs`), so a value with no resource and no `fn` in
        // it is as plain data here.
        // D-0315: a `void` part (`Result<void, String>`) holds nothing.
        // D-0315 (step 4): a noted value may hold a `fn` value (its end is
        // its box's, `drop_fn_handle`).
        let elem = |x: &Type, seen: &mut Vec<Type>| matches!(x, Type::Void) || (self.noting.get() && matches!(x, Type::Fn(..))) || (self.plain_data(x) && !self.has_refs(x) && !self.holds_fn(x)) || (self.decided && self.decided_plain(x)) || self.objectless_type_in(x, seen);
        match t {
            Type::Named(n, a) if n == "std::String" && a.is_empty() => true,
            // D-0315 (step 4): a thread's handle, for a noted value (its end
            // is `[Handle-Destructor]`); an array, by its elements.
            Type::Handle(_) if self.decided && self.noting.get() => true,
            // D-0315 (step 4): a mutex by its interior; a lock's guard, noted
            // (its end releases the lock).
            Type::Mutex(inner) if self.decided => elem(inner, seen),
            Type::Guard(_) if self.decided && self.noting.get() => true,
            Type::Array(inner, _) if self.decided => elem(inner, seen),
            // D-0315 (step 4): in a decided program a closure, by its captures
            // (a reference is plain there).
            Type::Closure(id) if self.decided => match self.closures.get(id) {
                Some(info) => info.fields.iter().all(|(_, ft)| elem(ft, seen)),
                None => false,
            },
            // D-0294: in a decided program a `Box`, whose end the runtime
            // makes natively from the type descriptor (`box_of`), over such
            // a value (in a tracked one its contents' paths would need the
            // object `cb_raw_move_in` keeps).
            Type::Named(n, a) if n == "std::Box" && a.len() == 1 && self.decided => elem(&a[0], seen),
            Type::Named(n, a) if (n == "std::Vec" || n == "std::Queue" || n == "std::HashSet") && a.len() == 1 => elem(&a[0], seen),
            Type::Named(n, a) if n == "std::HashMap" && a.len() == 2 => elem(&a[0], seen) && elem(&a[1], seen),
            // (D-0294: a struct's; before, this arm took every name of the
            // program's, and an enum of the program's never reached the
            // next, D-0286's.)
            // D-0315: in a decided program, also one declared `resource` with
            // no destructor of its own: its end is its fields'.
            Type::Named(n, a) if (!n.starts_with("std::") || self.decided) && self.struct_decl(n).is_some() => match self.struct_decl(n) {
                Some(sd) if (!sd.resource || (self.decided && (!self.items.fns.contains_key(&format!("{}::drop", n)) || Self::MEMORY_ONLY_DROPS.contains(&n.as_str()) || self.noting.get()))) && sd.bits.is_none() => {
                    let sub = subst_of(&sd.type_params, a);
                    seen.push(t.clone());
                    let ok = sd.fields.iter().all(|f| elem(&apply(&f.ty, &sub), seen));
                    seen.pop();
                    ok
                }
                _ => false,
            },
            // D-0286: in a decided program, an enum (`Option`, `Result`, the
            // program's own) not declared `resource`, with no destructor of
            // its own, whose payloads are such values or plain data.
            Type::Named(n, a) if self.decided => match self.enum_decl(n) {
                Some(ed) if self.noting.get() || (!ed.resource && !self.items.fns.contains_key(&format!("{}::drop", n))) => {
                    let sub = subst_of(&ed.type_params, a);
                    seen.push(t.clone());
                    let ok = ed.variants.iter().all(|v| v.payload.as_ref().map_or(true, |p| elem(&apply(p, &sub), seen)));
                    seen.pop();
                    ok
                }
                _ => false,
            },
            _ => false,
        }
    }

    // D-0294: `plain_data` for a decided program (`Gen::decided`): a
    // reference or slice, which no runtime state follows there, counts as
    // plain, in a value as alone; a struct or enum the program declares
    // (not a type parameter's bare name) over such values too.
    // CHG-0429: its answers kept (`dp_memo`), but for a type that names a
    // closure.
    pub(super) fn decided_plain(&self, t: &Type) -> bool {
        if matches!(t, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Rawptr(_) | Type::Ref(..) | Type::Slice(..)) || names_closure(t) {
            return self.decided_plain_walk(t);
        }
        if let Some(&r) = self.dp_memo.borrow().get(t) {
            return r;
        }
        let r = self.decided_plain_walk(t);
        self.dp_memo.borrow_mut().insert(t.clone(), r);
        r
    }

    fn decided_plain_walk(&self, t: &Type) -> bool {
        match t {
            Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Rawptr(_) | Type::Ref(..) | Type::Slice(..) => true,
            Type::Array(inner, _) | Type::Simd(inner, _) => self.decided_plain(inner),
            Type::Named(n, args) => {
                if let Some(sd) = self.struct_decl(n) {
                    let sub = subst_of(&sd.type_params, args);
                    !sd.resource && sd.fields.iter().all(|f| self.decided_plain(&apply(&f.ty, &sub)))
                } else if let Some(ed) = self.enum_decl(n) {
                    let sub = subst_of(&ed.type_params, args);
                    !ed.resource && ed.variants.iter().all(|v| v.payload.as_ref().map_or(true, |p| self.decided_plain(&apply(p, &sub))))
                } else {
                    false
                }
            }
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
        // D-0312: their key is a probe now, a reference only for a struct
        // or enum element, and passed as the probe's type is.
        if pos == 1 && matches!(key, "std::Vec::contains" | "std::Vec::index_of" | "std::Vec::binary_search") {
            return self.items.fns.get(key).map_or(false, |f| in_prelude(f) && matches!(f.params.get(1).map(|p| &p.ty), Some(Type::Ref(..))));
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

// CHG-0429: whether `t` names a closure type anywhere in it.
pub(super) fn names_closure(t: &Type) -> bool {
    match t {
        Type::Closure(_) => true,
        Type::Ref(x, _) | Type::Slice(x, _) | Type::Rawptr(x) | Type::Array(x, _) | Type::Simd(x, _) | Type::Handle(x) | Type::Mutex(x) | Type::Guard(x) => names_closure(x),
        Type::Fn(ps, r) => ps.iter().any(names_closure) || names_closure(r),
        Type::Named(_, a) => a.iter().any(names_closure),
        _ => false,
    }
}
