// D-0309 (`[Call-Self-Tail]`, spec/15): the self tail calls of a function
// -- a call of the function itself, by its own path, in a tail position
// of its body -- that are not a level of nesting for
// `[Call-Stack-Exhausted]`, when the function's frame holds nothing to
// destroy and nothing in it is ever borrowed, and its body is decided
// (`private/D-0309-call-self-tail-design.md` §2). The static pass records
// them by address in `Items::self_tails`; `coby` runs one without growing
// the host stack, `cobc` as a jump to the function's start.

use crate::ast::*;

// The tail positions of `e` (§2, condition 3): the expression itself, and
// through parentheses, a block's tail, both branches of an `if`/`else` and
// every arm of a `match`. `unit`: the function returns nothing, so a value
// is discarded where it ends: an `if` without `else` is a tail too, and so
// is a block's last statement when it has no tail expression.
fn tails<'a>(e: &'a Expr, unit: bool, out: &mut Vec<&'a Expr>) {
    match &e.kind {
        ExprKind::Paren(x) => tails(x, unit, out),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => block_tails(b, unit, out),
        ExprKind::If(_, t, el) if el.is_some() || unit => {
            block_tails(t, unit, out);
            if let Some(el) = el {
                tails(el, unit, out);
            }
        }
        ExprKind::Match(_, arms) => {
            for a in arms {
                tails(&a.body, unit, out);
            }
        }
        _ => out.push(e),
    }
}

fn block_tails<'a>(b: &'a Block, unit: bool, out: &mut Vec<&'a Expr>) {
    match &b.tail {
        Some(t) => tails(t, unit, out),
        None if unit => match b.stmts.last() {
            Some(Stmt::Expr(e)) | Some(Stmt::BlockLike(e)) => tails(e, unit, out),
            _ => {}
        },
        None => {}
    }
}

// Every expression of `b` outside closures' bodies, with `f` applied.
fn visit_block<'a>(b: &'a Block, f: &mut dyn FnMut(&'a Expr)) {
    for st in &b.stmts {
        match st {
            Stmt::Let { init: Some(e), .. } | Stmt::Expr(e) | Stmt::BlockLike(e) => visit(e, f),
            Stmt::Destructure { init, .. } => visit(init, f),
            Stmt::Let { init: None, .. } => {}
        }
    }
    if let Some(t) = &b.tail {
        visit(t, f);
    }
}

fn visit<'a>(e: &'a Expr, f: &mut dyn FnMut(&'a Expr)) {
    f(e);
    match &e.kind {
        ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => visit(a, f),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            visit(a, f);
            visit(b, f);
        }
        ExprKind::SliceOf(_, a, lo, hi) => {
            visit(a, f);
            visit(lo, f);
            visit(hi, f);
        }
        ExprKind::Call(c, args) => {
            visit(c, f);
            args.iter().for_each(|a| visit(a, f));
        }
        ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| visit(a, f)),
        ExprKind::ArrayLit(es) => es.iter().for_each(|a| visit(a, f)),
        ExprKind::ArrayRepeat(a, _) => visit(a, f),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => visit_block(b, f),
        ExprKind::If(c, t, el) => {
            visit(c, f);
            visit_block(t, f);
            if let Some(el) = el {
                visit(el, f);
            }
        }
        ExprKind::While(c, b, step) => {
            visit(c, f);
            visit_block(b, f);
            if let Some(st) = step {
                visit(st, f);
            }
        }
        ExprKind::Match(sc, arms) => {
            visit(sc, f);
            arms.iter().for_each(|a| visit(&a.body, f));
        }
        ExprKind::Return(Some(a)) => visit(a, f),
        // A closure's body is the closure's frame, not the function's.
        _ => {}
    }
}

// The self tail calls of `f` (named `name`) by address, given the types of
// every binding its frame declares (parameters included) and `kept`, the
// test of a type condition 4 rules out. Empty when a condition fails.
pub(crate) fn self_tail_calls(name: &str, f: &FnDecl, frame: &[(String, Type)], kept: &dyn Fn(&Type) -> bool) -> Vec<usize> {
    // Condition 2: not generic.
    if !f.type_params.is_empty() {
        return Vec::new();
    }
    // Condition 4: nothing in the frame to destroy.
    if frame.iter().any(|(_, t)| kept(t)) {
        return Vec::new();
    }
    let is_ref = |t: &Type| matches!(t, Type::Ref(..) | Type::Slice(..));
    let names_frame = |x: &str| frame.iter().any(|(n, _)| n == x);
    let through_ref = |x: &str| frame.iter().filter(|(n, _)| n == x).all(|(_, t)| is_ref(t));
    // Condition 5: nothing in the frame borrowed, and no closure (whose
    // captures may borrow it). A place's root: through fields and indexes
    // to a name; a dereference on the way reaches the referent instead.
    let mut borrowed = false;
    visit_block(&f.body, &mut |e| {
        let place = match &e.kind {
            ExprKind::Borrow(_, p) | ExprKind::SliceOf(_, p, _, _) => p,
            ExprKind::Closure { .. } => {
                borrowed = true;
                return;
            }
            _ => return,
        };
        let mut at: &Expr = place;
        let mut projected = false;
        loop {
            match &at.kind {
                ExprKind::Paren(x) => at = x,
                ExprKind::Field(x, _) | ExprKind::Index(x, _) => {
                    projected = true;
                    at = x;
                }
                ExprKind::Deref(_) => return,
                ExprKind::Path(segs, _) if segs.len() == 1 && names_frame(&segs[0]) => {
                    // The binding itself (`&n`), or a place in it that is not
                    // reached through a reference.
                    if !projected || !through_ref(&segs[0]) {
                        borrowed = true;
                    }
                    return;
                }
                _ => return,
            }
        }
    });
    if borrowed {
        return Vec::new();
    }
    // Condition 3: the tail positions, the body's own and every `return`'s.
    let mut at: Vec<&Expr> = Vec::new();
    let unit = matches!(f.ret, Type::Void);
    block_tails(&f.body, unit, &mut at);
    visit_block(&f.body, &mut |e| {
        if let ExprKind::Return(Some(x)) = &e.kind {
            tails(x, unit, &mut at);
        }
    });
    // Condition 1: a call of `name` itself, by its path.
    let mut out: Vec<usize> = at
        .into_iter()
        .filter(|e| matches!(&e.kind, ExprKind::Call(c, _) if matches!(&c.kind, ExprKind::Path(segs, targs) if targs.is_empty() && segs.join("::") == name)))
        .map(|e| e as *const Expr as usize)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}
