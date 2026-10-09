// `const` items (D-0036, `spec/17` §1 `rule.module.const`), after name
// resolution: a use of a constant is already a call of it (`modres`).
//
// 1. No constant may depend on itself, through any chain of others
//    (`[Const-Cycle]`, `diag.const-not-constant` at the first one found).
// 2. A constant whose initializer is integer, float, `bool` or `str`
//    arithmetic over literals and other such constants is folded: its
//    value computed here with the checked arithmetic of `spec/06` (an
//    overflow, a division by zero, a shift out of range is the same
//    diagnostic it would be at run time, reported at the constant), and
//    every use rewritten to a literal of the constant's type. Any other
//    constant keeps the call; its initializer is checked to be a constant
//    expression by the static pass (`typecheck`).

use crate::ast::*;
use crate::fxhash::HashMap;

#[derive(Clone, Debug)]
enum Val {
    // A signed value, or an unsigned one's bit pattern, of its type.
    Int(IntTy, i128),
    Float(f64, bool),
    Bool(bool),
    Str(String),
}

struct Const {
    ty: Type,
    init: Expr,
    line: usize,
}

pub fn process(program: &mut Program) -> Result<(), String> {
    resolve_auto(program)?;
    let mut consts = HashMap::default();
    collect(&program.items, &mut Vec::new(), &mut consts);
    if consts.is_empty() {
        return Ok(());
    }
    check_cycles(&consts)?;
    let mut folded: HashMap<String, Option<Val>> = HashMap::default();
    let mut keys: Vec<&String> = consts.keys().collect();
    keys.sort();
    for k in keys {
        fold(k, &consts, &mut folded)?;
    }
    let lits: HashMap<String, Expr> = folded.into_iter().filter_map(|(k, v)| v.map(|v| (k, v))).map(|(k, v)| (k, literal(&v))).collect();
    if !lits.is_empty() {
        rewrite_items(&mut program.items, &lits);
    }
    Ok(())
}

fn collect(items: &[Item], path: &mut Vec<String>, out: &mut HashMap<String, Const>) {
    for it in items {
        match it {
            Item::Fn(f) if f.is_const => {
                if let Some(init) = &f.body.tail {
                    out.insert(crate::modres::qualify(path, &f.name), Const { ty: f.ret.clone(), init: (**init).clone(), line: f.line });
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

// The constants `e` uses: `modres` made each use a call with no arguments.
fn uses(e: &Expr, consts: &HashMap<String, Const>, out: &mut Vec<String>) {
    visit(e, &mut |x| {
        if let ExprKind::Call(c, args) = &x.kind {
            if let (ExprKind::Path(segs, _), true) = (&c.kind, args.is_empty()) {
                let k = segs.join("::");
                if consts.contains_key(&k) {
                    out.push(k);
                }
            }
        }
    });
}

fn check_cycles(consts: &HashMap<String, Const>) -> Result<(), String> {
    // 0 = unseen, 1 = on the current path, 2 = done.
    fn dfs(k: &str, consts: &HashMap<String, Const>, state: &mut HashMap<String, u8>) -> Result<(), String> {
        match state.get(k) {
            Some(2) => return Ok(()),
            Some(1) => return Err(format!("diag.const-not-constant@{}", consts[k].line)),
            _ => {}
        }
        state.insert(k.to_string(), 1);
        let mut deps = Vec::new();
        uses(&consts[k].init, consts, &mut deps);
        for d in deps {
            dfs(&d, consts, state)?;
        }
        state.insert(k.to_string(), 2);
        Ok(())
    }
    let mut state = HashMap::default();
    let mut keys: Vec<&String> = consts.keys().collect();
    keys.sort();
    for k in keys {
        dfs(k, consts, &mut state)?;
    }
    Ok(())
}

fn fold(k: &str, consts: &HashMap<String, Const>, folded: &mut HashMap<String, Option<Val>>) -> Result<Option<Val>, String> {
    if let Some(v) = folded.get(k) {
        return Ok(v.clone());
    }
    let c = &consts[k];
    let v = eval(&c.init, &c.ty, consts, folded).map_err(|d| format!("{}@{}", d, c.line))?;
    folded.insert(k.to_string(), v.clone());
    Ok(v)
}

fn range(t: IntTy) -> (i128, u128) {
    let bw = t.bitwidth(crate::value::ADDR_WIDTH);
    if t.signed() {
        let min = if bw >= 128 { i128::MIN } else { -(1i128 << (bw - 1)) };
        (min, if bw >= 128 { i128::MAX as u128 } else { (1u128 << (bw - 1)) - 1 })
    } else {
        (0, if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 })
    }
}

// An integer of type `t` from a mathematical value, if it is one.
fn int_of(t: IntTy, neg: bool, mag: u128) -> Option<Val> {
    let (min, max) = range(t);
    if neg {
        if mag == 0 {
            return Some(Val::Int(t, 0));
        }
        let lo = min.unsigned_abs();
        if !t.signed() || mag > lo {
            return None;
        }
        Some(Val::Int(t, (mag as i128).wrapping_neg()))
    } else if mag > max {
        None
    } else {
        Some(Val::Int(t, mag as i128))
    }
}

fn signed_parts(t: IntTy, v: i128) -> (bool, u128) {
    if t.signed() {
        (v < 0, v.unsigned_abs())
    } else {
        (false, v as u128)
    }
}

// The type `e` has on its own, as `rule.type.typing` synthesizes it:
// `None` for a bare unsuffixed literal, which takes its type from its
// context (`[Literal-Type-From-Context]`).
fn synth(e: &Expr, consts: &HashMap<String, Const>) -> Option<Type> {
    match &e.kind {
        // D-0102: a byte literal takes its type from its context.
        ExprKind::IntLit(_, Some(s)) if s == "byte" => None,
        ExprKind::IntLit(_, Some(s)) => IntTy::from_str(s).map(Type::Int),
        ExprKind::FloatLit(_, Some(s)) => Some(if s == "f32" { Type::F32 } else { Type::F64 }),
        ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None) => None,
        ExprKind::BoolLit(_) => Some(Type::Bool),
        ExprKind::StrLit(_) => Some(Type::Str),
        ExprKind::Paren(x) | ExprKind::Unary(_, x) => synth(x, consts),
        ExprKind::Call(c, args) if args.is_empty() => match &c.kind {
            ExprKind::Path(segs, _) => consts.get(&segs.join("::")).map(|k| k.ty.clone()),
            _ => None,
        },
        ExprKind::Binary(op, a, b) => {
            if matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::And | BinOp::Or) {
                return Some(Type::Bool);
            }
            // A literal expression takes its context's type (D-0037).
            if is_literal_expr(e) {
                return None;
            }
            let t = if matches!(op, BinOp::Shl | BinOp::Shr) { synth(a, consts) } else { synth(a, consts).or_else(|| synth(b, consts)) };
            Some(t.unwrap_or_else(|| default_of(a)))
        }
        _ => None,
    }
}

// The type an unsuffixed literal operand defaults to with nothing to take
// one from: `i32`, or `f64` for a float.
fn default_of(e: &Expr) -> Type {
    let mut float = false;
    visit(e, &mut |x| {
        if matches!(x.kind, ExprKind::FloatLit(..)) {
            float = true;
        }
    });
    if float {
        Type::F64
    } else {
        Type::Int(IntTy::I32)
    }
}

// `e`'s value when its context expects `ty` (a bare literal takes it),
// or `None` when this pass does not compute it (the constant then stays
// a call, and the static pass types it as written); `Err` for a
// checked failure.
fn eval(e: &Expr, ty: &Type, consts: &HashMap<String, Const>, folded: &mut HashMap<String, Option<Val>>) -> Result<Option<Val>, String> {
    // An operator's operands are typed as `rule.type.typing` types them:
    // a bare literal takes the other operand's type, or the default.
    let operand_ty = |a: &Expr, b: &Expr, op: BinOp| -> Type {
        let t = if matches!(op, BinOp::Shl | BinOp::Shr) { synth(a, consts) } else { synth(a, consts).or_else(|| synth(b, consts)) };
        // Literal operands only: the context's number type (D-0037).
        t.unwrap_or_else(|| if matches!(ty, Type::Int(_) | Type::F32 | Type::F64) { ty.clone() } else { default_of(a) })
    };
    Ok(match (&e.kind, ty) {
        (ExprKind::Paren(x), _) => return eval(x, ty, consts, folded),
        (ExprKind::IntLit(v, suf), Type::Int(t)) => {
            if suf.as_deref().map_or(false, |s| s != t.name()) {
                return Ok(None);
            }
            int_of(*t, false, *v)
        }
        (ExprKind::FloatLit(v, suf), Type::F32 | Type::F64) => {
            let f32 = *ty == Type::F32;
            if suf.as_deref().map_or(false, |s| s != if f32 { "f32" } else { "f64" }) {
                return Ok(None);
            }
            let v = if f32 { *v as f32 as f64 } else { *v };
            if v.is_infinite() {
                return Ok(None);
            }
            Some(Val::Float(v, f32))
        }
        (ExprKind::BoolLit(b), Type::Bool) => Some(Val::Bool(*b)),
        (ExprKind::StrLit(s), Type::Str) => Some(Val::Str(s.clone())),
        (ExprKind::Call(c, args), _) if args.is_empty() => {
            let ExprKind::Path(segs, _) = &c.kind else { return Ok(None) };
            let k = segs.join("::");
            match consts.get(&k) {
                Some(other) if other.ty == *ty => fold(&k, consts, folded)?,
                _ => None,
            }
        }
        (ExprKind::Unary(UnOp::Neg, x), Type::Int(t)) => {
            // `-L` is one literal (D-0025): `-128` is an `i8`.
            if let ExprKind::IntLit(v, suf) = &x.kind {
                if suf.as_deref().map_or(true, |s| s == t.name()) {
                    return Ok(int_of(*t, true, *v));
                }
            }
            match eval(x, ty, consts, folded)? {
                Some(Val::Int(_, v)) => {
                    if !t.signed() {
                        return Ok(None);
                    }
                    let (neg, mag) = signed_parts(*t, v);
                    Some(int_of(*t, !neg, mag).ok_or("diag.arith-overflow")?)
                }
                _ => None,
            }
        }
        (ExprKind::Unary(UnOp::Neg, x), Type::F32 | Type::F64) => match eval(x, ty, consts, folded)? {
            Some(Val::Float(v, f)) => Some(Val::Float(-v, f)),
            _ => None,
        },
        (ExprKind::Unary(UnOp::Not, x), Type::Bool) => match eval(x, ty, consts, folded)? {
            Some(Val::Bool(b)) => Some(Val::Bool(!b)),
            _ => None,
        },
        (ExprKind::Unary(UnOp::BitNot, x), Type::Int(t)) => match eval(x, ty, consts, folded)? {
            Some(Val::Int(_, v)) => {
                let bw = t.bitwidth(crate::value::ADDR_WIDTH);
                let bits = !(v as u128) & if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 };
                Some(if t.signed() && bw < 128 && bits >> (bw - 1) & 1 == 1 {
                    Val::Int(*t, (bits as i128) - (1i128 << bw))
                } else {
                    Val::Int(*t, bits as i128)
                })
            }
            _ => None,
        },
        (ExprKind::Binary(op, a, b), Type::Bool) if matches!(op, BinOp::And | BinOp::Or) => {
            match (eval(a, ty, consts, folded)?, eval(b, ty, consts, folded)?) {
                (Some(Val::Bool(x)), Some(Val::Bool(y))) => Some(Val::Bool(if *op == BinOp::And { x && y } else { x || y })),
                _ => None,
            }
        }
        (ExprKind::Binary(op, a, b), Type::Int(t)) => {
            if operand_ty(a, b, *op) != *ty {
                return Ok(None);
            }
            let rty = if matches!(op, BinOp::Shl | BinOp::Shr) { Type::Int(IntTy::U32) } else { ty.clone() };
            let (Some(Val::Int(_, x)), Some(Val::Int(_, y))) = (eval(a, ty, consts, folded)?, eval(b, &rty, consts, folded)?) else {
                return Ok(None);
            };
            int_op(*op, *t, x, y)?
        }
        (ExprKind::Binary(op, a, b), Type::F32 | Type::F64) => {
            if operand_ty(a, b, *op) != *ty {
                return Ok(None);
            }
            let (Some(Val::Float(x, f)), Some(Val::Float(y, _))) = (eval(a, ty, consts, folded)?, eval(b, ty, consts, folded)?) else {
                return Ok(None);
            };
            let r = match op {
                BinOp::Add => x + y,
                BinOp::Sub => x - y,
                BinOp::Mul => x * y,
                BinOp::Div => x / y,
                _ => return Ok(None),
            };
            // Each operation rounds in its own type (spec/06).
            let r = if f { r as f32 as f64 } else { r };
            if !r.is_finite() {
                return Ok(None);
            }
            Some(Val::Float(r, f))
        }
        _ => None,
    })
}

// `x op y` in `t`, checked as at run time (spec/06): `Ok(None)` for an
// operator this pass leaves alone.
fn int_op(op: BinOp, t: IntTy, x: i128, y: i128) -> Result<Option<Val>, String> {
    let bw = t.bitwidth(crate::value::ADDR_WIDTH);
    let (xn, xm) = signed_parts(t, x);
    let (yn, ym) = signed_parts(t, y);
    let over = || "diag.arith-overflow".to_string();
    // Mathematical results through i128/u128 with sign-magnitude.
    let from_i = |r: Option<i128>| -> Result<Val, String> {
        let r = r.ok_or_else(over)?;
        int_of(t, r < 0, r.unsigned_abs()).ok_or_else(over)
    };
    let from_u = |r: Option<u128>| -> Result<Val, String> { int_of(t, false, r.ok_or_else(over)?).ok_or_else(over) };
    Ok(Some(match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul if t.signed() => {
            let r = match op {
                BinOp::Add => x.checked_add(y),
                BinOp::Sub => x.checked_sub(y),
                _ => x.checked_mul(y),
            };
            from_i(r)?
        }
        BinOp::Add | BinOp::Sub | BinOp::Mul => {
            let (a, b) = (xm, ym);
            let r = match op {
                BinOp::Add => a.checked_add(b),
                BinOp::Sub => a.checked_sub(b),
                _ => a.checked_mul(b),
            };
            from_u(r)?
        }
        BinOp::Div | BinOp::Rem => {
            if ym == 0 {
                return Err("diag.div-by-zero".into());
            }
            if t.signed() {
                if op == BinOp::Div && x == range(t).0 && y == -1 {
                    return Err("diag.div-overflow".into());
                }
                from_i(if op == BinOp::Div { x.checked_div(y) } else { x.checked_rem(y) })?
            } else {
                from_u(if op == BinOp::Div { xm.checked_div(ym) } else { xm.checked_rem(ym) })?
            }
        }
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
            let r = match op {
                BinOp::BitAnd => x & y,
                BinOp::BitOr => x | y,
                _ => x ^ y,
            };
            Val::Int(t, r)
        }
        BinOp::Shl | BinOp::Shr => {
            if ym >= bw as u128 || yn {
                return Err("diag.shift-amount-out-of-range".into());
            }
            let s = ym as u32;
            let mask = if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 };
            let bits = (x as u128) & mask;
            let r = if op == BinOp::Shl { (bits << s) & mask } else if t.signed() && xn { ((x >> s) as u128) & mask } else { bits >> s };
            if t.signed() && bw < 128 && (r >> (bw - 1)) & 1 == 1 {
                Val::Int(t, (r as i128) - (1i128 << bw))
            } else {
                Val::Int(t, r as i128)
            }
        }
        _ => return Ok(None),
    }))
}

// A folded value as the literal that means it, typed by its suffix.
fn literal(v: &Val) -> Expr {
    let mk = |kind: ExprKind| Expr { kind, line: 0 };
    match v {
        Val::Int(t, x) => {
            let (neg, mag) = signed_parts(*t, *x);
            let lit = mk(ExprKind::IntLit(mag, Some(t.name().to_string())));
            if neg {
                mk(ExprKind::Unary(UnOp::Neg, Box::new(lit)))
            } else {
                lit
            }
        }
        Val::Float(f, is32) => {
            let lit = mk(ExprKind::FloatLit(f.abs(), Some(if *is32 { "f32" } else { "f64" }.to_string())));
            if f.is_sign_negative() {
                mk(ExprKind::Unary(UnOp::Neg, Box::new(lit)))
            } else {
                lit
            }
        }
        Val::Bool(b) => mk(ExprKind::BoolLit(*b)),
        Val::Str(s) => mk(ExprKind::StrLit(s.clone())),
    }
}

fn rewrite_items(items: &mut [Item], lits: &HashMap<String, Expr>) {
    for it in items.iter_mut() {
        match it {
            Item::Fn(f) => {
                let decl = std::sync::Arc::get_mut(f).expect("fresh AST: item uniquely owned");
                rewrite_block(&mut decl.body, lits);
            }
            Item::Module(_, _, inner, _) => rewrite_items(inner, lits),
            _ => {}
        }
    }
}

fn rewrite_block(b: &mut Block, lits: &HashMap<String, Expr>) {
    for st in b.stmts.iter_mut() {
        match st {
            Stmt::Let { init: Some(e), .. } | Stmt::Expr(e) | Stmt::BlockLike(e) => rewrite(e, lits),
            Stmt::Destructure { init, .. } => rewrite(init, lits),
            Stmt::Let { init: None, .. } => {}
        }
    }
    if let Some(t) = &mut b.tail {
        rewrite(t, lits);
    }
}

// Every use of a folded constant, `NAME()` after `modres`, becomes its
// literal, keeping the use's line.
fn rewrite(e: &mut Expr, lits: &HashMap<String, Expr>) {
    if let ExprKind::Call(c, args) = &e.kind {
        if let (ExprKind::Path(segs, _), true) = (&c.kind, args.is_empty()) {
            if let Some(l) = lits.get(&segs.join("::")) {
                let line = e.line;
                *e = l.clone();
                set_line(e, line);
                return;
            }
        }
    }
    match &mut e.kind {
        ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => rewrite(a, lits),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            rewrite(a, lits);
            rewrite(b, lits);
        }
        ExprKind::Call(c, args) => {
            rewrite(c, lits);
            args.iter_mut().for_each(|a| rewrite(a, lits));
        }
        ExprKind::StructLit(_, _, fs) => fs.iter_mut().for_each(|(_, a)| rewrite(a, lits)),
        ExprKind::ArrayLit(es) => es.iter_mut().for_each(|a| rewrite(a, lits)),
        ExprKind::ArrayRepeat(a, _) => rewrite(a, lits),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => rewrite_block(b, lits),
        ExprKind::If(c, t, f) => {
            rewrite(c, lits);
            rewrite_block(t, lits);
            if let Some(f) = f {
                rewrite(f, lits);
            }
        }
        ExprKind::While(c, b, step) => {
            rewrite(c, lits);
            rewrite_block(b, lits);
            if let Some(s) = step {
                rewrite(s, lits);
            }
        }
        ExprKind::Match(s, arms) => {
            rewrite(s, lits);
            arms.iter_mut().for_each(|a| {
                // D-0058: a constant in a pattern, folded to its literal.
                if let Some(l) = a.lit.as_mut() {
                    rewrite(l, lits);
                }
                rewrite(&mut a.body, lits)
            });
        }
        ExprKind::Return(Some(a)) => rewrite(a, lits),
        // The body is not yet shared (closures are formed only at run time).
        ExprKind::Closure { body, .. } => rewrite_block(std::sync::Arc::make_mut(body), lits),
        _ => {}
    }
}

fn set_line(e: &mut Expr, line: usize) {
    e.line = line;
    if let ExprKind::Unary(_, a) = &mut e.kind {
        a.line = line;
    }
}

// Every expression in `e`, closures' bodies included.
fn visit(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => visit(a, f),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            visit(a, f);
            visit(b, f);
        }
        ExprKind::Call(c, args) => {
            visit(c, f);
            args.iter().for_each(|a| visit(a, f));
        }
        ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| visit(a, f)),
        ExprKind::ArrayLit(es) => es.iter().for_each(|a| visit(a, f)),
        ExprKind::ArrayRepeat(a, _) => visit(a, f),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => visit_block(b, f),
        ExprKind::If(c, t, e2) => {
            visit(c, f);
            visit_block(t, f);
            if let Some(e2) = e2 {
                visit(e2, f);
            }
        }
        ExprKind::While(c, b, step) => {
            visit(c, f);
            visit_block(b, f);
            if let Some(s) = step {
                visit(s, f);
            }
        }
        ExprKind::Match(s, arms) => {
            visit(s, f);
            arms.iter().for_each(|a| visit(&a.body, f));
        }
        ExprKind::Return(Some(a)) => visit(a, f),
        ExprKind::Closure { body, .. } => visit_block(body, f),
        _ => {}
    }
}

fn visit_block(b: &Block, f: &mut dyn FnMut(&Expr)) {
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


// ---- `const auto` (D-0068) ----

// The declarations a constant expression can name, by qualified name.
struct Decls {
    structs: HashMap<String, std::sync::Arc<StructDecl>>,
    enums: HashMap<String, std::sync::Arc<EnumDecl>>,
    fns: HashMap<String, std::sync::Arc<FnDecl>>,
}

fn gather(items: &[Item], path: &mut Vec<String>, d: &mut Decls) {
    for it in items {
        match it {
            Item::Struct(sd) => {
                d.structs.insert(crate::modres::qualify(path, &sd.name), sd.clone());
            }
            Item::Enum(e) => {
                d.enums.insert(crate::modres::qualify(path, &e.name), e.clone());
            }
            Item::Fn(f) => {
                let n = match &f.assoc_type {
                    Some(t) => format!("{}::{}", t, f.name),
                    None => f.name.clone(),
                };
                d.fns.insert(crate::modres::qualify(path, &n), f.clone());
            }
            Item::Module(_, name, inner, _) => {
                path.push(name.clone());
                gather(inner, path, d);
                path.pop();
            }
            _ => {}
        }
    }
}

// `const auto N = e;`: N's type is the type `auto x = e;` would give x
// (`rule.type.typing`), synthesized from e alone; a constant whose type
// e does not determine (`None`, a generic struct without its type
// arguments) must have it written (`diag.type-mismatch`).
fn resolve_auto(program: &mut Program) -> Result<(), String> {
    let mut d = Decls { structs: HashMap::default(), enums: HashMap::default(), fns: HashMap::default() };
    gather(&program.items, &mut Vec::new(), &mut d);
    if !d.fns.values().any(|f| f.is_const && f.auto_type) {
        return Ok(());
    }
    let mut done: HashMap<String, Type> = HashMap::default();
    let mut keys: Vec<String> = d.fns.iter().filter(|(_, f)| f.is_const && f.auto_type).map(|(k, _)| k.clone()).collect();
    keys.sort();
    for k in &keys {
        let mut active = Vec::new();
        auto_type_of(k, &d, &mut done, &mut active)?;
    }
    write_back(&mut program.items, &mut Vec::new(), &done);
    Ok(())
}

// The type of the constant `k`: written, or synthesized (and memoized).
fn auto_type_of(k: &str, d: &Decls, done: &mut HashMap<String, Type>, active: &mut Vec<String>) -> Result<Option<Type>, String> {
    let Some(f) = d.fns.get(k) else { return Ok(None) };
    if !f.auto_type {
        return Ok(Some(f.ret.clone()));
    }
    if let Some(t) = done.get(k) {
        return Ok(Some(t.clone()));
    }
    if active.iter().any(|a| a == k) {
        // `[Const-Cycle]`.
        return Err(format!("diag.const-not-constant@{}", f.line));
    }
    active.push(k.to_string());
    let init = f.body.tail.as_deref();
    let t = match init {
        Some(e) => infer(e, d, done, active)?,
        None => None,
    };
    active.pop();
    match t {
        Some(t) => {
            done.insert(k.to_string(), t.clone());
            Ok(Some(t))
        }
        None => Err(format!("diag.type-mismatch@{}", f.line)),
    }
}

fn infer(e: &Expr, d: &Decls, done: &mut HashMap<String, Type>, active: &mut Vec<String>) -> Result<Option<Type>, String> {
    use BinOp::*;
    Ok(match &e.kind {
        ExprKind::IntLit(_, Some(sfx)) => Some(Type::Int(IntTy::of_literal(Some(sfx), None).0)),
        ExprKind::IntLit(_, None) => Some(Type::Int(IntTy::I32)),
        ExprKind::FloatLit(_, Some(sfx)) => Some(if sfx == "f32" { Type::F32 } else { Type::F64 }),
        ExprKind::FloatLit(_, None) => Some(Type::F64),
        ExprKind::BoolLit(_) => Some(Type::Bool),
        ExprKind::StrLit(_) => Some(Type::Str),
        ExprKind::Unit => Some(Type::Void),
        ExprKind::Paren(x) => infer(x, d, done, active)?,
        ExprKind::Unary(UnOp::Not, _) => Some(Type::Bool),
        ExprKind::Unary(_, x) => infer(x, d, done, active)?,
        ExprKind::Binary(op, a, b) => {
            if matches!(op, Eq | Ne | Lt | Le | Gt | Ge | And | Or) {
                return Ok(Some(Type::Bool));
            }
            // A bare literal operand takes the other operand's type; two
            // literals with no expected type take the default (D-0037).
            let ta = if is_literal_expr(a) { None } else { infer(a, d, done, active)? };
            if matches!(op, Shl | Shr) {
                return Ok(Some(ta.unwrap_or_else(|| default_of(a))));
            }
            let tb = if is_literal_expr(b) { None } else { infer(b, d, done, active)? };
            Some(ta.or(tb).unwrap_or_else(|| default_of(e)))
        }
        ExprKind::Call(c, args) => {
            let ExprKind::Path(segs, targs) = &c.kind else { return Ok(None) };
            let k = segs.join("::");
            if args.is_empty() {
                if let Some(f) = d.fns.get(&k) {
                    if f.is_const {
                        return auto_type_of(&k, d, done, active);
                    }
                }
            }
            match k.as_str() {
                "sizeof" | "alignof" => Some(Type::Int(IntTy::Usize)),
                "min_value" | "max_value" | "widen" | "narrow" | "narrow_wrapping" | "reinterpret" | "to_float" | "to_int" => targs.first().cloned(),
                "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add" | "saturating_sub" | "saturating_mul" => {
                    let mut t = None;
                    for a in args {
                        if !is_literal_expr(a) {
                            t = infer(a, d, done, active)?;
                            break;
                        }
                    }
                    Some(t.unwrap_or_else(|| args.first().map(default_of).unwrap_or(Type::Int(IntTy::I32))))
                }
                _ => {
                    // An enum variant with a payload.
                    match variant_enum(segs, d) {
                        Some((en, v, key)) => {
                            if en.type_params.is_empty() {
                                Some(Type::Named(key, Vec::new()))
                            } else if en.type_params.len() == 1 && matches!(&v.payload, Some(Type::Named(p, a)) if a.is_empty() && *p == en.type_params[0]) {
                                match args.first() {
                                    Some(x) => infer(x, d, done, active)?.map(|t| Type::Named(key, vec![t])),
                                    None => None,
                                }
                            } else {
                                None
                            }
                        }
                        None => None,
                    }
                }
            }
        }
        ExprKind::Path(segs, _) => {
            // A unit variant of a non-generic enum, or a function's name.
            if let Some((en, _, key)) = variant_enum(segs, d) {
                if en.type_params.is_empty() {
                    return Ok(Some(Type::Named(key, Vec::new())));
                }
                return Ok(None);
            }
            match d.fns.get(&segs.join("::")) {
                Some(f) if !f.is_const && f.type_params.is_empty() => Some(Type::Fn(f.params.iter().map(|p| p.ty.clone()).collect(), Box::new(f.ret.clone()))),
                _ => None,
            }
        }
        ExprKind::StructLit(segs, targs, _) => {
            let k = segs.join("::");
            match d.structs.get(&k) {
                Some(sd) if sd.type_params.is_empty() => Some(Type::Named(k, Vec::new())),
                Some(sd) if targs.len() == sd.type_params.len() => Some(Type::Named(k, targs.clone())),
                _ => None,
            }
        }
        ExprKind::ArrayLit(es) => {
            let mut t = None;
            for x in es {
                if !is_literal_expr(x) {
                    t = infer(x, d, done, active)?;
                    break;
                }
            }
            let t = match t {
                Some(t) => t,
                None => match es.first() {
                    Some(x) => default_of(x),
                    None => return Ok(None),
                },
            };
            Some(Type::Array(Box::new(t), es.len() as u128))
        }
        ExprKind::ArrayRepeat(x, n) => {
            let t = if is_literal_expr(x) { Some(default_of(x)) } else { infer(x, d, done, active)? };
            t.map(|t| Type::Array(Box::new(t), *n))
        }
        ExprKind::Field(x, f) => match infer(x, d, done, active)? {
            Some(Type::Named(n, args)) => match d.structs.get(&n) {
                Some(sd) => {
                    let subst: HashMap<String, Type> = sd.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    sd.fields.iter().find(|fd| &fd.name == f).map(|fd| subst_type(&fd.ty, &subst))
                }
                None => None,
            },
            _ => None,
        },
        ExprKind::Index(x, _) => match infer(x, d, done, active)? {
            Some(Type::Array(t, _)) => Some(*t),
            _ => None,
        },
        _ => None,
    })
}

// The enum a variant path names (`M::E::V`, or `V` alone when exactly one
// enum has a variant of that name), its qualified name, and the variant.
fn variant_enum<'a>(segs: &[String], d: &'a Decls) -> Option<(&'a EnumDecl, &'a VariantDecl, String)> {
    let last = segs.last()?;
    if segs.len() >= 2 {
        let key = segs[..segs.len() - 1].join("::");
        let en = d.enums.get(&key)?;
        let v = en.variants.iter().find(|v| &v.name == last)?;
        return Some((&**en, v, key));
    }
    let mut found = d.enums.iter().filter_map(|(k, en)| en.variants.iter().find(|v| &v.name == last).map(|v| (&**en, v, k.clone())));
    let one = found.next()?;
    if found.next().is_some() {
        return None;
    }
    Some(one)
}

fn subst_type(t: &Type, s: &HashMap<String, Type>) -> Type {
    match t {
        Type::Named(n, a) if a.is_empty() && s.contains_key(n) => s[n].clone(),
        Type::Named(n, a) => Type::Named(n.clone(), a.iter().map(|x| subst_type(x, s)).collect()),
        Type::Array(x, n) => Type::Array(Box::new(subst_type(x, s)), *n),
        Type::Ref(x, m) => Type::Ref(Box::new(subst_type(x, s)), m.clone()),
        other => other.clone(),
    }
}

fn write_back(items: &mut [Item], path: &mut Vec<String>, done: &HashMap<String, Type>) {
    for it in items.iter_mut() {
        match it {
            Item::Fn(f) if f.is_const && f.auto_type => {
                if let Some(t) = done.get(&crate::modres::qualify(path, &f.name)) {
                    let m = std::sync::Arc::make_mut(f);
                    m.ret = t.clone();
                    m.auto_type = false;
                }
            }
            Item::Module(_, name, inner, _) => {
                path.push(name.clone());
                write_back(inner, path, done);
                path.pop();
            }
            _ => {}
        }
    }
}
