// D-0134 `[Str-Literal-View]` and D-0149 `[Str-Literal-String]` (spec/21
// §2a): a `str` literal where a `StringView` is expected is
// `StringView::of(L)`, and one where a `String` is expected is
// `String::from_str(L)`. "Expected" is a type written in a declaration:
// the parameter of the function a call names, the type of a declaration
// (`StringView v = "…";`, `String s = "…";`), the field of a struct
// literal, and, for `String` only, the result type of the function the
// literal is returned from (`return "…";`, or the literal as the body's
// tail expression, through `if` and `match` arms). A literal has no type
// until its context gives it one, as an unsuffixed `5` has none (D-0079);
// a `str` binding is a value of type `str` and is not affected, nor is a
// parameter of a type-parameter type. Runs after `modres`, so every path
// is qualified and each call's function is found by its key.
use crate::ast::*;
use crate::modres;
use crate::fxhash::HashMap;
use std::sync::Arc;

const VIEW: &str = "std::StringView";
const OWNED: &str = "std::String";

// What a declared type asks of a literal in its position.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    No,
    View,
    Owned,
}

struct Sigs {
    // A function's key to what each of its parameters asks.
    fns: HashMap<String, Vec<Kind>>,
    // A struct's key to what each of its text fields asks.
    structs: HashMap<String, Vec<(String, Kind)>>,
}

fn kind_of(t: &Type) -> Kind {
    match t {
        Type::Named(n, a) if a.is_empty() && n == VIEW => Kind::View,
        Type::Named(n, a) if a.is_empty() && n == OWNED => Kind::Owned,
        _ => Kind::No,
    }
}

pub fn process(program: &mut Program) {
    let mut sigs = Sigs { fns: HashMap::default(), structs: HashMap::default() };
    collect(&program.items, &mut Vec::new(), &mut sigs);
    rewrite_items(&mut program.items, &sigs);
}

fn collect(items: &[Item], path: &mut Vec<String>, out: &mut Sigs) {
    for it in items {
        match it {
            Item::Fn(f) => {
                let kinds: Vec<Kind> = f.params.iter().map(|p| kind_of(&p.ty)).collect();
                if kinds.iter().any(|k| *k != Kind::No) {
                    let simple = match &f.assoc_type {
                        Some(a) => format!("{}::{}", a, f.name),
                        None => f.name.clone(),
                    };
                    out.fns.insert(modres::qualify(path, &simple), kinds);
                }
            }
            Item::Struct(s) => {
                let fs: Vec<(String, Kind)> = s.fields.iter().map(|f| (f.name.clone(), kind_of(&f.ty))).filter(|(_, k)| *k != Kind::No).collect();
                if !fs.is_empty() {
                    out.structs.insert(modres::qualify(path, &s.name), fs);
                }
            }
            Item::Module(_, name, inner, _) => {
                path.push(name.clone());
                collect(inner, path, out);
                path.pop();
            }
            _ => {}
        }
    }
}

fn rewrite_items(items: &mut [Item], sigs: &Sigs) {
    for it in items.iter_mut() {
        match it {
            Item::Fn(f) => {
                let f = Arc::make_mut(f);
                block(&mut f.body, sigs);
                // D-0149: the result type of the function is the expected
                // type of what it returns.
                if !f.is_const && kind_of(&f.ret) == Kind::Owned {
                    returns_in_block(&mut f.body);
                    tail_of_block(&mut f.body);
                }
            }
            Item::Module(_, _, inner, _) => rewrite_items(inner, sigs),
            _ => {}
        }
    }
}

// `L` as `std::StringView::of(L)` or `std::String::from_str(L)`, when `e`
// is a literal (in parentheses or not).
fn wrap(e: &mut Expr, kind: Kind) {
    if kind == Kind::No {
        return;
    }
    // D-0155: the literal may be what a block, an `if` branch or a `match`
    // arm in that position gives; each such tail is in the position too.
    match &mut e.kind {
        ExprKind::Paren(inner) => return wrap(inner, kind),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => {
            if let Some(t) = &mut b.tail {
                wrap(t, kind);
            }
            return;
        }
        ExprKind::If(_, t, els) => {
            if let Some(x) = &mut t.tail {
                wrap(x, kind);
            }
            if let Some(x) = els {
                wrap(x, kind);
            }
            return;
        }
        ExprKind::Match(_, arms) => {
            for a in arms.iter_mut() {
                wrap(&mut a.body, kind);
            }
            return;
        }
        ExprKind::StrLit(_) => {}
        _ => return,
    }
    let line = e.line;
    let lit = std::mem::replace(&mut e.kind, ExprKind::Unit);
    let path = match kind {
        Kind::View => vec!["std".into(), "StringView".into(), "of".into()],
        _ => vec!["std".into(), "String".into(), "from_str".into()],
    };
    let callee = Expr { kind: ExprKind::Path(path, Vec::new()), line };
    e.kind = ExprKind::Call(Box::new(callee), vec![Expr { kind: lit, line }]);
}

// D-0149: every `return L;` of the function's own body (a closure's
// `return` is the closure's and is left alone).
fn returns_in_block(b: &mut Block) {
    for s in &mut b.stmts {
        match s {
            Stmt::Let { init, .. } => {
                if let Some(e) = init {
                    returns_in_expr(e);
                }
            }
            Stmt::Destructure { init, .. } => returns_in_expr(init),
            Stmt::Expr(e) | Stmt::BlockLike(e) => returns_in_expr(e),
        }
    }
    if let Some(t) = &mut b.tail {
        returns_in_expr(t);
    }
}

fn returns_in_expr(e: &mut Expr) {
    match &mut e.kind {
        ExprKind::Return(Some(x)) => {
            returns_in_expr(x);
            wrap(x, Kind::Owned);
        }
        ExprKind::Return(None) | ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Unit | ExprKind::Path(..) | ExprKind::Dollar | ExprKind::Break | ExprKind::Continue | ExprKind::Closure { .. } => {}
        ExprKind::StructLit(_, _, fields) => fields.iter_mut().for_each(|(_, x)| returns_in_expr(x)),
        ExprKind::ArrayLit(xs) => xs.iter_mut().for_each(returns_in_expr),
        ExprKind::ArrayRepeat(x, _) | ExprKind::Unary(_, x) | ExprKind::Borrow(_, x) | ExprKind::Deref(x) | ExprKind::Field(x, _) | ExprKind::Propagate(x) | ExprKind::Paren(x) => returns_in_expr(x),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            returns_in_expr(a);
            returns_in_expr(b);
        }
        ExprKind::SliceOf(_, a, b, c) => {
            returns_in_expr(a);
            returns_in_expr(b);
            returns_in_expr(c);
        }
        ExprKind::Call(callee, args) => {
            returns_in_expr(callee);
            args.iter_mut().for_each(returns_in_expr);
        }
        ExprKind::Block(b) | ExprKind::Unsafe(b) => returns_in_block(b),
        ExprKind::If(c, t, els) => {
            returns_in_expr(c);
            returns_in_block(t);
            if let Some(x) = els {
                returns_in_expr(x);
            }
        }
        ExprKind::While(c, b, step) => {
            returns_in_expr(c);
            returns_in_block(b);
            if let Some(x) = step {
                returns_in_expr(x);
            }
        }
        ExprKind::Match(s, arms) => {
            returns_in_expr(s);
            arms.iter_mut().for_each(|a| returns_in_expr(&mut a.body));
        }
    }
}

// D-0149: the body's tail expression, through blocks, `if` branches and
// `match` arms, is what the function gives.
fn tail_of_block(b: &mut Block) {
    if let Some(t) = &mut b.tail {
        tail_of_expr(t);
    }
}

fn tail_of_expr(e: &mut Expr) {
    wrap(e, Kind::Owned);
}

fn block(b: &mut Block, sigs: &Sigs) {
    for s in &mut b.stmts {
        match s {
            Stmt::Let { ty, init, .. } => {
                if let Some(e) = init {
                    expr(e, sigs);
                    if let Some(t) = ty {
                        wrap(e, kind_of(t));
                    }
                }
            }
            Stmt::Destructure { init, .. } => expr(init, sigs),
            Stmt::Expr(e) | Stmt::BlockLike(e) => expr(e, sigs),
        }
    }
    if let Some(t) = &mut b.tail {
        expr(t, sigs);
    }
}

fn expr(e: &mut Expr, sigs: &Sigs) {
    match &mut e.kind {
        ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Unit | ExprKind::Path(..) | ExprKind::Dollar | ExprKind::Break | ExprKind::Continue => {}
        ExprKind::StructLit(segs, _, fields) => {
            let kinds = sigs.structs.get(&segs.join("::"));
            for (name, x) in fields.iter_mut() {
                expr(x, sigs);
                if let Some(k) = kinds.and_then(|v| v.iter().find(|(n, _)| n == name)) {
                    wrap(x, k.1);
                }
            }
        }
        ExprKind::ArrayLit(xs) => xs.iter_mut().for_each(|x| expr(x, sigs)),
        ExprKind::ArrayRepeat(x, _) | ExprKind::Unary(_, x) | ExprKind::Borrow(_, x) | ExprKind::Deref(x) | ExprKind::Field(x, _) | ExprKind::Propagate(x) | ExprKind::Paren(x) => expr(x, sigs),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            expr(a, sigs);
            expr(b, sigs);
        }
        ExprKind::SliceOf(_, a, b, c) => {
            expr(a, sigs);
            expr(b, sigs);
            expr(c, sigs);
        }
        ExprKind::Call(callee, args) => {
            expr(callee, sigs);
            for a in args.iter_mut() {
                expr(a, sigs);
            }
            if let ExprKind::Path(segs, _) = &callee.kind {
                if let Some(kinds) = sigs.fns.get(&segs.join("::")) {
                    if kinds.len() == args.len() {
                        for (a, k) in args.iter_mut().zip(kinds.iter()) {
                            wrap(a, *k);
                        }
                    }
                }
            }
        }
        ExprKind::Block(b) | ExprKind::Unsafe(b) => block(b, sigs),
        ExprKind::If(c, t, els) => {
            expr(c, sigs);
            block(t, sigs);
            if let Some(x) = els {
                expr(x, sigs);
            }
        }
        ExprKind::While(c, b, step) => {
            expr(c, sigs);
            block(b, sigs);
            if let Some(x) = step {
                expr(x, sigs);
            }
        }
        ExprKind::Match(s, arms) => {
            expr(s, sigs);
            for a in arms.iter_mut() {
                if let Some(l) = &mut a.lit {
                    expr(l, sigs);
                }
                expr(&mut a.body, sigs);
            }
        }
        ExprKind::Return(x) => {
            if let Some(x) = x {
                expr(x, sigs);
            }
        }
        ExprKind::Closure { body, .. } => block(Arc::make_mut(body), sigs),
    }
}

// D-0162: the text literals at `addresses` (found by the checker: a
// generic call's parameter typed them as `String`) become
// `String::from_str(L)`. Called with the items dropped, so each function's
// `Arc` is unique and `make_mut` keeps every node where it was.
pub fn rewrite_literals_as_string(program: &mut Program, addresses: &crate::fxhash::HashSet<usize>, rewrites: &HashMap<usize, crate::typecheck::Rewrite>) {
    type Rw = HashMap<usize, crate::typecheck::Rewrite>;
    fn items(list: &mut [Item], a: &crate::fxhash::HashSet<usize>, rw: &Rw) {
        for it in list.iter_mut() {
            match it {
                Item::Fn(f) => {
                    let f = Arc::make_mut(f);
                    blk(&mut f.body, a, rw);
                }
                Item::Module(_, _, inner, _) => items(inner, a, rw),
                _ => {}
            }
        }
    }
    fn blk(b: &mut Block, a: &crate::fxhash::HashSet<usize>, rw: &Rw) {
        for s in &mut b.stmts {
            match s {
                Stmt::Let { init, .. } => {
                    if let Some(e) = init {
                        ex(e, a, rw);
                    }
                }
                Stmt::Destructure { init, .. } => ex(init, a, rw),
                Stmt::Expr(e) | Stmt::BlockLike(e) => ex(e, a, rw),
            }
        }
        if let Some(t) = &mut b.tail {
            ex(t, a, rw);
        }
    }
    fn ex(e: &mut Expr, a: &crate::fxhash::HashSet<usize>, rw: &Rw) {
        let addr = e as *const Expr as usize;
        if matches!(e.kind, ExprKind::StrLit(_)) && a.contains(&addr) {
            wrap(e, Kind::Owned);
            return;
        }
        // D-0164: `s == t` with `s` a `String` and `t` a `str` or a
        // `StringView` (either way round) is `String::eq_str(&s, t)` or
        // `String::eq_view(&s, t)`, negated for `!=`.
        if let Some(crate::typecheck::Rewrite::TextEq { string_left, other_is_str, negate }) = rw.get(&addr).copied() {
            if let ExprKind::Binary(_, l, r) = &mut e.kind {
                ex(l, a, rw);
                ex(r, a, rw);
                let line = e.line;
                let (s, t) = if string_left { (std::mem::replace(&mut **l, Expr { kind: ExprKind::Unit, line }), std::mem::replace(&mut **r, Expr { kind: ExprKind::Unit, line })) } else { (std::mem::replace(&mut **r, Expr { kind: ExprKind::Unit, line }), std::mem::replace(&mut **l, Expr { kind: ExprKind::Unit, line })) };
                let f = if other_is_str { "eq_str" } else { "eq_view" };
                let callee = Expr { kind: ExprKind::Path(vec!["std".into(), "String".into(), f.into()], Vec::new()), line };
                let borrowed = Expr { kind: ExprKind::Borrow(Mode::Shared, Box::new(s)), line };
                let call = Expr { kind: ExprKind::Call(Box::new(callee), vec![borrowed, t]), line };
                e.kind = if negate { ExprKind::Unary(crate::ast::UnOp::Not, Box::new(Expr { kind: ExprKind::Paren(Box::new(call)), line })) } else { call.kind };
                return;
            }
        }
        match &mut e.kind {
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Unit | ExprKind::Path(..) | ExprKind::Dollar | ExprKind::Break | ExprKind::Continue => {}
            ExprKind::StructLit(_, _, fields) => fields.iter_mut().for_each(|(_, x)| ex(x, a, rw)),
            ExprKind::ArrayLit(xs) => xs.iter_mut().for_each(|x| ex(x, a, rw)),
            ExprKind::ArrayRepeat(x, _) | ExprKind::Unary(_, x) | ExprKind::Borrow(_, x) | ExprKind::Deref(x) | ExprKind::Field(x, _) | ExprKind::Propagate(x) | ExprKind::Paren(x) => ex(x, a, rw),
            ExprKind::Binary(_, l, r) | ExprKind::Index(l, r) | ExprKind::Assign(l, r) => {
                ex(l, a, rw);
                ex(r, a, rw);
            }
            ExprKind::SliceOf(_, x, y, z) => {
                ex(x, a, rw);
                ex(y, a, rw);
                ex(z, a, rw);
            }
            ExprKind::Call(c, args) => {
                ex(c, a, rw);
                args.iter_mut().for_each(|x| ex(x, a, rw));
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => blk(b, a, rw),
            ExprKind::If(c, t, els) => {
                ex(c, a, rw);
                blk(t, a, rw);
                if let Some(x) = els {
                    ex(x, a, rw);
                }
            }
            ExprKind::While(c, b, step) => {
                ex(c, a, rw);
                blk(b, a, rw);
                if let Some(x) = step {
                    ex(x, a, rw);
                }
            }
            ExprKind::Match(sc, arms) => {
                ex(sc, a, rw);
                for arm in arms.iter_mut() {
                    if let Some(l) = &mut arm.lit {
                        ex(l, a, rw);
                    }
                    ex(&mut arm.body, a, rw);
                }
            }
            ExprKind::Return(x) => {
                if let Some(x) = x {
                    ex(x, a, rw);
                }
            }
            ExprKind::Closure { body, .. } => blk(Arc::make_mut(body), a, rw),
        }
    }
    items(&mut program.items, addresses, rewrites);
}
