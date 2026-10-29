// A canonical text form of a parsed program's own part (CHG-0350): the
// items after the prelude's, and its tests. `coby --dump-ast` prints it,
// and the parser written in CobaltC (`impl/selfhost/parser`) prints the
// same text, so the two parsers are compared output for output. One
// S-expression per node; every expression carries its `line:col`; a
// string is written as its bytes in hex, a float as its bits, so that the
// text needs no escaping and no float formatting.

use crate::ast::*;

// The line the program's own text begins on, subtracted from every line
// shown (the prelude comes first in the text the parser reads).
thread_local! {
    static BASE: std::cell::Cell<usize> = std::cell::Cell::new(0);
}

fn ln(l: usize) -> usize {
    l.saturating_sub(BASE.with(|b| b.get()))
}

pub fn program(p: &Program, skip_items: usize, skip_tests: usize, base: usize) -> String {
    BASE.with(|b| b.set(base));
    let mut o = String::new();
    for it in p.items.iter().skip(skip_items) {
        item(&mut o, it, 0);
    }
    for t in p.tests.iter().skip(skip_tests) {
        o.push_str(&format!("(test {} {} {} {})\n", path_text(&t.module), t.fn_name, opt_hex(t.name.as_deref()), ln(t.line)));
    }
    o
}

fn path_text(segs: &[String]) -> String {
    if segs.is_empty() {
        "-".to_string()
    } else {
        segs.join("::")
    }
}

fn hex(s: &[u8]) -> String {
    let mut o = String::from("x");
    for b in s {
        o.push_str(&format!("{:02x}", b));
    }
    o
}

fn opt_hex(s: Option<&str>) -> String {
    match s {
        Some(s) => hex(s.as_bytes()),
        None => "-".to_string(),
    }
}

fn opt_str(s: Option<&str>) -> String {
    s.map_or("-".to_string(), |s| s.to_string())
}

fn ind(o: &mut String, depth: usize) {
    for _ in 0..depth {
        o.push_str("  ");
    }
}

pub fn ty(t: &Type) -> String {
    match t {
        Type::Int(i) => i.name().to_string(),
        Type::F32 => "f32".into(),
        Type::F64 => "f64".into(),
        Type::Bool => "bool".into(),
        Type::Str => "str".into(),
        Type::Void => "void".into(),
        Type::Never => "never".into(),
        Type::Ref(x, m) => format!("(ref {} {})", ty(x), mode(m)),
        Type::Slice(x, m) => format!("(slice {} {})", ty(x), mode(m)),
        Type::Rawptr(x) => format!("(rawptr {})", ty(x)),
        Type::Array(x, n) => format!("(array {} {})", ty(x), n),
        Type::Simd(x, n) => format!("(simd {} {})", ty(x), n),
        Type::Fn(ps, r) => format!("(fn ({}) {})", ps.iter().map(ty).collect::<Vec<_>>().join(" "), ty(r)),
        Type::Handle(x) => format!("(handle {})", ty(x)),
        Type::Mutex(x) => format!("(mutex {})", ty(x)),
        Type::Guard(x) => format!("(guard {})", ty(x)),
        Type::Named(n, a) => format!("(named {} ({}))", n, a.iter().map(ty).collect::<Vec<_>>().join(" ")),
        Type::Closure(id) => format!("(closure {})", id),
    }
}

fn mode(m: &Mode) -> &'static str {
    match m {
        Mode::Shared => "shared",
        Mode::Exclusive => "exclusive",
    }
}

fn params(ps: &[Param]) -> String {
    format!("({})", ps.iter().map(|p| format!("({} {})", ty(&p.ty), p.name)).collect::<Vec<_>>().join(" "))
}

fn bound(b: &Option<Bound>) -> String {
    b.map_or("-".to_string(), |b| b.name().to_string())
}

fn item(o: &mut String, it: &Item, d: usize) {
    ind(o, d);
    match it {
        Item::Fn(f) => {
            o.push_str(&format!(
                "(fn {} {} {} ({}) ({}) {} {} line {} const {} auto {} derived {} c {}\n",
                f.export,
                opt_str(f.assoc_type.as_deref()),
                f.name,
                f.type_params.join(" "),
                f.type_bounds.iter().map(bound).collect::<Vec<_>>().join(" "),
                params(&f.params),
                ty(&f.ret),
                ln(f.line),
                f.is_const,
                f.auto_type,
                f.derived,
                f.c_entry
            ));
            block(o, &f.body, d + 1);
            ind(o, d);
            o.push_str(")\n");
        }
        Item::Extern(e) => o.push_str(&format!("(extern {} {} {} {} line {})\n", e.export, e.name, params(&e.params), ty(&e.ret), ln(e.line))),
        Item::Static(s) => {
            o.push_str(&format!("(static {} {} {} line {}\n", s.export, s.name, ty(&s.ty), ln(s.line)));
            expr(o, &s.init, d + 1);
            ind(o, d);
            o.push_str(")\n");
        }
        Item::Struct(s) => {
            o.push_str(&format!(
                "(struct {} {} {} ({}) line {} bits {} align {}\n",
                s.export,
                s.resource,
                s.name,
                s.type_params.join(" "),
                ln(s.line),
                s.bits.map_or("-".to_string(), |b| b.name().to_string()),
                s.align.map_or("-".to_string(), |a| a.to_string())
            ));
            for f in &s.fields {
                ind(o, d + 1);
                o.push_str(&format!("(field {} {} {} {})\n", f.export, ty(&f.ty), f.name, f.width.map_or("-".to_string(), |w| w.to_string())));
            }
            ind(o, d);
            o.push_str(")\n");
        }
        Item::Enum(e) => {
            o.push_str(&format!("(enum {} {} {} ({}) line {}\n", e.export, e.resource, e.name, e.type_params.join(" "), ln(e.line)));
            for v in &e.variants {
                ind(o, d + 1);
                o.push_str(&format!("(variant {} {})\n", v.name, v.payload.as_ref().map_or("-".to_string(), ty)));
            }
            ind(o, d);
            o.push_str(")\n");
        }
        Item::Module(export, name, items, line) => {
            o.push_str(&format!("(module {} {} line {}\n", export, name, ln(*line)));
            for it in items {
                item(o, it, d + 1);
            }
            ind(o, d);
            o.push_str(")\n");
        }
        Item::Import(export, path, line) => o.push_str(&format!("(import {} {} line {})\n", export, path, ln(*line))),
        Item::ExternCode(s, line) => o.push_str(&format!("(extern-code {} line {})\n", hex(s.as_bytes()), ln(*line))),
    }
}

fn block(o: &mut String, b: &Block, d: usize) {
    ind(o, d);
    o.push_str("(block\n");
    for s in &b.stmts {
        stmt(o, s, d + 1);
    }
    match &b.tail {
        Some(t) => {
            ind(o, d + 1);
            o.push_str("(tail\n");
            expr(o, t, d + 2);
            ind(o, d + 1);
            o.push_str(")\n");
        }
        None => {}
    }
    ind(o, d);
    o.push_str(")\n");
}

fn stmt(o: &mut String, s: &Stmt, d: usize) {
    ind(o, d);
    match s {
        Stmt::Let { ty: t, name, init, line, col } => {
            o.push_str(&format!("(let {} {} @{}:{}\n", t.as_ref().map_or("auto".to_string(), ty), name, ln(*line), col));
            if let Some(e) = init {
                expr(o, e, d + 1);
            }
            ind(o, d);
            o.push_str(")\n");
        }
        Stmt::Destructure { struct_name, fields, names, rest, init } => {
            o.push_str(&format!("(destructure {} ({}) ({}) {}\n", struct_name, fields.join(" "), names.join(" "), rest));
            expr(o, init, d + 1);
            ind(o, d);
            o.push_str(")\n");
        }
        Stmt::Expr(e) => {
            o.push_str("(expr\n");
            expr(o, e, d + 1);
            ind(o, d);
            o.push_str(")\n");
        }
        Stmt::BlockLike(e) => {
            o.push_str("(blocklike\n");
            expr(o, e, d + 1);
            ind(o, d);
            o.push_str(")\n");
        }
    }
}

fn binop(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

fn unop(op: UnOp) -> &'static str {
    match op {
        UnOp::Neg => "neg",
        UnOp::Not => "not",
        UnOp::BitNot => "bitnot",
    }
}

fn targs(ts: &[Type]) -> String {
    format!("<{}>", ts.iter().map(ty).collect::<Vec<_>>().join(" "))
}

// An expression: `(KIND @line:col …)`, its parts on the lines below.
fn expr(o: &mut String, e: &Expr, d: usize) {
    ind(o, d);
    let at = format!("@{}:{}", ln(e.line), e.col);
    let open = |o: &mut String, head: String| {
        o.push_str(&head);
        o.push('\n');
    };
    let close = |o: &mut String| {
        ind(o, d);
        o.push_str(")\n");
    };
    match &e.kind {
        ExprKind::IntLit(v, s) => o.push_str(&format!("(int {} {} {})\n", at, v, opt_str(s.as_deref()))),
        ExprKind::FloatLit(v, s) => o.push_str(&format!("(float {} {:016x} {})\n", at, v.to_bits(), opt_str(s.as_deref()))),
        ExprKind::StrLit(s) => o.push_str(&format!("(str {} {})\n", at, hex(s.as_bytes()))),
        ExprKind::BoolLit(b) => o.push_str(&format!("(bool {} {})\n", at, b)),
        ExprKind::Unit => o.push_str(&format!("(unit {})\n", at)),
        ExprKind::Dollar => o.push_str(&format!("(dollar {})\n", at)),
        ExprKind::Break => o.push_str(&format!("(break {})\n", at)),
        ExprKind::Continue => o.push_str(&format!("(continue {})\n", at)),
        ExprKind::Path(segs, ts) => o.push_str(&format!("(path {} {} {})\n", at, segs.join("::"), targs(ts))),
        ExprKind::StructLit(segs, ts, fields) => {
            open(o, format!("(structlit {} {} {}", at, segs.join("::"), targs(ts)));
            for (f, x) in fields {
                ind(o, d + 1);
                o.push_str(&format!("(set {}\n", f));
                expr(o, x, d + 2);
                ind(o, d + 1);
                o.push_str(")\n");
            }
            close(o);
        }
        ExprKind::ArrayLit(xs) => {
            open(o, format!("(arraylit {}", at));
            xs.iter().for_each(|x| expr(o, x, d + 1));
            close(o);
        }
        ExprKind::ArrayRepeat(x, n) => {
            open(o, format!("(arrayrepeat {} {}", at, n));
            expr(o, x, d + 1);
            close(o);
        }
        ExprKind::Unary(op, x) => {
            open(o, format!("(unary {} {}", at, unop(*op)));
            expr(o, x, d + 1);
            close(o);
        }
        ExprKind::Binary(op, a, b) => {
            open(o, format!("(binary {} {}", at, binop(*op)));
            expr(o, a, d + 1);
            expr(o, b, d + 1);
            close(o);
        }
        ExprKind::Borrow(m, x) => {
            open(o, format!("(borrow {} {}", at, mode(m)));
            expr(o, x, d + 1);
            close(o);
        }
        ExprKind::SliceOf(m, a, lo, hi) => {
            open(o, format!("(sliceof {} {}", at, mode(m)));
            expr(o, a, d + 1);
            expr(o, lo, d + 1);
            expr(o, hi, d + 1);
            close(o);
        }
        ExprKind::Deref(x) => {
            open(o, format!("(deref {}", at));
            expr(o, x, d + 1);
            close(o);
        }
        ExprKind::Field(x, f) => {
            open(o, format!("(field {} {}", at, f));
            expr(o, x, d + 1);
            close(o);
        }
        ExprKind::Index(a, b) => {
            open(o, format!("(index {}", at));
            expr(o, a, d + 1);
            expr(o, b, d + 1);
            close(o);
        }
        ExprKind::Call(f, args) => {
            open(o, format!("(call {}", at));
            expr(o, f, d + 1);
            args.iter().for_each(|x| expr(o, x, d + 1));
            close(o);
        }
        ExprKind::Assign(a, b) => {
            open(o, format!("(assign {}", at));
            expr(o, a, d + 1);
            expr(o, b, d + 1);
            close(o);
        }
        ExprKind::Propagate(x) => {
            open(o, format!("(propagate {}", at));
            expr(o, x, d + 1);
            close(o);
        }
        ExprKind::Paren(x) => {
            open(o, format!("(paren {}", at));
            expr(o, x, d + 1);
            close(o);
        }
        ExprKind::Block(b) => {
            open(o, format!("(blockexpr {}", at));
            block(o, b, d + 1);
            close(o);
        }
        ExprKind::Unsafe(b) => {
            open(o, format!("(unsafe {}", at));
            block(o, b, d + 1);
            close(o);
        }
        ExprKind::If(c, b, els) => {
            open(o, format!("(if {}", at));
            expr(o, c, d + 1);
            block(o, b, d + 1);
            if let Some(x) = els {
                expr(o, x, d + 1);
            }
            close(o);
        }
        ExprKind::While(c, b, step) => {
            open(o, format!("(while {}", at));
            expr(o, c, d + 1);
            block(o, b, d + 1);
            if let Some(x) = step {
                expr(o, x, d + 1);
            }
            close(o);
        }
        ExprKind::Match(x, arms) => {
            open(o, format!("(match {}", at));
            expr(o, x, d + 1);
            for a in arms {
                ind(o, d + 1);
                o.push_str(&format!("(arm {} ({}) {}\n", opt_str(a.variant.as_deref()), a.nested.join(" "), opt_str(a.binder.as_deref())));
                if let Some(l) = &a.lit {
                    ind(o, d + 2);
                    o.push_str("(lit\n");
                    expr(o, l, d + 3);
                    ind(o, d + 2);
                    o.push_str(")\n");
                }
                expr(o, &a.body, d + 2);
                ind(o, d + 1);
                o.push_str(")\n");
            }
            close(o);
        }
        ExprKind::Return(x) => {
            open(o, format!("(return {}", at));
            if let Some(x) = x {
                expr(o, x, d + 1);
            }
            close(o);
        }
        ExprKind::Closure { is_move, captures, params: ps, ret, body } => {
            open(o, format!("(closure {} {} ({}) {} {}", at, is_move, captures.join(" "), params(ps), ret.as_ref().map_or("-".to_string(), ty)));
            block(o, body, d + 1);
            close(o);
        }
    }
}
