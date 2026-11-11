// The checker's rewrites (D-0312 `[Coerce-Text]`, `[Cmp-Text]` and
// `[Cmp-Through-Shared-Ref]`; D-0164, D-0218, D-0225, D-0301): the type
// checker notes, for an expression, what the program means by it where the
// written form is not what the position takes, and this pass writes that
// meaning out before the program is checked again. Runs after `modres`, so
// every path is qualified.
use crate::ast::*;
use crate::fxhash::HashMap;
use std::sync::Arc;

thread_local! {
    // D-0314: the `[Coerce-Slice-Literal]` nodes' array types (from the
    // checker, `set_slice_literals`), and the constants the pass adds.
    static SLICE_TYPES: std::cell::RefCell<HashMap<usize, Type>> = std::cell::RefCell::new(HashMap::default());
    static NEW_CONSTS: std::cell::RefCell<Vec<Item>> = std::cell::RefCell::new(Vec::new());
}

/// D-0314: the array types of the literals the next rewrite makes slices of
/// constants (`typecheck::take_slice_literals`).
pub fn set_slice_literals(t: HashMap<usize, Type>) {
    SLICE_TYPES.with(|g| *g.borrow_mut() = t);
}

const SLICE_LITERAL: &str = "__slice_literal_";

// A literal's value as text (`canon`): numbers as `{}` writes them, an
// array `[a,b]`, a repeat `[a;n]`, a struct `{f:a,g:b}` in written order;
// the type, which `slice_literal` adds, says what each is.
fn canon(e: &Expr, out: &mut String) {
    match &e.kind {
        ExprKind::IntLit(v, _) => out.push_str(&v.to_string()),
        ExprKind::FloatLit(v, _) => out.push_str(&format!("{}", v)),
        ExprKind::BoolLit(b) => out.push_str(if *b { "true" } else { "false" }),
        ExprKind::Unary(_, x) => {
            out.push('-');
            canon(x, out);
        }
        ExprKind::Paren(x) => canon(x, out),
        ExprKind::ArrayLit(xs) => {
            out.push('[');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canon(x, out);
            }
            out.push(']');
        }
        ExprKind::ArrayRepeat(x, n) => {
            out.push('[');
            canon(x, out);
            out.push_str(&format!(";{}]", n));
        }
        ExprKind::StructLit(_, _, fs) => {
            out.push('{');
            for (i, (f, x)) in fs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(f);
                out.push(':');
                canon(x, out);
            }
            out.push('}');
        }
        _ => out.push('?'),
    }
}

// `[Coerce-Slice-Literal]`: the literal `e` becomes the constant
// `__slice_literal_H` of type `t` (a root item), and `e` is `&(*&K)[0..$]`.
// `H` is the FNV-1a hash (64 bits) of the type and the value, `cbc`'s too:
// equal literals share one constant.
fn slice_literal(e: &mut Expr, t: Type) {
    let (line, col) = (e.line, e.col);
    let lit = std::mem::replace(e, Expr { kind: ExprKind::Unit, line, col });
    let mut text = format!("{}=", t);
    canon(&lit, &mut text);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let name = format!("{}{}", SLICE_LITERAL, h);
    NEW_CONSTS.with(|g| {
        let mut g = g.borrow_mut();
        if g.iter().any(|it| matches!(it, Item::Fn(f) if f.name == name)) {
            return;
        }
        let f = FnDecl {
            export: false,
            assoc_type: None,
            name: name.clone(),
            type_params: Vec::new(),
            type_bounds: Vec::new(),
            params: Vec::new(),
            ret: t,
            body: Block { stmts: Vec::new(), tail: Some(Box::new(lit)) },
            line,
            is_const: true,
            auto_type: false,
            derived: false,
            c_entry: false,
            source: Vec::new(),
            requires: Vec::new(),
            retired: Retired::Live,
        };
        g.push(Item::Fn(Arc::new(f)));
    });
    let at = |kind| Expr { kind, line, col };
    let call = at(ExprKind::Call(Box::new(at(ExprKind::Path(vec![name], Vec::new()))), Vec::new()));
    let root = at(ExprKind::Deref(Box::new(at(ExprKind::Borrow(Mode::Shared, Box::new(call))))));
    e.kind = ExprKind::SliceOf(Mode::Shared, Box::new(root), Box::new(Expr { kind: ExprKind::IntLit(0, None), line, col: 0 }), Box::new(at(ExprKind::Dollar)));
}

// D-0312 §2: a `str` `t` sliced: `t` as `std::StringView::of(t)`.
fn wrap_view(e: &mut Expr) {
    let line = e.line;
    let col = e.col;
    let t = std::mem::replace(&mut e.kind, ExprKind::Unit);
    let callee = Expr { kind: ExprKind::Path(vec!["std".into(), "StringView".into(), "of".into()], Vec::new()), line, col };
    e.kind = ExprKind::Call(Box::new(callee), vec![Expr { kind: t, line, col }]);
}

// `L` as `std::String::copy_str(L)`.
fn wrap_owned(e: &mut Expr) {
    let line = e.line;
    let col = e.col;
    let lit = std::mem::replace(&mut e.kind, ExprKind::Unit);
    let callee = Expr { kind: ExprKind::Path(vec!["std".into(), "String".into(), "copy_str".into()], Vec::new()), line, col };
    e.kind = ExprKind::Call(Box::new(callee), vec![Expr { kind: lit, line, col }]);
}

// D-0162: the text literals at `addresses` (found by the checker: a
// generic call's parameter typed them as `String`) become
// `String::copy_str(L)`. Called with the items dropped, so each function's
// `Arc` is unique and `make_mut` keeps every node where it was.
// D-0322 (`Rewrite::AssertValues`): `assert(l OP r)`, which `modres` made
// `if (!(l OP r)) { $assert_fail(m); }`, becomes
//
//     { τl $assert_l = l; τr $assert_r = r;
//       if ($assert_l OP $assert_r) { } else { $assert_fail($fmt(F, …, $assert_l, $assert_r)); } }
//
// each operand bound once, in order, as the comparison evaluated it, at the
// type the checker gave it (a literal's from the other side), and F
// `assertion failed: %v OP %v`, or the program's own format followed by
// ` (%v OP %v)`. The new shape is not an `assert`'s, so the next check asks
// for nothing more.
fn assert_values(e: &mut Expr, rw: &HashMap<usize, crate::typecheck::Rewrite>) {
    let ExprKind::If(c, t, None) = &mut e.kind else { return };
    let ExprKind::Unary(crate::ast::UnOp::Not, inner) = &mut c.kind else { return };
    let ExprKind::Paren(cmp) = &mut inner.kind else { return };
    let addr = &**cmp as *const Expr as usize;
    let Some(crate::typecheck::Rewrite::AssertValues { l: lt, r: rt }) = rw.get(&addr).copied() else { return };
    let ExprKind::Binary(op, l, r) = &mut cmp.kind else { return };
    if t.stmts.len() != 1 {
        return;
    }
    let crate::ast::Stmt::Expr(call) = &mut t.stmts[0] else { return };
    let ExprKind::Call(_, msg) = &mut call.kind else { return };
    let (line, col) = (e.line, e.col);
    let op = *op;
    let shown = match op {
        crate::ast::BinOp::Eq => "==",
        crate::ast::BinOp::Ne => "!=",
        crate::ast::BinOp::Lt => "<",
        crate::ast::BinOp::Le => "<=",
        crate::ast::BinOp::Gt => ">",
        _ => ">=",
    };
    let unit = || Expr { kind: ExprKind::Unit, line, col };
    let path = |n: &str| Expr { kind: ExprKind::Path(vec![n.to_string()], Vec::new()), line, col };
    let lx = std::mem::replace(l, Box::new(unit()));
    let rx = std::mem::replace(r, Box::new(unit()));
    let mut msg = std::mem::take(msg);
    let mut stmts = Vec::new();
    let mut operand = |x: Box<Expr>, ty: crate::typecheck::Scalar, name: &str| -> Expr {
        stmts.push(crate::ast::Stmt::Let { ty: Some(ty.ty()), name: name.to_string(), init: Some(x), line, col: 0 });
        path(name)
    };
    let lv = operand(lx, lt, "$assert_l");
    let rv = operand(rx, rt, "$assert_r");
    let mut args = match msg.pop() {
        Some(Expr { kind: ExprKind::Call(_, mut fargs), .. }) if matches!(fargs.first().map(|f| &f.kind), Some(ExprKind::StrLit(_))) => {
            if let Some(Expr { kind: ExprKind::StrLit(s), .. }) = fargs.first_mut() {
                s.push_str(&format!(" (%v {} %v)", shown));
            }
            fargs
        }
        _ => vec![Expr { kind: ExprKind::StrLit(format!("assertion failed: %v {} %v", shown)), line, col }],
    };
    args.push(lv.clone());
    args.push(rv.clone());
    let fmt = Expr { kind: ExprKind::Call(Box::new(path("$fmt")), args), line, col };
    let fail = Expr { kind: ExprKind::Call(Box::new(path("$assert_fail")), vec![fmt]), line, col };
    let otherwise = Expr { kind: ExprKind::Block(Block { stmts: vec![crate::ast::Stmt::Expr(Box::new(fail))], tail: None }), line, col };
    let test = Expr { kind: ExprKind::Binary(op, Box::new(lv), Box::new(rv)), line, col };
    let iff = Expr { kind: ExprKind::If(Box::new(test), Block { stmts: Vec::new(), tail: None }, Some(Box::new(otherwise))), line, col };
    stmts.push(crate::ast::Stmt::Expr(Box::new(iff)));
    e.kind = ExprKind::Block(Block { stmts, tail: None });
}

pub fn rewrite_literals_as_string(program: &mut Program, addresses: &crate::fxhash::HashSet<usize>, rewrites: &HashMap<usize, crate::typecheck::Rewrite>) {
    type Rw = HashMap<usize, crate::typecheck::Rewrite>;
    fn items(list: &mut [Item], a: &crate::fxhash::HashSet<usize>, rw: &Rw) {
        for it in list.iter_mut() {
            match it {
                Item::Fn(f) => {
                    let f = Arc::make_mut(f);
                    blk(&mut f.body, a, rw);
                }
                Item::Module(_, _, inner, _, _) => items(inner, a, rw),
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
        if rw.get(&addr) == Some(&crate::typecheck::Rewrite::StrView) {
            wrap_view(e);
            return;
        }
        if rw.get(&addr) == Some(&crate::typecheck::Rewrite::SliceLiteral) {
            if let Some(t) = SLICE_TYPES.with(|g| g.borrow_mut().remove(&addr)) {
                slice_literal(e, t);
                return;
            }
        }
        if matches!(e.kind, ExprKind::StrLit(_)) && a.contains(&addr) {
            wrap_owned(e);
            return;
        }
        // D-0164: `s == t` with `s` a `String` and `t` a `str` or a
        // `StringView` (either way round) is `String::same_str(&s, t)` or
        // `String::same_view(&s, t)`, negated for `!=`.
        if let Some(crate::typecheck::Rewrite::TextEq { string_left, other_is_str, negate }) = rw.get(&addr).copied() {
            if let ExprKind::Binary(_, l, r) = &mut e.kind {
                ex(l, a, rw);
                ex(r, a, rw);
                let line = e.line;
                let col = e.col;
                let (s, t) = if string_left { (std::mem::replace(&mut **l, Expr { kind: ExprKind::Unit, line, col }), std::mem::replace(&mut **r, Expr { kind: ExprKind::Unit, line, col })) } else { (std::mem::replace(&mut **r, Expr { kind: ExprKind::Unit, line, col }), std::mem::replace(&mut **l, Expr { kind: ExprKind::Unit, line, col })) };
                let f = if other_is_str { "same_str" } else { "same_view" };
                let callee = Expr { kind: ExprKind::Path(vec!["std".into(), "String".into(), f.into()], Vec::new()), line, col };
                let borrowed = Expr { kind: ExprKind::Borrow(Mode::Shared, Box::new(s)), line, col };
                let call = Expr { kind: ExprKind::Call(Box::new(callee), vec![borrowed, t]), line, col };
                e.kind = if negate { ExprKind::Unary(crate::ast::UnOp::Not, Box::new(Expr { kind: ExprKind::Paren(Box::new(call)), line, col })) } else { call.kind };
                return;
            }
        }
        // D-0301: a `*x` binder's call, `Box::get_mut` or `Box::into_inner`.
        if let Some(r @ (crate::typecheck::Rewrite::BoxGetMut | crate::typecheck::Rewrite::BoxIntoInner)) = rw.get(&addr).copied() {
            if let ExprKind::Path(segs, _) = &mut e.kind {
                *segs.last_mut().unwrap() = if r == crate::typecheck::Rewrite::BoxGetMut { "get_mut".into() } else { "into_inner".into() };
                return;
            }
        }
        // D-0302: text where a `StringView` is declared: `&e` is `&e[0..$]`,
        // a place `r` of type `ref<String, shared>` is `&(*r)[0..$]`, and a
        // `str` value `t` is `StringView::of(t)`.
        if let Some(r @ (crate::typecheck::Rewrite::RefStringView | crate::typecheck::Rewrite::RefPlaceView | crate::typecheck::Rewrite::StrValueView)) = rw.get(&addr).copied() {
            let line = e.line;
            let col = e.col;
            let old = std::mem::replace(e, Expr { kind: ExprKind::Unit, line, col });
            let zero = Box::new(Expr { kind: ExprKind::IntLit(0, None), line, col: 0 });
            let dollar = Box::new(Expr { kind: ExprKind::Dollar, line, col });
            e.kind = match (r, old.kind) {
                (crate::typecheck::Rewrite::RefStringView, ExprKind::Borrow(_, inner)) => ExprKind::SliceOf(Mode::Shared, inner, zero, dollar),
                (crate::typecheck::Rewrite::RefPlaceView, k) => ExprKind::SliceOf(Mode::Shared, Box::new(Expr { kind: ExprKind::Deref(Box::new(Expr { kind: k, line, col })), line, col }), zero, dollar),
                (_, k) => {
                    let callee = Expr { kind: ExprKind::Path(vec!["std".into(), "StringView".into(), "of".into()], Vec::new()), line, col };
                    ExprKind::Call(Box::new(callee), vec![Expr { kind: k, line, col }])
                }
            };
            // The node keeps its address: its parts, not it, are visited.
            match &mut e.kind {
                ExprKind::SliceOf(_, x, _, _) => ex(x, a, rw),
                ExprKind::Call(_, args) => args.iter_mut().for_each(|x| ex(x, a, rw)),
                _ => {}
            }
            return;
        }
        // D-0312 `[Coerce-Slice]`: `&v` is `&v[0..$]`, a reference `e` to a
        // `Vec` or an array `&(*e)[0..$]`, in the slice's mode.
        if let Some(r @ (crate::typecheck::Rewrite::SeqSlice { .. } | crate::typecheck::Rewrite::SeqPlaceSlice { .. })) = rw.get(&addr).copied() {
            let line = e.line;
            let col = e.col;
            let mode = match r {
                crate::typecheck::Rewrite::SeqSlice { exclusive: true } | crate::typecheck::Rewrite::SeqPlaceSlice { exclusive: true } => Mode::Exclusive,
                _ => Mode::Shared,
            };
            let old = std::mem::replace(e, Expr { kind: ExprKind::Unit, line, col });
            let zero = Box::new(Expr { kind: ExprKind::IntLit(0, None), line, col: 0 });
            let dollar = Box::new(Expr { kind: ExprKind::Dollar, line, col });
            e.kind = match (r, old.kind) {
                (crate::typecheck::Rewrite::SeqSlice { .. }, ExprKind::Borrow(_, inner)) => ExprKind::SliceOf(mode, inner, zero, dollar),
                (_, k) => ExprKind::SliceOf(mode, Box::new(Expr { kind: ExprKind::Deref(Box::new(Expr { kind: k, line, col })), line, col }), zero, dollar),
            };
            if let ExprKind::SliceOf(_, x, _, _) = &mut e.kind {
                ex(x, a, rw);
            }
            return;
        }
        // D-0312 `[Cmp-Through-Shared-Ref]`: the reference operand, `*e`.
        if let Some(crate::typecheck::Rewrite::DerefOperand) = rw.get(&addr).copied() {
            let line = e.line;
            let col = e.col;
            let old = std::mem::replace(e, Expr { kind: ExprKind::Unit, line, col });
            e.kind = ExprKind::Paren(Box::new(Expr { kind: ExprKind::Deref(Box::new(old)), line, col }));
            if let ExprKind::Paren(x) = &mut e.kind {
                if let ExprKind::Deref(y) = &mut x.kind {
                    ex(y, a, rw);
                }
            }
            return;
        }
        // D-0312 `[Cmp-Text]`: mixed text ordered by `StringView::less`.
        if let Some(crate::typecheck::Rewrite::TextOrder { swap, negate, borrow_left, borrow_right }) = rw.get(&addr).copied() {
            if let ExprKind::Binary(_, l, r) = &mut e.kind {
                ex(l, a, rw);
                ex(r, a, rw);
                let line = e.line;
                let col = e.col;
                let unit = || Expr { kind: ExprKind::Unit, line, col };
                let mut x = std::mem::replace(&mut **l, unit());
                let mut y = std::mem::replace(&mut **r, unit());
                if borrow_left {
                    x = Expr { kind: ExprKind::Borrow(Mode::Shared, Box::new(x)), line, col };
                }
                if borrow_right {
                    y = Expr { kind: ExprKind::Borrow(Mode::Shared, Box::new(y)), line, col };
                }
                let (p, q) = if swap { (y, x) } else { (x, y) };
                let callee = Expr { kind: ExprKind::Path(vec!["std".into(), "StringView".into(), "less".into()], Vec::new()), line, col };
                let call = Expr { kind: ExprKind::Call(Box::new(callee), vec![p, q]), line, col };
                e.kind = if negate { ExprKind::Unary(crate::ast::UnOp::Not, Box::new(Expr { kind: ExprKind::Paren(Box::new(call)), line, col })) } else { call.kind };
                return;
            }
        }
        // D-0225: a text literal as a variant's `StringView` payload.
        if let Some(crate::typecheck::Rewrite::LiteralView) = rw.get(&addr).copied() {
            let line = e.line;
            let col = e.col;
            let lit = std::mem::replace(&mut e.kind, ExprKind::Unit);
            let callee = Expr { kind: ExprKind::Path(vec!["std".into(), "StringView".into(), "of".into()], Vec::new()), line, col };
            e.kind = ExprKind::Call(Box::new(callee), vec![Expr { kind: lit, line, col }]);
            return;
        }
        // D-0218: an exclusive reference read through a shared one is a
        // shared borrow of what it reaches: `&*e`, or `&e[0..$]` for a slice.
        if let Some(r @ (crate::typecheck::Rewrite::SharedRef | crate::typecheck::Rewrite::SharedSlice)) = rw.get(&addr).copied() {
            let line = e.line;
            let col = e.col;
            let old = std::mem::replace(e, Expr { kind: ExprKind::Unit, line, col });
            e.kind = if r == crate::typecheck::Rewrite::SharedRef {
                ExprKind::Borrow(Mode::Shared, Box::new(Expr { kind: ExprKind::Deref(Box::new(old)), line, col }))
            } else {
                ExprKind::SliceOf(Mode::Shared, Box::new(old), Box::new(Expr { kind: ExprKind::IntLit(0, None), line, col: 0 }), Box::new(Expr { kind: ExprKind::Dollar, line, col }))
            };
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
                // D-0322: an `assert` of two scalars, its operands' own
                // rewrites applied first.
                assert_values(e, rw);
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
    NEW_CONSTS.with(|g| g.borrow_mut().clear());
    items(&mut program.items, addresses, rewrites);
    let added = NEW_CONSTS.with(|g| std::mem::take(&mut *g.borrow_mut()));
    for it in added {
        // One a literal of an earlier round already made is the same.
        let Item::Fn(f) = &it else { continue };
        if !program.items.iter().any(|x| matches!(x, Item::Fn(g) if g.name == f.name)) {
            program.items.push(it);
        }
    }
    SLICE_TYPES.with(|g| g.borrow_mut().clear());
}
