// D-0134 `[Str-Literal-View]` (spec/21 §2a): a `str` literal where a
// `StringView` is expected is `StringView::of(L)`. "Expected" is a type
// written in a declaration: the parameter of the function a call names,
// the type of a declaration (`StringView v = "…";`), and the field of a
// struct literal. A literal has no type until its context gives it one,
// as an unsuffixed `5` has none (D-0079); a `str` binding is a value of
// type `str` and is not affected. Runs after `modres`, so every path is
// qualified and each call's function is found by its key.
use crate::ast::*;
use crate::modres;
use std::collections::HashMap;
use std::sync::Arc;

const VIEW: &str = "std::StringView";

struct Sigs {
    // A function's key to which of its parameters are `StringView`s.
    fns: HashMap<String, Vec<bool>>,
    // A struct's key to its `StringView` fields.
    structs: HashMap<String, Vec<String>>,
}

fn is_view(t: &Type) -> bool {
    matches!(t, Type::Named(n, a) if n == VIEW && a.is_empty())
}

pub fn process(program: &mut Program) {
    let mut sigs = Sigs { fns: HashMap::new(), structs: HashMap::new() };
    collect(&program.items, &mut Vec::new(), &mut sigs);
    if sigs.fns.is_empty() && sigs.structs.is_empty() {
        return;
    }
    rewrite_items(&mut program.items, &sigs);
}

fn collect(items: &[Item], path: &mut Vec<String>, out: &mut Sigs) {
    for it in items {
        match it {
            Item::Fn(f) => {
                let views: Vec<bool> = f.params.iter().map(|p| is_view(&p.ty)).collect();
                if views.iter().any(|v| *v) {
                    let simple = match &f.assoc_type {
                        Some(a) => format!("{}::{}", a, f.name),
                        None => f.name.clone(),
                    };
                    out.fns.insert(modres::qualify(path, &simple), views);
                }
            }
            Item::Struct(s) => {
                let fs: Vec<String> = s.fields.iter().filter(|f| is_view(&f.ty)).map(|f| f.name.clone()).collect();
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
            }
            Item::Module(_, _, inner, _) => rewrite_items(inner, sigs),
            _ => {}
        }
    }
}

// `L` as `std::StringView::of(L)`, when `e` is a literal (in parentheses
// or not).
fn view_of(e: &mut Expr) {
    if let ExprKind::Paren(inner) = &mut e.kind {
        return view_of(inner);
    }
    if !matches!(e.kind, ExprKind::StrLit(_)) {
        return;
    }
    let line = e.line;
    let lit = std::mem::replace(&mut e.kind, ExprKind::Unit);
    let callee = Expr { kind: ExprKind::Path(vec!["std".into(), "StringView".into(), "of".into()], Vec::new()), line };
    e.kind = ExprKind::Call(Box::new(callee), vec![Expr { kind: lit, line }]);
}

fn block(b: &mut Block, sigs: &Sigs) {
    for s in &mut b.stmts {
        match s {
            Stmt::Let { ty, init, .. } => {
                if let Some(e) = init {
                    expr(e, sigs);
                    if ty.as_ref().map_or(false, is_view) {
                        view_of(e);
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
            let views = sigs.structs.get(&segs.join("::"));
            for (name, x) in fields.iter_mut() {
                expr(x, sigs);
                if views.map_or(false, |v| v.contains(name)) {
                    view_of(x);
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
                if let Some(views) = sigs.fns.get(&segs.join("::")) {
                    if views.len() == args.len() {
                        for (a, v) in args.iter_mut().zip(views.iter()) {
                            if *v {
                                view_of(a);
                            }
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
