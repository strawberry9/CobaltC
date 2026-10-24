// D-0258: statics and addressed constants as C globals. A static
// (`static Atomic<T> NAME = literal;`) is one object for the program's
// whole run that no path ever writes except through its own atomic
// operations, and an addressed constant (`&K`, plain data written as a
// literal) is read-only data: each is a C global of the program's
// translation unit, and a borrow of it is its address with token 0, as a
// `str`'s slice is -- static data that never ends and no path can write,
// so every check on it is proven.

use super::*;

impl<'a> Gen<'a> {
    // The C expression of `&NAME`, and the static's type.
    pub(super) fn static_ref(&mut self, key: &str) -> R<(String, Type)> {
        let Some(decl) = self.items.statics.get(key).cloned() else { return internal("a borrow of an unknown static") };
        let cname = format!("g_{}", san(key));
        // Not while probing: the probe's buffers are discarded (its
        // `strings` restored), so the definition would be lost.
        if self.probing == 0 && self.globals.insert(cname.clone()) {
            let ct = self.ctype(&decl.ty)?;
            let init = self.static_init(&decl)?;
            writeln!(self.strings, "static {} {} = {};", ct, cname, init).unwrap();
        }
        Ok((format!("((cb_ref){{ (void *)&{}, 0 }})", cname), decl.ty.clone()))
    }

    // The C expression of `&K`, and the constant's type.
    pub(super) fn const_ref(&mut self, key: &str) -> R<(String, Type)> {
        let Some(f) = self.items.fns.get(key).filter(|f| f.is_const).cloned() else { return internal("a borrow of an unknown constant") };
        let Some(init) = f.body.tail.as_ref() else { return internal("a constant without a value") };
        let cname = format!("k_{}", san(key));
        if self.probing == 0 && self.globals.insert(cname.clone()) {
            let ct = self.ctype(&f.ret)?;
            let lit = self.c_literal(init, &f.ret)?;
            writeln!(self.strings, "static const {} {} = {};", ct, cname, lit).unwrap();
        }
        Ok((format!("((cb_ref){{ (void *)&{}, 0 }})", cname), f.ret.clone()))
    }

    // An atomic's initializer: `Atomic<T> { .atomic_value = v }`, or
    // `AtomicBool { .atomic_flag = Atomic<u8> { .atomic_value = 0 or 1 } }`.
    fn static_init(&mut self, decl: &StaticDecl) -> R<String> {
        let v: i128 = match &decl.init.kind {
            ExprKind::IntLit(v, _) => *v as i128,
            ExprKind::BoolLit(b) => if *b { 1 } else { 0 },
            ExprKind::Unary(UnOp::Neg, inner) => match &inner.kind {
                ExprKind::IntLit(v, _) => -(*v as i128),
                _ => return internal("a static's initializer"),
            },
            _ => return internal("a static's initializer"),
        };
        Ok(match &decl.ty {
            Type::Named(n, a) if n == "std::Atomic" => match a.first() {
                Some(Type::Int(it)) => format!("{{ {} }}", c_int(*it, v)),
                _ => return internal("a static's lane type"),
            },
            Type::Named(n, _) if n == "std::AtomicBool" => format!("{{ {{ {} }} }}", c_int(IntTy::U8, v)),
            _ => return internal("a static's type"),
        })
    }

    // A literal tree as a C initializer of `ty`: numbers and `bool` as
    // scalars of their C type, an array as its struct's `a` member, a
    // struct by designated fields.
    fn c_literal(&mut self, e: &Expr, ty: &Type) -> R<String> {
        Ok(match (&e.kind, ty) {
            (ExprKind::Paren(inner), _) => self.c_literal(inner, ty)?,
            (ExprKind::IntLit(v, _), Type::Int(it)) => c_int(*it, *v as i128),
            (ExprKind::IntLit(v, _), Type::F32) => format!("{}.0f", v),
            (ExprKind::IntLit(v, _), Type::F64) => format!("{}.0", v),
            (ExprKind::FloatLit(v, _), Type::F32) => format!("{:e}f", v),
            (ExprKind::FloatLit(v, _), Type::F64) => format!("{:e}", v),
            (ExprKind::BoolLit(b), Type::Bool) => (if *b { "1" } else { "0" }).to_string(),
            (ExprKind::Unary(UnOp::Neg, inner), _) => format!("(-{})", self.c_literal(inner, ty)?),
            (ExprKind::ArrayLit(es), Type::Array(elem, _)) => {
                let mut parts = Vec::new();
                for x in es {
                    parts.push(self.c_literal(x, elem)?);
                }
                format!("{{ {{ {} }} }}", parts.join(", "))
            }
            (ExprKind::ArrayRepeat(x, n), Type::Array(elem, _)) => {
                let one = self.c_literal(x, elem)?;
                let parts: Vec<String> = (0..*n).map(|_| one.clone()).collect();
                format!("{{ {{ {} }} }}", parts.join(", "))
            }
            (ExprKind::StructLit(_, _, fs), Type::Named(name, args)) => {
                let Some(decl) = self.struct_decl(name) else { return internal("a constant's struct type") };
                let subst: HashMap<String, Type> = decl.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                let mut parts = Vec::new();
                for fd in &decl.fields {
                    let Some((_, x)) = fs.iter().find(|(n, _)| *n == fd.name) else { return internal("a constant's struct literal misses a field") };
                    let ft = subst_type(&fd.ty, &subst);
                    parts.push(format!(".{} = {}", san(&fd.name), self.c_literal(x, &ft)?));
                }
                format!("{{ {} }}", parts.join(", "))
            }
            _ => return internal("a constant's literal"),
        })
    }
}

// A struct's field type with its type arguments in place.
fn subst_type(t: &Type, subst: &HashMap<String, Type>) -> Type {
    match t {
        Type::Named(n, a) if a.is_empty() => subst.get(n).cloned().unwrap_or_else(|| t.clone()),
        Type::Named(n, a) => Type::Named(n.clone(), a.iter().map(|x| subst_type(x, subst)).collect()),
        Type::Array(e, n) => Type::Array(Box::new(subst_type(e, subst)), *n),
        Type::Rawptr(e) => Type::Rawptr(Box::new(subst_type(e, subst))),
        Type::Simd(e, n) => Type::Simd(Box::new(subst_type(e, subst)), *n),
        other => other.clone(),
    }
}
