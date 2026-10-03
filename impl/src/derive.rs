// D-0116: a derived `T::clone` for every struct or enum that declares
// none and is not marked `resource` — ordinary CobaltC, added before name
// resolution so both tools compile it like any function:
//
//     fn S::clone<A: clone, …>(ref<S<A, …>, shared> self) : S<A, …>
//     {
//         S { .f1 = clone(&self.f1), … }
//     }
//     fn E::clone<…>(ref<E<…>, shared> self) : E<…>
//     {
//         match (self) { V1(p) : E::V1(clone(p)), V2 : E::V2, … }
//     }
//
// Whether `S` *is* `clone` is decided by the checker from its fields
// (`Ck::clone_problem`): a derived function whose type is not `clone` is
// never reached, since the call is rejected first. A type whose fields
// are all plain (integers, references, …) needs no function — reading it
// copies it — so none is made for it; a `resource`-marked type declares
// its own `clone` or has none.
use crate::ast::*;
use std::sync::Arc;

pub fn add_derived_clones(program: &mut Program) {
    add_in(&mut program.items);
}

fn add_in(items: &mut Vec<Item>) {
    let declared: std::collections::HashSet<String> = items
        .iter()
        .filter_map(|it| match it {
            Item::Fn(f) if f.name == "clone" => f.assoc_type.clone(),
            _ => None,
        })
        .collect();
    let mut made = Vec::new();
    for it in items.iter() {
        match it {
            // D-0118: a `bitstruct`'s `Name::bits` and `Name::from_bits`.
            Item::Struct(s) if s.bits.is_some() => {
                made.push(Item::Fn(Arc::new(bitstruct_bits(s))));
                made.push(Item::Fn(Arc::new(bitstruct_from_bits(s))));
            }
            Item::Struct(s) if !s.resource && !declared.contains(&s.name) && s.fields.iter().any(|f| may_own(&f.ty)) => {
                made.push(Item::Fn(Arc::new(struct_clone(s))));
            }
            Item::Enum(e) if !e.resource && !declared.contains(&e.name) && e.variants.iter().any(|v| v.payload.as_ref().map_or(false, may_own)) => {
                made.push(Item::Fn(Arc::new(enum_clone(e))));
            }
            _ => {}
        }
    }
    items.extend(made);
    for it in items.iter_mut() {
        if let Item::Module(_, _, sub, _) = it {
            add_in(sub);
        }
    }
}

// A field type that may own a resource, so a read is not a copy: a
// named type (a struct, an enum, `String`, `Vec<…>`, …) or an array of
// one. Everything else is copied by a read.
fn may_own(t: &Type) -> bool {
    match t {
        Type::Named(..) => true,
        Type::Array(e, _) => may_own(e),
        _ => false,
    }
}

fn self_type(name: &str, params: &[String]) -> Type {
    Type::Named(name.to_string(), params.iter().map(|p| Type::Named(p.clone(), Vec::new())).collect())
}

fn path(segs: &[&str], line: usize) -> Expr {
    Expr { kind: ExprKind::Path(segs.iter().map(|s| s.to_string()).collect(), Vec::new()), line }
}

fn clone_of(arg: Expr, line: usize) -> Expr {
    Expr { kind: ExprKind::Call(Box::new(path(&["clone"], line)), vec![arg]), line }
}

fn header(name: &str, params: &[String], line: usize, body: Block, export: bool) -> FnDecl {
    FnDecl {
        export,
        assoc_type: Some(name.to_string()),
        name: "clone".to_string(),
        type_params: params.to_vec(),
        type_bounds: params.iter().map(|_| Some(Bound::Clone)).collect(),
        params: vec![Param { ty: Type::Ref(Box::new(self_type(name, params)), Mode::Shared), name: "self".to_string() }],
        ret: self_type(name, params),
        body,
        line,
        is_const: false,
        auto_type: false,
        derived: true,
    }
}

// D-0118: `fn Name::bits(Name v) : uN { widen<uN>(v.f0) << 0 | widen<uN>(v.f1) << w0 | … }`
// and `fn Name::from_bits(uN b) : Name { Name { .f0 = narrow<t0>(b & m0), .f1 = narrow<t1>((b >> w0) & m1), … } }`,
// the fields low bit first; a field as wide as the backing type is
// taken as it is.
fn int_lit(v: u128, ty: IntTy, line: usize) -> Expr {
    Expr { kind: ExprKind::IntLit(v, Some(ty.name().to_string())), line }
}

fn call_generic(name: &str, targ: Type, arg: Expr, line: usize) -> Expr {
    Expr { kind: ExprKind::Call(Box::new(Expr { kind: ExprKind::Path(vec![name.to_string()], vec![targ]), line }), vec![arg]), line }
}

fn bin(op: BinOp, a: Expr, b: Expr, line: usize) -> Expr {
    Expr { kind: ExprKind::Binary(op, Box::new(a), Box::new(b)), line }
}

fn bitstruct_bits(s: &StructDecl) -> FnDecl {
    let line = s.line;
    let backing = s.bits.expect("bitstruct");
    let bt = Type::Int(backing);
    let mut acc: Option<Expr> = None;
    let mut off: u32 = 0;
    for f in &s.fields {
        let w = f.width.expect("width");
        let field = Expr { kind: ExprKind::Field(Box::new(path(&["v"], line)), f.name.clone()), line };
        let wide = if f.ty == bt { field } else { call_generic("widen", bt.clone(), field, line) };
        let placed = if off == 0 { wide } else { bin(BinOp::Shl, wide, int_lit(off as u128, IntTy::U32, line), line) };
        acc = Some(match acc {
            None => placed,
            Some(a) => bin(BinOp::BitOr, a, placed, line),
        });
        off += w;
    }
    FnDecl {
        export: s.export,
        assoc_type: Some(s.name.clone()),
        name: "bits".to_string(),
        type_params: Vec::new(),
        type_bounds: Vec::new(),
        params: vec![Param { ty: self_type(&s.name, &[]), name: "v".to_string() }],
        ret: bt,
        body: Block { stmts: Vec::new(), tail: Some(Box::new(acc.expect("a bitstruct has fields"))) },
        line,
        is_const: false,
        auto_type: false,
        derived: true,
    }
}

fn bitstruct_from_bits(s: &StructDecl) -> FnDecl {
    let line = s.line;
    let backing = s.bits.expect("bitstruct");
    let bt = Type::Int(backing);
    let mut fields = Vec::new();
    let mut off: u32 = 0;
    for f in &s.fields {
        let w = f.width.expect("width");
        let mask: u128 = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
        let shifted = if off == 0 { path(&["b"], line) } else { bin(BinOp::Shr, path(&["b"], line), int_lit(off as u128, IntTy::U32, line), line) };
        let masked = bin(BinOp::BitAnd, shifted, int_lit(mask, backing, line), line);
        let value = if f.ty == bt { masked } else { call_generic("narrow", f.ty.clone(), masked, line) };
        fields.push((f.name.clone(), value));
        off += w;
    }
    let lit = Expr { kind: ExprKind::StructLit(vec![s.name.clone()], Vec::new(), fields), line };
    FnDecl {
        export: s.export,
        assoc_type: Some(s.name.clone()),
        name: "from_bits".to_string(),
        type_params: Vec::new(),
        type_bounds: Vec::new(),
        params: vec![Param { ty: bt, name: "b".to_string() }],
        ret: self_type(&s.name, &[]),
        body: Block { stmts: Vec::new(), tail: Some(Box::new(lit)) },
        line,
        is_const: false,
        auto_type: false,
        derived: true,
    }
}

fn struct_clone(s: &StructDecl) -> FnDecl {
    let line = s.line;
    let fields: Vec<(String, Expr)> = s
        .fields
        .iter()
        .map(|f| {
            let place = Expr { kind: ExprKind::Field(Box::new(path(&["self"], line)), f.name.clone()), line };
            let borrowed = Expr { kind: ExprKind::Borrow(Mode::Shared, Box::new(place)), line };
            (f.name.clone(), clone_of(borrowed, line))
        })
        .collect();
    let lit = Expr { kind: ExprKind::StructLit(vec![s.name.clone()], Vec::new(), fields), line };
    header(&s.name, &s.type_params, line, Block { stmts: Vec::new(), tail: Some(Box::new(lit)) }, s.export)
}

fn enum_clone(e: &EnumDecl) -> FnDecl {
    let line = e.line;
    let arms: Vec<Arm> = e
        .variants
        .iter()
        .map(|v| {
            let make = path(&[e.name.as_str(), v.name.as_str()], line);
            match &v.payload {
                Some(_) => Arm {
                    variant: Some(v.name.clone()),
                    nested: Vec::new(),
                    binder: Some("p".to_string()),
                    lit: None,
                    body: Box::new(Expr { kind: ExprKind::Call(Box::new(make), vec![clone_of(path(&["p"], line), line)]), line }),
                },
                None => Arm { variant: Some(v.name.clone()), nested: Vec::new(), binder: None, lit: None, body: Box::new(make) },
            }
        })
        .collect();
    let m = Expr { kind: ExprKind::Match(Box::new(path(&["self"], line)), arms), line };
    header(&e.name, &e.type_params, line, Block { stmts: Vec::new(), tail: Some(Box::new(m)) }, e.export)
}
