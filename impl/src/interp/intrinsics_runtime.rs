// The intrinsics: exit, printing, std's native helpers, assertions and formatting.

use super::*;

impl Interp {
    pub(super) fn intrinsic_runtime(
        &mut self,
        name: &str,
        targs: &[Type],
        args: &[Expr],
        subst: &HashMap<String, Type>,
    ) -> Result<Option<EvalResult>, Flow> {
        let t0 = || -> Type { Type::Void };
        let targ = |i: usize, s: &Interp| -> Type {
            targs.get(i).map(|t| s.apply_subst(t, subst)).unwrap_or_else(t0)
        };
        macro_rules! ok { ($v:expr) => { return Ok(Some(EvalResult::Val($v))) } }
        match name {
            "std::exit" => {
                // `[Exit]` (D-0150): the calling thread unwinds exactly as a
                // fault unwinds it (destructors run, other threads stop), the
                // status carried in the flow behind `EXIT_MARK`; the top of
                // the run (`lib::run_items_raw`) turns it into `ok(status)`.
                if args.len() != 1 {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                }
                let v = self.eval_to_value(&args[0], subst)?;
                let Value::Int(_, s) = v else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                return Err(Flow::Fault(format!("{}{}", EXIT_MARK, s as u8)));
            }
            "std::print" => {
                // `rule.stdlib.print`: `str` and `ref<String, shared>` go to
                // `std`'s two helpers; numbers and `bool` are formatted here.
                if args.len() != 1 {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                }
                let v = self.eval_to_value(&args[0], subst)?;
                let helper = match &v {
                    Value::Str(_) => Some("std::print_str"),
                    Value::Ref { pointee: Type::Named(n, _), .. } if n == "std::String" => Some("std::print_string"),
                    _ => None,
                };
                if let Some(h) = helper {
                    self.invoke_callable(EvalResult::Val(Value::FnVal(h.to_string())), vec![EvalResult::Val(v)], subst)?;
                    ok!(Value::Unit)
                }
                let Some(text) = number_text(&v) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let _ = crate::outbuf::write(text.as_bytes());
                ok!(Value::Unit)
            }
            // `rule.stdlib.text` (spec/21 §2d): `std`'s own helpers for
            // `String::append<T>` and `parse<T>`.
            "append_native" => {
                // `str` and `ref<String, shared>` go to `std`'s two helpers,
                // numbers and `bool` to `append_text` (through `text_write`).
                let s_ref = self.eval(&args[0], subst)?;
                let v = self.eval_to_value(&args[1], subst)?;
                let helper = match &v {
                    Value::Str(_) => "std::String::append_str",
                    Value::Ref { pointee: Type::Named(n, _), .. } if n == "std::String" => "std::String::append_string",
                    Value::Struct(_) => "std::String::append_view", // D-0053
                    _ => "std::String::append_text",
                };
                self.invoke_callable(EvalResult::Val(Value::FnVal(helper.to_string())), vec![s_ref, EvalResult::Val(v)], subst)?;
                ok!(Value::Unit)
            }
            // `foreach` (D-0042): the expansion for the type of the hidden
            // binding the call names first (src/each.rs), evaluated here.
            n if crate::each::NAMES.contains(&n) => {
                let t0 = match args.first().map(|a| &a.kind) {
                    Some(ExprKind::Path(segs, _)) if segs.len() == 1 => {
                        self.lookup(&segs[0]).and_then(|(obj, _)| self.objects.get(&obj).map(|o| o.ty.clone()))
                    }
                    _ => None,
                };
                let Some(t0) = t0 else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let t0 = self.apply_subst(&t0, subst);
                let line = args.first().map_or(0, |a| a.line);
                let x = crate::each::expand(n, &t0, args, line, args.first().map_or(0, |a| a.col)).map_err(Flow::Fault)?;
                return self.eval(&x, subst).map(Some);
            }
            // `swap_places(a, b)` (D-0048): the two places' contents exchange.
            // Both are exclusive references; the same place is left alone.
            // D-0213 `[Thread-Lend]`, std-only: `lend_places(&mut v)` lends
            // the exclusive references `v` holds (`Channel::send`, before
            // the value enters the channel).
            "lend_places" => {
                let Value::Ref { obj, path, token, .. } = self.eval_to_value(&args[0], subst)? else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                if !self.object_live(obj) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                // D-0235: the place is written by `[Write]`, with its premises.
                self.affine(token, true)?;
                if !self.clash_check_write(obj, &path, &[token]) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                let mut v = self.read_place(obj, &path);
                if Self::value_has_ref(&v) {
                    self.lend_value(&mut v)?;
                    self.write_place(obj, &path, v.clone());
                    self.index_refs(obj, &v);
                }
                ok!(Value::Unit)
            }
            // D-0235: each place is written by `[Write]`, with all its
            // premises -- the token's thread (`[Thread-Affine]`) and no
            // conflicting live path (`¬clash(·, exclusive)`, the token
            // itself and its ancestors excluded) -- as `*a = …` would be.
            // The same place twice is no access.
            "swap_places" => {
                let mut places = Vec::new();
                let mut checks = Vec::new();
                for a in args {
                    let Value::Ref { obj, path, token, range, .. } = self.eval_to_value(a, subst)? else {
                        return Err(Flow::Fault("diag.type-mismatch".into()));
                    };
                    if !self.object_live(obj) {
                        return Err(Flow::Fault("diag.stale-binding".into()));
                    }
                    let mut cp = path.clone();
                    if let Some((lo, hi)) = range {
                        cp.push(Proj::Range(lo, hi));
                    }
                    places.push((obj, path));
                    checks.push((obj, cp, token));
                }
                if places[0] != places[1] {
                    for (obj, cp, token) in &checks {
                        self.affine(*token, true)?;
                        if !self.clash_check_write(*obj, cp, &[*token]) {
                            return Err(Flow::Fault("diag.aliasing-conflict".into()));
                        }
                    }
                    let va = self.read_place(places[0].0, &places[0].1);
                    let vb = self.read_place(places[1].0, &places[1].1);
                    self.write_place(places[0].0, &places[0].1, vb);
                    self.write_place(places[1].0, &places[1].1, va);
                }
                ok!(Value::Unit)
            }
            // `rule.stdlib.hashmap` (spec/21 §1a, D-0041): a key's hash and
            // equality, both over its bytes (`key_bytes`).
            // D-0062: the key types' order -- integers by value, `false`
            // before `true`, text by its bytes.
            // D-0062/performance: `key_less_at(v, i, j)`, `key_less` of
            // elements `i` and `j` of the `Vec` `v` refers to, read in
            // place (`v` is borrowed shared for the whole comparison).
            "key_less_at" => {
                let Value::Ref { obj, path, .. } = self.eval_to_value(&args[0], subst)? else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let idx = |me: &mut Self, e: &Expr| -> Result<usize, Flow> {
                    match me.eval_to_value(e, subst)? {
                        Value::Int(_, n) => Ok(n as usize),
                        _ => Err(Flow::Fault("diag.type-mismatch".into())),
                    }
                };
                let (i, j) = (idx(self, &args[1])?, idx(self, &args[2])?);
                let elem = match self.type_at(obj, &path) {
                    Type::Named(n, a) if n == "std::Vec" => a.first().cloned().unwrap_or(Type::Void),
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let ea = self.vec_element(obj, &path, &elem, i)?;
                let eb = self.vec_element(obj, &path, &elem, j)?;
                let mk = |o: u64| Value::Ref { obj: o, path: Vec::new(), mode: Mode::Shared, pointee: elem.clone(), token: 0, range: None, anc: None };
                let (a, b) = (mk(ea), mk(eb));
                return self.key_less_values(a, b).map(|r| Some(EvalResult::Val(Value::Bool(r))));
            }
            "key_less" => {
                let a = self.eval_to_value(&args[0], subst)?;
                let b = self.eval_to_value(&args[1], subst)?;
                let lt = self.key_less_values(a, b)?;
                ok!(Value::Bool(lt))
            }
            "key_hash" | "key_eq" => {
                let mut keys = Vec::new();
                for a in args {
                    let v = self.eval_to_value(a, subst)?;
                    keys.push(self.key_bytes(v)?);
                }
                if name == "key_hash" {
                    let mut h: u64 = 14695981039346656037;
                    for b in &keys[0] {
                        h = (h ^ *b as u64).wrapping_mul(1099511628211);
                    }
                    ok!(Value::Int(IntTy::U64, h as i128))
                }
                ok!(Value::Bool(keys[0] == keys[1]))
            }
            "text_write" => {
                let v = self.eval_to_value(&args[0], subst)?;
                let Value::Rawptr(addr, _) = self.eval_to_value(&args[1], subst)? else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let Some(text) = number_text(&v) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                for (j, b) in text.bytes().enumerate() {
                    self.arena.insert(addr + j as u64, b);
                }
                ok!(Value::Int(IntTy::Usize, text.len() as i128))
            }
            "parse_check" | "parse_value" => {
                let t = targ(0, self);
                let (Value::Rawptr(addr, _), Value::Int(_, n)) = (self.eval_to_value(&args[0], subst)?, self.eval_to_value(&args[1], subst)?) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let bytes: Vec<u8> = (0..n as u64).map(|j| *self.arena.get(&(addr + j)).unwrap_or(&0)).collect();
                let r = match &t {
                    Type::Int(it) => crate::numtext::parse_int(&bytes, it.signed(), it.bitwidth(crate::value::ADDR_WIDTH) as u32)
                        .map(|x| Value::Int(*it, x as i128)),
                    Type::F64 => crate::numtext::parse_float(&bytes, false).map(Value::F64),
                    Type::F32 => crate::numtext::parse_float(&bytes, true).map(|x| Value::F32(x as f32)),
                    Type::Bool => crate::numtext::parse_bool(&bytes).map(Value::Bool),
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                if name == "parse_check" {
                    ok!(Value::Int(IntTy::Isize, match r {
                        Ok(_) => -1,
                        Err(code) => code as i128,
                    }))
                }
                match r {
                    Ok(v) => ok!(v),
                    Err(_) => return Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            // `rule.stdlib.format` (D-0038): `printf`'s and `appendf`'s text
            // (`modres` made them `print`/`append` of `$fmt(…)`).
            "$assert_fail" => {
                // `[Assert-Fail]` (D-0065): the formatted message travels
                // with the fault, as a `static_assert`'s does.
                let Some(a0) = args.first() else {
                    return Err(Flow::Fault("diag.assert-failed".into()));
                };
                let Value::Str(m) = self.eval_to_value(a0, subst)? else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let m = String::from_utf8_lossy(&m).into_owned();
                return Err(Flow::Fault(format!("diag.assert-failed{}{}", crate::diagnostics::DETAIL_SEP, m)));
            }
            "$fmt" => {
                let Value::Str(f) = self.eval_to_value(&args[0], subst)? else {
                    return Err(Flow::Fault("diag.format-invalid".into()));
                };
                let pieces = crate::fmt::parse(&f).map_err(|_| Flow::Fault("diag.format-invalid".into()))?;
                let mut fargs = Vec::new();
                for a in &args[1..] {
                    let r = self.eval(a, subst)?;
                    // D-0100: an enum (or a reference to one) prints as
                    // its variant's name.
                    let enum_name = |me: &Self, t: &Type| -> Option<String> {
                        match t {
                            Type::Named(n, _) if me.items.enums.contains_key(n) => Some(n.clone()),
                            Type::Ref(inner, _) => match &**inner {
                                Type::Named(n, _) if me.items.enums.contains_key(n) => Some(n.clone()),
                                _ => None,
                            },
                            _ => None,
                        }
                    };
                    let en = self.result_type_hint(&r).and_then(|t| enum_name(self, &t));
                    let v = self.result_to_value(r)?;
                    if let Some(en) = en {
                        let v = match v {
                            Value::Ref { obj, path, .. } => {
                                if !self.object_live(obj) {
                                    return Err(Flow::Fault("diag.stale-binding".into()));
                                }
                                self.read_place(obj, &path)
                            }
                            other => other,
                        };
                        let Value::Enum(idx, _) = v else {
                            return Err(Flow::Fault("diag.type-mismatch".into()));
                        };
                        let name = self.items.enums[&en].variants[idx].name.clone();
                        fargs.push(crate::fmt::Arg::Text(name.into_bytes()));
                        continue;
                    }
                    fargs.push(match v {
                        Value::Int(t, x) => crate::fmt::Arg::Int { bits: x as u128, signed: t.signed(), width: t.bitwidth(ADDR_WIDTH) },
                        Value::F64(x) => crate::fmt::Arg::Float(x, false),
                        Value::F32(x) => crate::fmt::Arg::Float(x as f64, true),
                        Value::Str(b) => crate::fmt::Arg::Text(b.to_vec()),
                        Value::Bool(b) => crate::fmt::Arg::Text(if b { b"true".to_vec() } else { b"false".to_vec() }),
                        // D-0053: a `StringView`, the one printable struct.
                        v @ Value::Struct(_) => crate::fmt::Arg::Text(self.view_bytes(&v)?),
                        Value::Ref { obj, path, pointee, .. } => {
                            if !self.object_live(obj) {
                                return Err(Flow::Fault("diag.stale-binding".into()));
                            }
                            if matches!(&pointee, Type::Named(n, _) if n == "std::String") {
                                crate::fmt::Arg::Text(self.string_bytes(obj, &path))
                            } else {
                                // A reference to a number, `bool` or `str`: its value (D-0042).
                                match self.read_place(obj, &path) {
                                    Value::Int(t, x) => crate::fmt::Arg::Int { bits: x as u128, signed: t.signed(), width: t.bitwidth(ADDR_WIDTH) },
                                    Value::F64(x) => crate::fmt::Arg::Float(x, false),
                                    Value::F32(x) => crate::fmt::Arg::Float(x as f64, true),
                                    Value::Str(b) => crate::fmt::Arg::Text(b.to_vec()),
                                    Value::Bool(b) => crate::fmt::Arg::Text(if b { b"true".to_vec() } else { b"false".to_vec() }),
                                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                                }
                            }
                        }
                        _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                    });
                }
                let text = crate::fmt::render(&pieces, &fargs);
                ok!(Value::Str(Arc::from(text.as_slice())))
            }
            "std::Result::map_err" => {
                let r0 = self.eval(&args[0], subst)?;
                let rty = self.result_type_hint(&r0);
                let r = self.result_to_value_resource_aware(r0)?;
                let f = self.eval(&args[1], subst)?;
                // `Result<T, F>`: `?` and `foreach` read the payload's
                // type from it.
                let targs = match &rty {
                    Some(Type::Named(n, a)) if n == "std::Result" && a.len() == 2 => {
                        let fe = match self.result_type_hint(&f) {
                            Some(Type::Fn(_, ret)) => *ret,
                            _ => a[1].clone(),
                        };
                        vec![a[0].clone(), fe]
                    }
                    _ => vec![],
                };
                match r {
                    Value::Enum(0, payload) => {
                        let ty = Type::Named("std::Result".into(), targs);
                        let id = self.new_object(ty, Value::Enum(0, payload));
                        return Ok(Some(EvalResult::Temp(id)));
                    }
                    Value::Enum(1, payload) => {
                        let mapped = self.invoke_callable(f, vec![EvalResult::Val(*payload)], subst)?;
                        // `f`'s result may be a resource (`String`, an error
                        // enum holding one): moved into the new `Err`, as
                        // the `Ok` payload above is.
                        let mv = self.result_to_value_resource_aware(mapped)?;
                        let ty = Type::Named("std::Result".into(), targs);
                        let id = self.new_object(ty, Value::Enum(1, Box::new(mv)));
                        return Ok(Some(EvalResult::Temp(id)));
                    }
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            _ => Ok(None),
        }
    }
}
