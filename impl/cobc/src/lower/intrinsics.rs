// Fx: spawn and the intrinsics.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // `[Spawn]`: the callee as a `fn` value and the arguments go into a
    // heap block that holds them (references as held slots, resources
    // detached from this statement); the new thread runs a trampoline
    // that makes an ordinary call from the block, through the callee's
    // thunk, and leaves the result at the block's start.
    // What `lower_spawn` may probe for an argument's type: what
    // `probe_safe` allows, and literals and enum constructors of such
    // (`spawn(Option::unwrap, Some(5))`), which call no function.
    fn spawn_probe_safe(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_) | ExprKind::StrLit(_) => true,
            ExprKind::Call(callee, xs) => match &callee.kind {
                ExprKind::Path(segs, _) => self.variant_of(segs).is_some() && xs.iter().all(|x| self.spawn_probe_safe(x)),
                _ => false,
            },
            _ => probe_safe(e),
        }
    }

    pub(super) fn lower_spawn(&mut self, args: &[Expr], line: usize) -> R<V> {
        // A generic function item spawned without type arguments
        // (`spawn(run_chunk, v, f)`) is instantiated from the arguments'
        // types, probed without lowering them, as coby infers it.
        let mut callee_expected: Option<Type> = None;
        if let ExprKind::Path(segs, targs) = &args[0].kind {
            let local = segs.len() == 1 && self.lookup(&segs[0]).is_some();
            if let Some(f) = self.g.items.fns.get(&segs.join("::")).cloned().filter(|f| !local && targs.is_empty() && !f.type_params.is_empty()) {
                let mut m: HashMap<String, Type> = HashMap::new();
                for (p, a) in f.params.iter().zip(args[1..].iter()) {
                    if !self.spawn_probe_safe(a) {
                        continue;
                    }
                    let hint = apply(&p.ty, &m);
                    let exp = if resolved(&hint, &f.type_params) { Some(hint) } else { None };
                    let Ok(t) = self.probe_with(|fx| fx.lower_expr(a, exp.as_ref())) else { continue };
                    let t = match (&p.ty, &t) {
                        (Type::Fn(..), Type::Closure(id)) => match self.g.closures.get(id) {
                            Some(info) => Type::Fn(info.params.clone(), Box::new(info.ret.clone())),
                            None => t,
                        },
                        _ => t,
                    };
                    unify(&p.ty, &t, &f.type_params, &mut m);
                }
                if f.type_params.iter().all(|tp| m.contains_key(tp)) {
                    callee_expected = Some(Type::Fn(f.params.iter().map(|p| apply(&p.ty, &m)).collect(), Box::new(apply(&f.ret, &m))));
                }
            }
        }
        let fv = self.lower_expr(&args[0], callee_expected.as_ref())?;
        if matches!(fv.ty, Type::Never) {
            return Ok(never());
        }
        let fty = match &fv.ty {
            Type::Fn(..) => fv.ty.clone(),
            Type::Closure(id) => {
                let info = self.g.closures[id].clone();
                Type::Fn(info.params.clone(), Box::new(info.ret.clone()))
            }
            _ => return internal("spawn of a non-callable"),
        };
        // D-0289: whether the thread is lent an exclusive reference, as
        // `cb_thread_start` finds from the paths it holds; a pathless
        // program has none, so the trampoline says so itself (spec/18
        // `contained`). A thread that runs a `fn` value is never pathless
        // (the static pass leaves it to the run).
        let excl = |t: &Type| matches!(t, Type::Ref(_, Mode::Exclusive) | Type::Slice(_, Mode::Exclusive));
        let mut lent_excl = match &fv.ty {
            Type::Closure(id) => self.g.closures[id].fields.iter().any(|(_, t)| excl(t)),
            _ => false,
        };
        let fv = if matches!(fv.ty, Type::Closure(_)) { self.box_closure(fv, fty.clone(), line)? } else { fv };
        let Type::Fn(ps, ret) = fty.clone() else { unreachable!() };
        lent_excl |= ps.iter().any(|p| excl(p));
        let ret = *ret;
        let h = self.g.fresh("h");
        self.emit(&format!("void *{} = {};", h, fv.c));
        let mut argv = Vec::new();
        for (pty, a) in ps.iter().zip(args[1..].iter()) {
            let v = self.lower_expr(a, Some(pty))?;
            if matches!(v.ty, Type::Never) {
                return Ok(never());
            }
            let t = self.into_temp(v, a.line)?;
            argv.push(self.weaken_arg(t, pty, a.line)?);
        }

        // The block's layout: the result first, then the callee, then
        // each argument in memory form (and a resource's object id).
        // Unique program-wide (`closure_id`: the function's C name hashed):
        // a function lowered twice (its `__raw` form) or two spawning
        // functions never share a block or trampoline name.
        let n = self.g.closure_id();
        let env = format!("cb_env_{}", n);
        let rc = self.g.ctype(&ret)?;
        let mut fields = format!("{} res; void *h;", rc);
        for (i, pty) in ps.iter().enumerate() {
            if matches!(pty, Type::Ref(..)) {
                write!(fields, " void *a{};", i).unwrap();
            } else {
                let c = self.g.ctype(pty)?;
                write!(fields, " {} a{};", c, i).unwrap();
                if self.is_res(pty) {
                    write!(fields, " uint64_t o{};", i).unwrap();
                }
            }
        }
        let e = self.g.fresh("env");
        self.emit(&format!("struct {} *{} = cb_env_alloc(sizeof(struct {}), _Alignof(struct {}));", env, e, env, env));
        self.emit(&format!("{}->h = {};", e, h));
        let mut body = String::new();
        let mut call_args = Vec::new();
        for (i, (pty, t)) in ps.iter().zip(argv.iter()).enumerate() {
            if let Type::Ref(inner, m) = pty {
                let itid = self.tid(inner)?;
                let md = if *m == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                self.emit(&format!("{}->a{} = {}.p;", e, i, t.c));
                self.emit(&format!("cb_hold_ref(&{}->a{}, {}.tok, {}.p, {}u, {}, CB_NOLOC, 0);", e, i, t.c, t.c, itid, md));
                writeln!(body, "    cb_ref r{} = {{ e->a{}, cb_load_ref(&e->a{}) }};", i, i, i).unwrap();
                call_args.push(format!("r{}", i));
                continue;
            }
            self.emit(&format!("{}->a{} = {};", e, i, t.c));
            if self.is_res(pty) {
                let o = t.obj.as_ref().expect("resource argument has object");
                self.emit(&format!("cb_move_to({}, &{}->a{});", o, e, i));
                self.emit(&format!("cb_hold({});", o));
                self.emit(&format!("{}->o{} = {};", e, i, o));
                // D-0286: in a decided program the thread takes the value
                // alone, as a callee does (`give_arg`).
                let give = if self.g.decided { "cb_forget_obj" } else { "cb_send" };
                writeln!(body, "    {}(e->o{});", give, i).unwrap();
            } else if self.has_refs(pty) {
                let tid = self.tid(pty)?;
                self.emit(&format!("cb_copy_datum(&{}->a{}, &({}), {}u);", e, i, t.c, tid));
                writeln!(body, "    cb_send_datum(&e->a{}, {}u);", i, tid).unwrap();
            }
            call_args.push(format!("e->a{}", i));
        }
        let thunk = self.g.thunk(&fty)?;
        let call = format!("{}(e->h{})", thunk, call_args.iter().map(|a| format!(", {}", a)).collect::<String>());
        let rtid = self.tid(&ret)?;
        let done = if matches!(ret, Type::Void | Type::Never) {
            writeln!(body, "    {};", call).unwrap();
            writeln!(body, "    cb_env_forget(e, sizeof(*e));").unwrap();
            "0".to_string()
        } else if matches!(ret, Type::Ref(..)) {
            // D-0094: a bare reference result travels in its stored form —
            // the pointer in `res`, the token as a slot record at `&res`
            // (as a reference argument travels the other way). The call's
            // own return is received here as any ref-returning call's is;
            // the slot's hold then keeps the token alive past this
            // thread's end, and `cb_join`'s rekeying carries it to the
            // joiner's storage.
            // The result slot is stored before the arguments' slots are
            // forgotten (`res` is the block's first field, so the forget
            // starts after it): when the result token IS an argument's
            // token (a function returning its own reference parameter),
            // a gap in holding would retire it — this thread has no
            // scope to keep an unheld path alive.
            writeln!(body, "    cb_ref rr = {};", call).unwrap();
            writeln!(body, "    cb_recv_ref();").unwrap();
            writeln!(body, "    e->res = rr.p;").unwrap();
            let (itid, md) = match &ret {
                Type::Ref(inner, m) => (self.tid(inner)?, if *m == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" }),
                _ => unreachable!(),
            };
            writeln!(body, "    cb_hold_ref(&e->res, rr.tok, rr.p, {}u, {}, CB_NOLOC, 0);", itid, md).unwrap();
            writeln!(body, "    cb_env_forget((uint8_t *)e + sizeof(void *), sizeof(*e) - sizeof(void *));").unwrap();
            "0".to_string()
        } else {
            writeln!(body, "    e->res = {};", call).unwrap();
            writeln!(body, "    cb_env_forget(e, sizeof(*e));").unwrap();
            if self.is_res(&ret) && self.g.decided {
                // D-0286: the result returns as its bytes (`take_result`).
                writeln!(body, "    uint64_t o = cb_new(&e->res, {}u, {});", rtid, self.loc(line)).unwrap();
                "o".to_string()
            } else if self.is_res(&ret) {
                writeln!(body, "    uint64_t o = cb_recv(&e->res);").unwrap();
                "o".to_string()
            } else {
                if self.has_refs(&ret) {
                    writeln!(body, "    cb_recv_datum(&e->res, {}u);", rtid).unwrap();
                }
                "0".to_string()
            }
        };
        writeln!(body, "    cb_drop_fn(e->h);").unwrap();
        writeln!(body, "    cb_thread_done({});", done).unwrap();
        let tramp = format!("cb_thr_{}", n);
        // A probe (`probe_type`) discards what it lowered; the block and
        // trampoline are written only by the real lowering.
        if self.g.probing == 0 {
            writeln!(self.g.thunks_out, "struct {} {{ {} }};", env, fields).unwrap();
            // D-0204: a contained fault's unwind ends at the `setjmp`
            // (`cb_thread_jmp`): the block's references forgotten, the
            // callee dropped, the failure the thread's result.
            // The call itself is in a function of its own: GCC does not
            // realign the frame of a function that calls `setjmp`, so an
            // over-aligned argument (`struct S align(64)`) copied there for
            // the call was only 16-aligned (found on Windows, D-0255).
            writeln!(
                self.g.thunks_out,
                "static __attribute__((noinline)) void {t}_run(struct {e} *e)\n{{\n{b}}}\nvoid {t}(void *p)\n{{\n    struct {e} *e = p;\n    jmp_buf jb;\n    if (setjmp(jb) == 0)\n    {{\n    cb_thread_jmp((void *)jb);\n    cb_thread_start((void *)e, sizeof(*e), (void *)e->h);\n{l}    {t}_run(e);\n    }}\n    else\n    {{\n    cb_env_forget(e, sizeof(*e));\n    cb_drop_fn(e->h);\n    cb_thread_failed();\n    }}\n}}",
                t = tramp,
                e = env,
                b = body,
                l = if lent_excl { "#ifdef CB_PATHLESS\n    cb_thread_lent();\n#endif\n" } else { "" }
            )
            .unwrap();
        }

        let hty = Type::Handle(Box::new(ret));
        let htid = self.tid(&hty)?;
        let loc = self.loc(line);
        // D-0213 `[Thread-Lend]`: the exclusive references the block and
        // the callee's captures carry are lent to the new thread, before
        // it can run.
        if ps.iter().any(|p| self.has_refs(p)) {
            self.emit(&format!("cb_lend((void *){}, sizeof(struct {}), {});", e, env, loc));
        }
        self.emit(&format!("cb_lend_fn({}, {});", h, loc));
        let hv = self.g.fresh("t");
        self.emit(&format!("uint64_t {} = cb_spawn({}, {}, sizeof(struct {}), {}u);", hv, tramp, e, env, rtid));
        let o = self.g.fresh("o");
        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, hv, htid, loc));
        Ok(V { c: hv, ty: hty, obj: Some(o) })
    }

    // A hash-table key's bytes, as an address and a length (`key_hash`,
    // `key_eq`): an integer's or `bool`'s own storage (little-endian, its
    // width), a `str`'s or `String`'s UTF-8 bytes. `a` is a `ref<K, shared>`.
    // D-0110: a struct or enum key, as a pointer to it read through its
    // reference (checked once).
    pub(super) fn lower_key_ptr(&mut self, a: &Expr, kt: &Type, line: usize) -> R<String> {
        let r = self.lower_expr(a, None)?;
        let r = self.into_temp(r, line)?.c;
        let loc = self.loc(line);
        self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", r, loc));
        let ct = self.g.ctype(kt)?;
        Ok(format!("((const {} *)({}).p)", ct, r))
    }

    pub(super) fn lower_key_bytes(&mut self, a: &Expr, line: usize) -> R<(String, String)> {
        let t = self.probe_type(a, None)?;
        let Type::Ref(inner, _) = &t else {
            return internal("a key that is not a reference");
        };
        if matches!(&**inner, Type::Named(n, _) if n == "std::String") {
            let r = self.lower_expr(a, None)?;
            let r = self.into_temp(r, line)?.c;
            let loc = self.loc(line);
            self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", r, loc));
            let sc = self.g.ctype(&Type::Named("std::String".into(), vec![]))?;
            return Ok((format!("(const void *)(({} *)({}).p)->bytes.ptr", sc, r), format!("(({} *)({}).p)->bytes.len", sc, r)));
        }
        let d = std::rc::Rc::new(Expr { kind: ExprKind::Deref(Box::new(a.clone())), line: a.line, col: a.col });
        self.kept.push(d.clone());
        let v = self.lower_expr(&d, None)?;
        let ct = self.g.ctype(&v.ty)?;
        let k = self.g.fresh("k");
        self.emit(&format!("{} {} = {};", ct, k, v.c));
        match &v.ty {
            Type::Str => Ok((format!("(const void *)({}).p", k), format!("({}).n", k))),
            Type::Int(_) | Type::Bool => Ok((format!("(const void *)&{}", k), format!("sizeof({})", k))),
            _ => internal("a key of a type that is not hashable"),
        }
    }

    pub(super) fn lower_intrinsic(&mut self, name: &str, targs: &[Type], args: &[Expr], expected: Option<&Type>, line: usize) -> R<V> {
        let loc = self.loc(line);
        match name {
            "sizeof" | "alignof"
            | "static_assert"
            | "min_value" | "max_value"
            | "std::sqrt" | "std::floor" | "std::ceil" | "std::round" | "std::trunc" | "std::ln" | "std::exp" | "std::log2" | "std::log10" | "std::sin" | "std::cos" | "std::tan" | "std::asin" | "std::acos" | "std::atan"
            | "std::atan2" | "std::powf"
            | "widen" | "narrow" | "narrow_wrapping" | "reinterpret" | "to_float" | "to_int"
            | "checked_narrow"
            | "count_ones" | "leading_zeros" | "trailing_zeros"
            | "rotate_left" | "rotate_right"
            | "read_volatile" | "write_volatile"
            | "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add" | "saturating_sub" | "saturating_mul" | "checked_add" | "checked_sub" | "checked_mul" | "checked_div" | "checked_rem"
                => self.lower_intrinsic_numeric(name, targs, args, expected, line, loc),
            "slice_len"
            | "str_len"
            | "str_byte"
            | "str_ptr"
            | "str_slice"
            | "rawptr_of"
            | "rawptr_from_address"
            | "rawptr_address"
            | "allocate"
            | "deallocate"
            | "release"
            | "copy_raw"
            | "reinterpret_ptr"
            | "dangling"
            | "null_rawptr"
            | "fault"
                => self.lower_intrinsic_raw(name, targs, args, line, loc),
            "drop"
            | "reclaim"
            | "spawn"
            | "join_status" | "cancel"
            | "join"
            | "lock"
            | "Mutex::new"
            | "std::exit"
            | "std::print"
            | "append_native"
                => self.lower_intrinsic_runtime(name, targs, args, expected, line, loc),
            "atomic_op" => self.lower_atomic_op(args, line, loc),
            _ if name.starts_with("simd_") => self.lower_simd_intrinsic(name, targs, args, expected, line),
            "atomic_wait_op" => self.lower_atomic_wait_op(args, line, loc),
            "cpu_op" => {
                // D-0253: 0 a sequentially consistent fence, 1 a spin hint
                // (the CPU's pause, and a schedule's point under `--explore`).
                let ExprKind::IntLit(op, _) = &strip_parens(&args[0]).kind else {
                    return internal("a cpu operation that is not a literal");
                };
                if *op == 0 {
                    self.emit("__atomic_thread_fence(__ATOMIC_SEQ_CST);");
                } else {
                    self.emit("cb_spin_hint();");
                    self.emit("if (__builtin_expect(__atomic_load_n(&cb_sched_flag, __ATOMIC_RELAXED) != 0, 0)) cb_atomic_point();");
                }
                Ok(unit())
            }
            "text_write"
            | "parse_check" | "parse_value"
            | "$assert_fail"
            | "$fmt"
            | "std::Result::map_err"
                => self.lower_intrinsic_text(name, targs, args, line, loc),
            _ => self.lower_intrinsic_places(name, args, expected, line, loc),
        }
    }

    // D-0249 `rule.conc.atomic`, std-only: `atomic_op(a, op, x, y)`, the
    // value the operation found. `a` is checked as a shared access of the
    // atomic (`cb_read`); the operation itself is one sequentially
    // consistent builtin on the atomic's one field, at its address. A
    // checked add or subtract is a compare-and-exchange loop that faults
    // before storing anything.
    fn lower_atomic_op(&mut self, args: &[Expr], line: usize, loc: String) -> R<V> {
        let ExprKind::IntLit(op, _) = &strip_parens(&args[1]).kind else {
            return internal("an atomic operation that is not a literal");
        };
        let op = *op;
        let av = self.lower_expr(&args[0], None)?;
        if matches!(av.ty, Type::Never) {
            return Ok(never());
        }
        // D-0253: an `AtomicPtr<T>`'s cell is a `rawptr<T>` (load, store,
        // swap and compare-and-exchange only).
        let ty = match &av.ty {
            Type::Ref(a, _) => match &**a {
                Type::Named(n, ta) if n == "std::Atomic" => match ta.first().map(|t| self.sub(t)) {
                    Some(Type::Int(t)) => Type::Int(t),
                    _ => return internal("an atomic of a non-integer"),
                },
                Type::Named(n, ta) if n == "std::AtomicPtr" && op <= 3 => match ta.first().map(|t| self.sub(t)) {
                    Some(t) => Type::Rawptr(Box::new(t)),
                    None => return internal("an atomic pointer without its type"),
                },
                _ => return internal("an atomic operation on a non-atomic"),
            },
            _ => return internal("an atomic operation through a non-reference"),
        };
        let at = self.into_temp(av, line)?;
        let operand = |me: &mut Self, e: &Expr| -> R<Option<String>> {
            let v = me.lower_expr(e, Some(&ty))?;
            if matches!(v.ty, Type::Never) {
                return Ok(None);
            }
            Ok(Some(me.into_temp(v, line)?.c))
        };
        let Some(x) = operand(self, &args[2])? else { return Ok(never()) };
        let Some(y) = operand(self, &args[3])? else { return Ok(never()) };
        let ct = match &ty {
            Type::Int(t) => c_int_type(*t).to_string(),
            _ => self.g.ctype(&ty)?,
        };
        self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", at.c, loc));
        self.emit("if (__builtin_expect(__atomic_load_n(&cb_sched_flag, __ATOMIC_RELAXED) != 0, 0)) cb_atomic_point();");
        let p = format!("(({} *)({}).p)", ct, at.c);
        let r = self.g.fresh("t");
        const SC: &str = "__ATOMIC_SEQ_CST";
        match op {
            0 => self.emit(&format!("{} {} = __atomic_load_n({}, {});", ct, r, p, SC)),
            1 => {
                self.emit(&format!("__atomic_store_n({}, ({}){}, {});", p, ct, x, SC));
                self.emit(&format!("{} {} = 0;", ct, r));
            }
            2 => self.emit(&format!("{} {} = __atomic_exchange_n({}, ({}){}, {});", ct, r, p, ct, x, SC)),
            3 => {
                // On failure the builtin writes the value it found into `r`.
                self.emit(&format!("{} {} = {};", ct, r, x));
                self.emit(&format!("(void)__atomic_compare_exchange_n({}, &{}, ({}){}, 0, {}, {});", p, r, ct, y, SC, SC));
            }
            4 | 5 => {
                let f = if op == 4 { "__builtin_add_overflow" } else { "__builtin_sub_overflow" };
                let n = self.g.fresh("t");
                self.emit(&format!("{} {} = __atomic_load_n({}, {});", ct, r, p, SC));
                self.emit(&format!("{} {};", ct, n));
                self.emit(&format!(
                    "do {{ if ({}({}, ({}){}, &{})) cb_fault(\"diag.arith-overflow\", {}); }} while (!__atomic_compare_exchange_n({}, &{}, {}, 1, {}, {}));",
                    f, r, ct, x, n, loc, p, r, n, SC, SC
                ));
            }
            6..=10 => {
                let f = ["__atomic_fetch_add", "__atomic_fetch_sub", "__atomic_fetch_and", "__atomic_fetch_or", "__atomic_fetch_xor"][(op - 6) as usize];
                self.emit(&format!("{} {} = {}({}, ({}){}, {});", ct, r, f, p, ct, x, SC));
            }
            _ => return internal("an unknown atomic operation"),
        }
        Ok(V { c: r, ty, obj: None })
    }

    // D-0253, std-only: `atomic_wait_op(a, op, seen)`, the wake count of the
    // atomic at `a`'s address, kept by the runtime (`cb_atomic_wait`): 0
    // reads it, 1 sleeps until it is not `seen`, 2 and 3 add one and wake one
    // or every waiter. `a` is checked as a shared access first.
    fn lower_atomic_wait_op(&mut self, args: &[Expr], line: usize, loc: String) -> R<V> {
        let av = self.lower_expr(&args[0], None)?;
        if matches!(av.ty, Type::Never) {
            return Ok(never());
        }
        let at = self.into_temp(av, line)?;
        let u8t = Type::Int(IntTy::U8);
        let op = self.lower_expr(&args[1], Some(&u8t))?;
        let op = self.into_temp(op, line)?;
        let u64t = Type::Int(IntTy::U64);
        let seen = self.lower_expr(&args[2], Some(&u64t))?;
        if matches!(seen.ty, Type::Never) {
            return Ok(never());
        }
        let seen = self.into_temp(seen, line)?;
        self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", at.c, loc));
        let r = self.g.fresh("t");
        self.emit(&format!("uint64_t {} = cb_atomic_wait((uint64_t){}, (const void *)({}).p, {});", r, op.c, at.c, seen.c));
        Ok(V { c: r, ty: u64t, obj: None })
    }

    // Intrinsics over numbers: sizes, conversions, bit operations, checked and wrapping arithmetic.
    fn lower_intrinsic_numeric(&mut self, name: &str, targs: &[Type], args: &[Expr], expected: Option<&Type>, line: usize, loc: String) -> R<V> {
        let targ = |i: usize, fx: &Self| targs.get(i).map(|t| fx.sub(t));
        match name {
            "sizeof" | "alignof" => {
                let t = targ(0, self).expect("type argument");
                let (s, a) = self.g.size_align(&t)?;
                Ok(V { c: format!("((uint64_t){}u)", if name == "sizeof" { s } else { a }), ty: Type::Int(IntTy::Usize), obj: None })
            }
            // D-0052: computed before the program was compiled.
            "static_assert" => Ok(unit()),
            "min_value" | "max_value" => {
                // D-0051: a float's largest finite value (as a hex float
                // literal, exact), negated for `min_value`.
                match targ(0, self) {
                    Some(Type::F32) => {
                        let c = if name == "max_value" { "(0x1.fffffep+127f)" } else { "(-0x1.fffffep+127f)" };
                        return Ok(V { c: c.to_string(), ty: Type::F32, obj: None });
                    }
                    Some(Type::F64) => {
                        let c = if name == "max_value" { "(0x1.fffffffffffffp+1023)" } else { "(-0x1.fffffffffffffp+1023)" };
                        return Ok(V { c: c.to_string(), ty: Type::F64, obj: None });
                    }
                    _ => {}
                }
                let Some(Type::Int(t)) = targ(0, self) else { return internal("a limit of a non-integer type") };
                let (min, max) = bounds(t);
                Ok(V { c: c_int(t, if name == "min_value" { min } else { max }), ty: Type::Int(t), obj: None })
            }
            // D-0091: C's functions of the same names (`<math.h>`, Annex F).
            // D-0136: `std::math`'s, realized natively.
            "std::sqrt" | "std::floor" | "std::ceil" | "std::round" | "std::trunc" | "std::ln" | "std::exp" | "std::log2" | "std::log10" | "std::sin"
            | "std::cos" | "std::tan" | "std::asin" | "std::acos" | "std::atan" => {
                let name = &name["std::".len()..];
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let x = self.into_temp(v.clone(), line)?.c;
                // D-0101: `ln` is C's `log`.
                // `round` is the runtime's (`cb_round`, D-0255).
                let base = match name {
                    "ln" => "log",
                    "round" => "cb_round",
                    _ => name,
                };
                let f = if v.ty == Type::F32 { format!("{}f", base) } else { base.to_string() };
                return Ok(V { c: format!("{}({})", f, x), ty: v.ty, obj: None });
            }
            // D-0101: C's `atan2` and `pow` (`powf` is CobaltC's name for the
            // float power; `pow` takes a `u32` exponent).
            "std::atan2" | "std::powf" => {
                let name = &name["std::".len()..];
                let a = self.lower_expr(&args[0], None)?;
                let a = self.into_temp(a, line)?;
                let b = self.lower_expr(&args[1], None)?;
                if matches!(a.ty, Type::Never) || matches!(b.ty, Type::Never) {
                    return Ok(never());
                }
                let b = self.into_temp(b, line)?;
                let base = if name == "atan2" { "atan2" } else { "pow" };
                let f = if a.ty == Type::F32 { format!("{}f", base) } else { base.to_string() };
                return Ok(V { c: format!("{}({}, {})", f, a.c, b.c), ty: a.ty, obj: None });
            }
            "widen" | "narrow" | "narrow_wrapping" | "reinterpret" | "to_float" | "to_int" => {
                let dst = targ(0, self).expect("type argument");
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let x = self.into_temp(v.clone(), line)?.c;
                let c = match (name, &v.ty, &dst) {
                    ("widen", Type::Int(_), Type::Int(d)) => format!("(({}){})", c_int_type(*d), x),
                    ("narrow", Type::Int(s), Type::Int(d)) => narrow_check(*s, *d, &x, &loc),
                    ("narrow_wrapping", Type::Int(_), Type::Int(d)) => format!("(({}){})", c_int_type(*d), x),
                    ("reinterpret", Type::Int(s), Type::Int(d)) => {
                        let (sb, db) = (s.bitwidth(64), d.bitwidth(64));
                        if db > sb {
                            let mid = if d.signed() { c_sint_of_width(sb) } else { c_uint_of_width(sb) };
                            format!("(({})({})({}){})", c_int_type(*d), mid, c_uint_of_width(sb), x)
                        } else {
                            format!("(({}){})", c_int_type(*d), x)
                        }
                    }
                    // D-0051: `f32` into `f64` exactly; `f64` into `f32`
                    // rounds as IEEE-754 does (C's conversion under Annex F,
                    // which gcc and clang implement on these targets).
                    ("widen" | "to_float", Type::F32 | Type::F64, Type::F64) => format!("((double){})", x),
                    ("widen" | "to_float", Type::F32 | Type::F64, Type::F32) => format!("((float){})", x),
                    // D-0051: a float's bits as an integer of its width, and back.
                    ("reinterpret", Type::F32, Type::Int(d)) => format!("(({})((union {{ float f; uint32_t u; }}){{ .f = {} }}).u)", c_int_type(*d), x),
                    ("reinterpret", Type::F64, Type::Int(d)) => format!("(({})((union {{ double f; uint64_t u; }}){{ .f = {} }}).u)", c_int_type(*d), x),
                    ("reinterpret", Type::Int(_), Type::F32) => format!("(((union {{ uint32_t u; float f; }}){{ .u = (uint32_t){} }}).f)", x),
                    ("reinterpret", Type::Int(_), Type::F64) => format!("(((union {{ uint64_t u; double f; }}){{ .u = (uint64_t){} }}).f)", x),
                    ("to_float", Type::Int(_), Type::F64) => format!("((double){})", x),
                    ("to_float", Type::Int(_), Type::F32) => format!("((float){})", x),
                    ("to_int", Type::F32 | Type::F64, Type::Int(d)) => to_int_check(*d, &x, &loc),
                    _ => return internal("conversion on an unexpected type"),
                };
                Ok(V { c, ty: dst, obj: None })
            }
            // D-0122: `narrow` answering `None`.
            "checked_narrow" => {
                let dst = targ(0, self).expect("type argument");
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let x = self.into_temp(v.clone(), line)?.c;
                let (Type::Int(s), Type::Int(d)) = (&v.ty, &dst) else { return internal("checked_narrow on a non-integer") };
                let cond = narrow_cond(*s, *d, &x);
                let opt = Type::Named("std::Option".to_string(), vec![dst.clone()]);
                let oc = self.g.ctype(&opt)?;
                let t2 = self.g.fresh("cn");
                self.emit(&format!("{} {};", oc, t2));
                self.emit(&format!("if ({}) {{ {}.tag = 0u; {}.u.v0 = ({}){}; }} else {}.tag = 1u;", cond, t2, t2, c_int_type(*d), x, t2));
                Ok(V { c: t2, ty: opt, obj: None })
            }
            // D-0123: bit counts on the unsigned image; rotations by `k mod width`.
            "count_ones" | "leading_zeros" | "trailing_zeros" => {
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let Type::Int(t) = v.ty else { return internal("bit count on a non-integer") };
                let x = self.into_temp(v.clone(), line)?.c;
                let bw = t.bitwidth(64);
                let c = if bw == 128 {
                    let f = match name { "count_ones" => "cb_popcount128", "leading_zeros" => "cb_clz128", _ => "cb_ctz128" };
                    format!("{}((cb_u128){})", f, x)
                } else {
                    let ux = format!("(uint64_t)({}){}", c_uint_of_width(bw), x);
                    match name {
                        "count_ones" => format!("cb_popcount64({})", ux),
                        "leading_zeros" => format!("cb_clz({}, {}u)", ux, bw),
                        _ => format!("cb_ctz({}, {}u)", ux, bw),
                    }
                };
                Ok(V { c, ty: Type::Int(IntTy::U32), obj: None })
            }
            "rotate_left" | "rotate_right" => {
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let Type::Int(t) = v.ty else { return internal("rotation of a non-integer") };
                let x = self.into_temp(v.clone(), line)?.c;
                let k = self.lower_expr(&args[1], Some(&Type::Int(IntTy::U32)))?;
                let k = self.into_temp(k, line)?.c;
                let bw = t.bitwidth(64);
                let c = if bw == 128 {
                    let f = if name == "rotate_left" { "cb_rotl128" } else { "cb_rotr128" };
                    format!("(({})({}((cb_u128){}, {})))", c_int_type(t), f, x, k)
                } else {
                    // At the operand's own width, in the form C compilers turn
                    // into one rotate instruction (`cb_rotr32`, …).
                    let f = if name == "rotate_left" { "cb_rotl" } else { "cb_rotr" };
                    format!("(({})({}{}(({}){}, {})))", c_int_type(t), f, bw, c_uint_of_width(bw), x, k)
                };
                Ok(V { c, ty: Type::Int(t), obj: None })
            }
            // D-0121: through a `volatile` lvalue, so the C compiler keeps
            // every access as written.
            "read_volatile" | "write_volatile" => {
                let p = self.lower_expr(&args[0], None)?;
                if matches!(p.ty, Type::Never) {
                    return Ok(never());
                }
                let Type::Rawptr(pointee) = p.ty.clone() else { return internal("volatile access through a non-pointer") };
                let pc = self.into_temp(p, line)?.c;
                let ct = self.g.ctype(&pointee)?;
                if name == "read_volatile" {
                    return Ok(V { c: format!("(*(volatile {} *)({}))", ct, pc), ty: (*pointee).clone(), obj: None });
                }
                let v = self.lower_expr(&args[1], Some(&pointee))?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let vc = self.into_temp(v, line)?.c;
                self.emit(&format!("*(volatile {} *)({}) = {};", ct, pc, vc));
                Ok(unit())
            }
            "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add" | "saturating_sub" | "saturating_mul" | "checked_add"
            | "checked_sub" | "checked_mul" | "checked_div" | "checked_rem" => {
                let (av, bv) = self.lower_operands(&args[0], &args[1], expected)?;
                let (Some(av), Some(bv)) = (av, bv) else { return Ok(never()) };
                // D-0254: on a `simd`, wrapping in the unsigned twin type,
                // saturating lane by lane as the scalar does.
                if let Type::Simd(lane, n) = av.ty.clone() {
                    let Type::Int(t) = *lane else { return internal("a wrapping simd of floats") };
                    let ty = av.ty.clone();
                    let ct = self.g.ctype(&ty)?;
                    let m = mangle(&ty);
                    let (family, opname) = name.split_once('_').unwrap();
                    let sym = match opname { "add" => "+", "sub" => "-", _ => "*" };
                    if family == "wrapping" {
                        return Ok(V { c: format!("(({ct})((cbvu_{m}){a} {sym} (cbvu_{m}){b}))", ct = ct, m = m, a = av.c, sym = sym, b = bv.c), ty, obj: None });
                    }
                    // Saturating `+` and `-` on whole vectors, so the C
                    // compiler emits its saturating instructions (a lane
                    // loop of `__builtin_*_overflow` was slower than
                    // scalar code under Clang): the wrapped sum, its
                    // overflow mask as the checked operators test it, and
                    // in an overflowing lane the bound on `a`'s side (an
                    // overflow needs `a` and `b` alike for `+`, unlike for
                    // `-`, so `a`'s sign names the bound: `(a >> (w - 1)) ^
                    // MAX` is MIN for a negative `a`, MAX otherwise).
                    if opname == "add" || opname == "sub" {
                        let (_, max) = bounds(t);
                        let c = match (t.signed(), opname) {
                            (false, "add") => format!("({{ {ct} _a = {a}, _b = {b}; {ct} _r = _a + _b; _r | ({ct})(_r < _a); }})", ct = ct, a = av.c, b = bv.c),
                            (false, _) => format!("({{ {ct} _a = {a}, _b = {b}; {ct} _r = _a - _b; _r & ~({ct})(_a < _b); }})", ct = ct, a = av.c, b = bv.c),
                            (true, _) => {
                                let ovf = if opname == "add" { "((_a ^ _r) & (_b ^ _r)) < 0" } else { "((_a ^ _b) & (_a ^ _r)) < 0" };
                                format!(
                                    "({{ {ct} _a = {a}, _b = {b}; {ct} _r = ({ct})((cbvu_{m})_a {sym} (cbvu_{m})_b); {ct} _o = ({ct})({ovf}); {ct} _s = (_a >> {sh}) ^ {max}; (_r & ~_o) | (_s & _o); }})",
                                    ct = ct, a = av.c, b = bv.c, m = m, sym = sym, ovf = ovf, sh = t.bitwidth(64) - 1, max = c_int(t, max)
                                )
                            }
                        };
                        return Ok(V { c, ty, obj: None });
                    }
                    let builtin = if opname == "add" { "__builtin_add_overflow" } else { "__builtin_sub_overflow" };
                    let (min, max) = bounds(t);
                    let tm = match (t.signed(), opname) {
                        (true, "add") => "_b[_i] > 0",
                        (true, _) => "_b[_i] < 0",
                        (false, "add") => "1",
                        _ => "0",
                    };
                    let c = format!(
                        "({{ {ct} _a = {a}, _b = {b}, _r; for (int _i = 0; _i < {n}; _i++) {{ {lc} _l; if ({builtin}(_a[_i], _b[_i], &_l)) _l = {tm} ? {max} : {min}; _r[_i] = _l; }} _r; }})",
                        ct = ct, a = av.c, b = bv.c, n = n, lc = c_int_type(t), builtin = builtin, tm = tm, max = c_int(t, max), min = c_int(t, min)
                    );
                    return Ok(V { c, ty, obj: None });
                }
                let Type::Int(t) = av.ty else { return internal("integer intrinsic on a non-integer") };
                let a = av.c;
                let b = bv.c;
                let ct = c_int_type(t);
                let ut = c_uint_type(t);
                let (min, max) = bounds(t);
                let (family, opname) = name.split_once('_').unwrap();
                let builtin = match opname {
                    "add" => "__builtin_add_overflow",
                    "sub" => "__builtin_sub_overflow",
                    "mul" => "__builtin_mul_overflow",
                    _ => "",
                };
                match family {
                    "wrapping" => {
                        let sym = match opname {
                            "add" => "+",
                            "sub" => "-",
                            _ => "*",
                        };
                        Ok(V { c: format!("(({})((({}){}) {} (({}){})))", ct, ut, a, sym, ut, b), ty: Type::Int(t), obj: None })
                    }
                    "saturating" => {
                        let toward_max = match opname {
                            "add" => format!("({} > 0)", b),
                            "sub" => format!("({} < 0)", b),
                            _ => format!("(({} < 0) == ({} < 0))", a, b),
                        };
                        let toward_max = if t.signed() { toward_max } else if opname == "sub" { "0".to_string() } else { "1".to_string() };
                        Ok(V {
                            c: format!(
                                "({{ {ct} _r; if ({builtin}({a}, {b}, &_r)) _r = {tm} ? {max} : {min}; _r; }})",
                                ct = ct,
                                builtin = builtin,
                                a = a,
                                b = b,
                                tm = toward_max,
                                max = c_int(t, max),
                                min = c_int(t, min)
                            ),
                            ty: Type::Int(t),
                            obj: None,
                        })
                    }
                    _ => {
                        let opt = Type::Named("std::Option".to_string(), vec![Type::Int(t)]);
                        let oc = self.g.ctype(&opt)?;
                        let fail = match opname {
                            "div" | "rem" => {
                                let sym = if opname == "div" { "/" } else { "%" };
                                let ovf = if t.signed() { format!(" || ({} == {} && {} == ({})-1)", a, c_int(t, min), b, ct) } else { String::new() };
                                format!("if ({b} == 0{ovf}) _f = 1; else _r = ({ct})({a} {sym} {b});")
                            }
                            _ => format!("_f = {}({}, {}, &_r);", builtin, a, b),
                        };
                        let t2 = self.g.fresh("ck");
                        self.emit(&format!("{} {};", oc, t2));
                        self.emit(&format!("{{ {} _r = 0; int _f = 0; {} if (_f) {}.tag = 1u; else {{ {}.tag = 0u; {}.u.v0 = _r; }} }}", ct, fail, t2, t2, t2));
                        Ok(V { c: t2, ty: opt, obj: None })
                    }
                }
            }
            _ => internal(&format!("unknown callee `{}`", name)),
        }
    }

    // Intrinsics over text bytes and raw memory, and `fault`.
    fn lower_intrinsic_raw(&mut self, name: &str, targs: &[Type], args: &[Expr], line: usize, loc: String) -> R<V> {
        let targ = |i: usize, fx: &Self| targs.get(i).map(|t| fx.sub(t));
        match name {
            "slice_len" => {
                let p = self.lower_place(&args[0])?;
                let p = self.auto_deref(p)?;
                Ok(V { c: format!("({}).len", p.c), ty: Type::Int(IntTy::Usize), obj: None })
            }
            "str_len" => {
                let s = self.lower_expr(&args[0], None)?;
                let st = self.into_temp(s, line)?;
                Ok(V { c: format!("({}).n", st.c), ty: Type::Int(IntTy::Usize), obj: None })
            }
            "str_byte" => {
                let s = self.lower_expr(&args[0], None)?;
                let st = self.into_temp(s, line)?;
                let i = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let it = self.into_temp(i, line)?;
                self.emit(&format!("cb_index_check((uint64_t){}, {}.n, {});", it.c, st.c, loc));
                Ok(V { c: format!("{}.p[{}]", st.c, it.c), ty: Type::Int(IntTy::U8), obj: None })
            }
            "str_ptr" => {
                let s = self.lower_expr(&args[0], None)?;
                let st = self.into_temp(s, line)?;
                Ok(V { c: format!("((uint8_t *)({}).p)", st.c), ty: Type::Rawptr(Box::new(Type::Int(IntTy::U8))), obj: None })
            }
            // D-0134 (`StringView::of`): the text's bytes as a shared slice.
            // A `str`'s bytes are static data that never ends and no path
            // can write: the slice has no path (below).
            "str_slice" => {
                let s = self.lower_expr(&args[0], None)?;
                let st = self.into_temp(s, line)?;
                let u8t = Type::Int(IntTy::U8);
                let tid = self.tid(&u8t)?;
                // `type.str` (spec/21): a `str`'s bytes are no object's
                // cells and no access path reaches them, so its slice has
                // no path (0), as an objectless `String`'s view has none
                // (D-0196). Reclaiming them made every thread that sliced a
                // literal meet in the runtime on the literal's address.
                let _ = tid;
                let tok = "0";
                let sty = Type::Slice(Box::new(u8t.clone()), Mode::Shared);
                let c = self.g.ctype(&sty)?;
                let t = self.g.fresh("sl");
                self.emit(&format!("{} {};", c, t));
                let rv = V { c: format!("((cb_ref){{ (void *)({}).p, {} }})", st.c, tok), ty: Type::Ref(Box::new(u8t), Mode::Shared), obj: None };
                self.store(&format!("{}.src", t), &rv.ty.clone(), &rv)?;
                self.emit(&format!("{}.data = (uint8_t *)({}).p;", t, st.c));
                self.emit(&format!("{}.len = ({}).n;", t, st.c));
                self.register_temp(t, sty, line)
            }
            "rawptr_of" => {
                // `[Rawptr-Of]`: the address of the place — a borrowed
                // place directly, or a reference value's target.
                if let ExprKind::Borrow(_, inner) = &args[0].kind {
                    let p = self.lower_place(inner)?;
                    return Ok(V { c: format!("(&{})", p.c), ty: Type::Rawptr(Box::new(p.ty)), obj: None });
                }
                let v = self.lower_expr(&args[0], None)?;
                let Type::Ref(pointee, _) = &v.ty else { return internal("rawptr_of on a non-reference") };
                let r = self.into_temp(v.clone(), line)?;
                Ok(V { c: format!("({}.p)", r.c), ty: Type::Rawptr(pointee.clone()), obj: None })
            }
            "allocate" => {
                let s = self.lower_expr(&args[0], Some(&Type::Int(IntTy::Usize)))?;
                let st = self.into_temp(s, line)?;
                let a = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let at = self.into_temp(a, line)?;
                let rty = Type::Named("std::Result".into(), vec![Type::Rawptr(Box::new(Type::Int(IntTy::U8))), Type::Named("std::AllocError".into(), vec![])]);
                let rc = self.g.ctype(&rty)?;
                let t = self.g.fresh("al");
                self.emit(&format!("{} {};", rc, t));
                self.emit(&format!("{{ uint8_t *_p = cb_allocate({}, {}); if (_p) {{ {}.tag = 0u; {}.u.v0 = _p; }} else {}.tag = 1u; }}", st.c, at.c, t, t, t));
                Ok(V { c: t, ty: rty, obj: None })
            }
            "deallocate" => {
                let p = self.lower_expr(&args[0], None)?;
                let pt = self.into_temp(p, line)?;
                let s = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let st = self.into_temp(s, line)?;
                let a = self.lower_expr(&args[2], Some(&Type::Int(IntTy::Usize)))?;
                let at = self.into_temp(a, line)?;
                self.emit(&format!("cb_deallocate({}, {}, {});", pt.c, st.c, at.c));
                Ok(unit())
            }
            "release" => {
                let p = self.lower_expr(&args[0], None)?;
                let pt = self.into_temp(p, line)?;
                let n = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let nt = self.into_temp(n, line)?;
                self.emit(&format!("cb_release({}, {});", pt.c, nt.c));
                Ok(unit())
            }
            "copy_raw" => {
                let d = self.lower_expr(&args[0], None)?;
                let dt = self.into_temp(d, line)?;
                let s = self.lower_expr(&args[1], None)?;
                let st = self.into_temp(s, line)?;
                let n = self.lower_expr(&args[2], Some(&Type::Int(IntTy::Usize)))?;
                let nt = self.into_temp(n, line)?;
                self.emit(&format!("cb_copy_raw({}, {}, {});", dt.c, st.c, nt.c));
                Ok(unit())
            }
            "reinterpret_ptr" => {
                let dst = targ(0, self).expect("type argument");
                let v = self.lower_expr(&args[0], None)?;
                let ty = Type::Rawptr(Box::new(dst));
                let c = self.g.ctype(&ty)?;
                Ok(V { c: format!("(({}){})", c, v.c), ty, obj: None })
            }
            "dangling" => {
                // T-aligned and not null: `alignof<T>()` itself, as `coby`.
                let dst = targ(0, self).expect("type argument");
                let (_, align) = self.g.size_align(&dst)?;
                let ty = Type::Rawptr(Box::new(dst));
                let c = self.g.ctype(&ty)?;
                Ok(V { c: format!("(({}){}u)", c, align), ty, obj: None })
            }
            // D-0258: an address as a pointer, and back (`[Rawptr-From-Address]`,
            // `[Rawptr-Address]`): the C casts through `uintptr_t`.
            "rawptr_from_address" => {
                let a = self.lower_expr(&args[0], Some(&Type::Int(IntTy::Usize)))?;
                if matches!(a.ty, Type::Never) {
                    return Ok(never());
                }
                let ac = self.into_temp(a, line)?.c;
                let dst = targ(0, self).expect("type argument");
                let ty = Type::Rawptr(Box::new(dst));
                let c = self.g.ctype(&ty)?;
                Ok(V { c: format!("(({})(uintptr_t)({}))", c, ac), ty, obj: None })
            }
            "rawptr_address" => {
                let p = self.lower_expr(&args[0], None)?;
                if matches!(p.ty, Type::Never) {
                    return Ok(never());
                }
                let pc = self.into_temp(p, line)?.c;
                Ok(V { c: format!("((uint64_t)(uintptr_t)({}))", pc), ty: Type::Int(IntTy::Usize), obj: None })
            }
            "null_rawptr" => {
                // D-0252: C's `NULL`.
                let dst = targ(0, self).expect("type argument");
                let ty = Type::Rawptr(Box::new(dst));
                let c = self.g.ctype(&ty)?;
                Ok(V { c: format!("(({})0)", c), ty, obj: None })
            }
            "fault" => {
                let ExprKind::Path(segs, _) = &args[0].kind else { return internal("fault's argument") };
                let id = format!("diag.{}", segs[0].replace('_', "-"));
                // D-0083: an optional `str` message travels with the fault.
                if let Some(a1) = args.get(1) {
                    let v = self.lower_expr(a1, Some(&Type::Str))?;
                    let t = self.into_temp(v, line)?;
                    self.emit(&format!("cb_fault_msg(\"{}\", {}.p, {}.n, {});", id, t.c, t.c, loc));
                    return Ok(never());
                }
                self.emit(&format!("cb_fault(\"{}\", {});", id, loc));
                Ok(never())
            }
            _ => internal(&format!("unknown callee `{}`", name)),
        }
    }

    // Intrinsics of the runtime: drop, threads, locks, exit and output.
    fn lower_intrinsic_runtime(&mut self, name: &str, targs: &[Type], args: &[Expr], expected: Option<&Type>, line: usize, loc: String) -> R<V> {
        let targ = |i: usize, fx: &Self| targs.get(i).map(|t| fx.sub(t));
        match name {
            "drop" => {
                // `drop(reclaim<T>(p))` of a resource `T`: `[Destroy]` of the
                // place `[Reclaim]` makes, through its root (`spec/07` §3), as
                // `coby` has it -- not a move out of it into a temporary,
                // which a live reference into the object faulted as
                // `diag.move-while-aliased` where `[Destroy-Not-Solitary]`'s
                // `diag.destroy-while-aliased` is the rule's.
                if let ExprKind::Call(c, _) = &args[0].kind {
                    if let ExprKind::Path(s, ta) = &c.kind {
                        if s.len() == 1 && s[0] == "reclaim" && self.lookup("reclaim").is_none() && ta.len() == 1 && self.is_res(&self.sub(&ta[0])) {
                            let p = self.lower_place(&args[0])?;
                            if let Some(root) = &p.base {
                                self.emit(&format!("cb_destroy({}, {});", root, loc));
                                return Ok(unit());
                            }
                            return internal("a reclaimed place with no root");
                        }
                    }
                }
                // `drop(e)` of a temporary: the value is destroyed at once
                // (a plain one simply ends).
                if !matches!(args[0].kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(..) | ExprKind::Paren(..)) {
                    let v = self.lower_expr(&args[0], None)?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(never());
                    }
                    let t = self.into_temp(v, line)?;
                    if let Some(o) = &t.obj {
                        self.emit(&format!("cb_consume({}, {});", o, loc));
                    } else if self.holds_fn(&t.ty) {
                        let tid = self.tid(&t.ty)?;
                        self.emit(&format!("cb_drop_value(&{}, {}u);", t.c, tid));
                    }
                    return Ok(unit());
                }
                let p = self.lower_place(&args[0])?;
                if p.raw.is_some() {
                    // `drop(*p)` of raw storage: the value moves out
                    // (`[Rawptr-Move-Out]`) and is destroyed at once.
                    let v = self.read_place(p, line)?;
                    let t = self.into_temp(v, line)?;
                    if let Some(o) = &t.obj {
                        self.emit(&format!("cb_consume({}, {});", o, loc));
                    } else if self.holds_fn(&t.ty) {
                        let tid = self.tid(&t.ty)?;
                        self.emit(&format!("cb_drop_value(&{}, {}u);", t.c, tid));
                    }
                    return Ok(unit());
                }
                match (&p.base, p.is_binding, p.projs.is_empty()) {
                    (Some(root), true, true) => self.emit(&format!("cb_destroy({}, {});", root, loc)),
                    (Some(tok), false, true) => {
                        // D-0292: the referent's place, for a light reference.
                        let tid = self.tid(&p.ty)?;
                        self.emit(&format!("cb_destroy_via({}, (void *)&({}), {}u, {});", tok, p.c, tid, loc));
                    }
                    (Some(_), _, false) => {
                        // `[Destroy-Projection]`: a field, element, or payload.
                        self.emit(&format!("cb_fault(\"diag.move-out-of-field\", {});", loc));
                        return Ok(never());
                    }
                    (None, _, _) => return internal("drop of a place with no path"),
                }
                Ok(unit())
            }
            "reclaim" => internal("reclaim outside place position"),
            "spawn" => self.lower_spawn(args, line),
            "join_status" | "cancel" => {
                // D-0204: `std::try_join`'s primitive (0 when the thread
                // finished, its failure's id plus one when it failed) and
                // `cancel(&h)`, through the reference to the handle.
                let hv = self.lower_expr(&args[0], None)?;
                if matches!(hv.ty, Type::Never) {
                    return Ok(never());
                }
                let ht = self.into_temp(hv, line)?;
                self.emit(&format!("cb_read({}.tok, NULL, 0, {});", ht.c, loc));
                let tid = format!("(*(uint64_t *)({}.p))", ht.c);
                if name == "cancel" {
                    self.emit(&format!("cb_cancel({});", tid));
                    return Ok(unit());
                }
                let mv = self.lower_expr(&args[1], Some(&Type::Int(IntTy::U8)))?;
                let mt = self.into_temp(mv, line)?;
                let t = self.g.fresh("t");
                self.emit(&format!("uint64_t {} = cb_join_status({}, {});", t, tid, mt.c));
                return Ok(V { c: t, ty: Type::Int(IntTy::U64), obj: None });
            }
            "join" => {
                // `[Join]`: the handle is consumed; the result, claimed
                // from the thread's block, is this statement's temporary.
                let hv = self.lower_expr(&args[0], None)?;
                if matches!(hv.ty, Type::Never) {
                    return Ok(never());
                }
                let Type::Handle(rty) = hv.ty.clone() else { return internal("join of a non-handle") };
                let rty = *rty;
                let ht = self.into_temp(hv, line)?;
                let ho = ht.obj.clone().expect("a handle has an object");
                let t = self.g.fresh("t");
                let c = self.g.ctype(&rty)?;
                self.emit(&format!("{} {};", c, t));
                let ro = self.g.fresh("o");
                self.emit(&format!("uint64_t {} = cb_join({}, &{});", ro, ht.c, t));
                // `[Handle-Destructor]` finds the result taken: nothing to discard.
                self.emit(&format!("cb_consume({}, {});", ho, loc));
                if matches!(rty, Type::Void) {
                    return Ok(unit());
                }
                if matches!(rty, Type::Ref(..)) {
                    // D-0094: the stored form (pointer + rekeyed slot at
                    // `t`) back to register form. The send/recv pair —
                    // both on this thread — detaches the token from the
                    // ended thread's scopes and stamps it into this
                    // statement's, so it outlives the transient slot,
                    // which is forgotten with the C local it sat in.
                    let rr = self.g.fresh("r");
                    self.emit(&format!("cb_ref {} = {{ {}, cb_load_ref(&{}) }};", rr, t, t));
                    self.emit(&format!("cb_send_ref({}.tok);", rr));
                    self.emit("cb_recv_ref();");
                    if self.tracked {
                        if let Type::Ref(inner, m) = &rty {
                            let itid = self.tid(inner)?;
                            let md = if *m == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                            self.emit(&format!("{}.tok = cb_rec_ref({}.tok, {}.p, {}u, {}, 0u, CB_NOLOC, 0);", rr, rr, rr, itid, md));
                        }
                    }
                    self.emit(&format!("cb_env_forget((uint8_t *)&{}, sizeof({}));", t, t));
                    return Ok(V { c: rr, ty: rty, obj: None });
                }
                let obj = if self.is_res(&rty) {
                    Some(ro)
                } else if self.has_refs(&rty) {
                    let tid = self.tid(&rty)?;
                    let o = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
                    Some(o)
                } else {
                    None
                };
                Ok(V { c: t, ty: rty, obj })
            }
            "lock" => {
                // `[Lock]`: a guard owning the lock path, held in its
                // storage as a stored reference (`[Repr-Guard]`).
                let mv = self.lower_expr(&args[0], None)?;
                if matches!(mv.ty, Type::Never) {
                    return Ok(never());
                }
                let inner = match &mv.ty {
                    Type::Ref(p, _) => match &**p {
                        Type::Mutex(i) => (**i).clone(),
                        _ => return internal("lock of a non-mutex"),
                    },
                    _ => return internal("lock of a non-reference"),
                };
                let mt = self.into_temp(mv, line)?;
                let gt = self.g.fresh("tok");
                // D-0292: the mutex's type, for its place behind a light reference.
                let mtid = self.tid(&Type::Mutex(Box::new(inner.clone())))?;
                self.emit(&format!("uint64_t {} = cb_lock({}.tok, {}.p, {}u, {});", gt, mt.c, mt.c, mtid, loc));
                // D-0204: an interior that may hold an exclusive reference
                // (a channel of them) lends its locker another's data.
                if coby::typecheck::may_hold_excl_ref(&self.g.items, &inner) {
                    self.emit("cb_thread_lent();");
                }
                let gty = Type::Guard(Box::new(inner));
                let tid = self.tid(&gty)?;
                let g = self.g.fresh("g");
                self.emit(&format!("void *{} = {}.p;", g, mt.c));
                let o = self.g.fresh("o");
                self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, g, tid, loc));
                self.emit(&format!("cb_store_ref(&{}, {});", g, gt));
                Ok(V { c: g, ty: gty, obj: Some(o) })
            }
            "Mutex::new" => {
                // `[Mutex-New]`: `inner` holds the value; the state cells
                // are zero and never read by a rule.
                let hint = match (targ(0, self), expected) {
                    (Some(t), _) => Some(t),
                    (None, Some(Type::Mutex(i))) => Some((**i).clone()),
                    _ => None,
                };
                let v = self.lower_expr(&args[0], hint.as_ref())?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let v = self.into_temp(v, line)?;
                let inner = v.ty.clone();
                let mty = Type::Mutex(Box::new(inner.clone()));
                let c = self.g.ctype(&mty)?;
                let t = self.g.fresh("t");
                self.emit(&format!("{} {};", c, t));
                self.emit(&format!("{}.state = 0;", t));
                self.store(&format!("{}.inner", t), &inner, &v)?;
                if self.is_res(&inner) {
                    self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource has object")));
                }
                self.register_temp(t, mty, line)
            }
            "std::exit" => {
                // `[Exit]` (D-0150): the runtime unwinds this thread and
                // ends the program with the status; of type `never`.
                let v = self.lower_expr(&args[0], Some(&Type::Int(IntTy::U8)))?;
                let t = self.into_temp(v, line)?;
                self.emit(&format!("cb_exit({});", t.c));
                Ok(never())
            }
            "std::print" => {
                // `rule.stdlib.print`: `str` and `ref<String, shared>` go to
                // `std`'s two helpers; numbers and `bool` to the runtime.
                let t = self.probe_type(&args[0], None)?;
                let helper = match &t {
                    Type::Str => Some("std::print_str"),
                    Type::Ref(inner, _) if matches!(&**inner, Type::Named(n, _) if n == "std::String") => Some("std::print_string"),
                    _ => None,
                };
                if let Some(h) = helper {
                    let f = self.g.items.fns.get(h).cloned().expect("std's print helper");
                    return self.lower_user_call(h, &f, &[], args, None, line, None);
                }
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let x = self.into_temp(v.clone(), line)?.c;
                let call = match &v.ty {
                    Type::Int(t) => {
                        let wide = if t.signed() { format!("(cb_u128)(cb_i128)({})", x) } else { format!("(cb_u128)({})", x) };
                        format!("{{ cb_u128 _p = {}; cb_print_int((uint64_t)(_p >> 64), (uint64_t)_p, {}u); }}", wide, t.signed() as u32)
                    }
                    Type::F64 => format!("cb_print_f64({});", x),
                    Type::F32 => format!("cb_print_f32({});", x),
                    Type::Bool => format!("cb_print_bool({});", x),
                    _ => return internal("print of an unprintable type"),
                };
                self.emit(&call);
                Ok(unit())
            }
            // `rule.stdlib.text` (spec/21 §2d): `std`'s own helpers for
            // `String::append<T>` and `parse<T>`.
            "append_native" => {
                // `str` and `ref<String, shared>` go to `std`'s two helpers,
                // numbers and `bool` to `append_text` (through `text_write`).
                let t = self.probe_type(&args[1], None)?;
                let (h, targs) = match &t {
                    Type::Str => ("std::String::append_str", vec![]),
                    Type::Ref(inner, _) if matches!(&**inner, Type::Named(n, _) if n == "std::String") => ("std::String::append_string", vec![]),
                    Type::Named(n, _) if n == "std::StringView" => ("std::String::append_view", vec![]), // D-0053
                    _ => ("std::String::append_text", vec![t.clone()]),
                };
                let f = self.g.items.fns.get(h).cloned().expect("std's append helper");
                self.lower_user_call(h, &f, &targs, args, None, line, None)
            }
            _ => internal(&format!("unknown callee `{}`", name)),
        }
    }

    // Intrinsics of std's text and formatting: text_write, parse, assert failures, `$fmt`, map_err.
    fn lower_intrinsic_text(&mut self, name: &str, targs: &[Type], args: &[Expr], line: usize, loc: String) -> R<V> {
        let targ = |i: usize, fx: &Self| targs.get(i).map(|t| fx.sub(t));
        match name {
            "text_write" => {
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let x = self.into_temp(v.clone(), line)?.c;
                let p = self.lower_expr(&args[1], None)?;
                let p = self.into_temp(p, line)?.c;
                let c = match &v.ty {
                    Type::Int(t) => {
                        let wide = if t.signed() { format!("(cb_u128)(cb_i128)({})", x) } else { format!("(cb_u128)({})", x) };
                        format!("cb_text_int((uint64_t)({} >> 64), (uint64_t)({}), {}u, (void *)({}))", wide, wide, t.signed() as u32, p)
                    }
                    Type::F64 => format!("cb_text_f64({}, (void *)({}))", x, p),
                    Type::F32 => format!("cb_text_f32({}, (void *)({}))", x, p),
                    Type::Bool => format!("cb_text_bool({}, (void *)({}))", x, p),
                    _ => return internal("text of an unprintable type"),
                };
                Ok(V { c, ty: Type::Int(IntTy::Usize), obj: None })
            }
            "parse_check" | "parse_value" => {
                let t = targ(0, self).expect("type argument");
                let p = self.lower_expr(&args[0], None)?;
                let p = self.into_temp(p, line)?.c;
                let n = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let n = self.into_temp(n, line)?.c;
                let check = name == "parse_check";
                match &t {
                    Type::Int(it) => {
                        let head = format!("cb_parse_int((const void *)({}), {}, {}u, {}u", p, n, it.signed() as u32, it.bitwidth(64));
                        if check {
                            return Ok(V { c: format!("{}, 0)", head), ty: Type::Int(IntTy::Isize), obj: None });
                        }
                        let o = self.g.fresh("o");
                        self.emit(&format!("uint64_t {}[2];", o));
                        self.emit(&format!("{}, {});", head, o));
                        let c = format!("(({})((((cb_u128){}[0]) << 64) | {}[1]))", self.g.ctype(&t)?, o, o);
                        Ok(V { c, ty: t.clone(), obj: None })
                    }
                    Type::F64 | Type::F32 => {
                        let head = format!("cb_parse_float((const void *)({}), {}, {}u", p, n, matches!(t, Type::F32) as u32);
                        if check {
                            return Ok(V { c: format!("{}, 0)", head), ty: Type::Int(IntTy::Isize), obj: None });
                        }
                        let o = self.g.fresh("o");
                        self.emit(&format!("double {};", o));
                        self.emit(&format!("{}, &{});", head, o));
                        Ok(V { c: format!("(({}){})", self.g.ctype(&t)?, o), ty: t.clone(), obj: None })
                    }
                    // D-0181: `bool`.
                    Type::Bool => {
                        let head = format!("cb_parse_bool((const void *)({}), {}", p, n);
                        if check {
                            return Ok(V { c: format!("{}, 0)", head), ty: Type::Int(IntTy::Isize), obj: None });
                        }
                        let o = self.g.fresh("o");
                        self.emit(&format!("uint8_t {};", o));
                        self.emit(&format!("{}, &{});", head, o));
                        Ok(V { c: format!("((uint8_t){})", o), ty: Type::Bool, obj: None })
                    }
                    _ => internal("parse of a non-numeric type"),
                }
            }
            // `rule.stdlib.format` (D-0038): the text of `printf` and
            // `String::appendf` (`modres` made them `print`/`append` of
            // `$fmt(…)`), in the runtime's buffer until that takes it.
            "$assert_fail" => {
                // D-0065: the fault, with `$fmt`'s text as its message.
                if args.is_empty() {
                    self.emit(&format!("cb_fault(\"diag.assert-failed\", {});", loc));
                    return Ok(never());
                }
                let v = self.lower_expr(&args[0], Some(&Type::Str))?;
                let t = self.into_temp(v, line)?;
                self.emit(&format!("cb_fault_msg(\"diag.assert-failed\", {}.p, {}.n, {});", t.c, t.c, loc));
                Ok(never())
            }
            "$fmt" => {
                let fv = self.lower_expr(&args[0], None)?;
                let fv = self.into_temp(fv, line)?;
                let mut inits = Vec::new();
                // D-0297: each argument as a literal format written in place
                // would take it (`None`: only `cb_format` takes it).
                let mut pargs: Vec<Option<FmtArg>> = Vec::new();
                for a in &args[1..] {
                    // `&s` of a `String` local that no other argument names
                    // (decided before any probe: an objectless local has no
                    // path to probe a borrow of):
                    // read where it is formatted, with nothing run between
                    // that could change it, so the read's check through `s`'s
                    // root (none for an objectless local) stands for the
                    // borrow's; no path is minted.
                    let direct = match &strip_parens(a).kind {
                        ExprKind::Borrow(Mode::Shared, place) => path_name(place)
                            .filter(|x| args[1..].iter().filter(|o| mentions(o, x)).count() == 1)
                            .and_then(|x| self.lookup(x))
                            .filter(|b| matches!(b.kind, BindKind::Local) && matches!(&b.ty, Type::Named(n, g) if n == "std::String" && g.is_empty()) && !self.direct_refs.contains(&b.c) && !self.unstored.contains_key(&b.c) && (self.objectless_c.contains(&b.c) || !b.root.is_empty())),
                        _ => None,
                    };
                    if let Some(b) = direct {
                        if !self.objectless_c.contains(&b.c) {
                            let loc = self.loc(a.line);
                            self.emit(&format!("cb_read({}, NULL, 0, {});", b.root, loc));
                        } else {
                            self.objectless_use(&b.c, a.line);
                        }
                        inits.push(format!("{{ 3u, 0u, 0, 0, 0.0, ({}).bytes.ptr, ({}).bytes.len }}", b.c, b.c));
                        pargs.push(Some(FmtArg::Text(format!("({}).bytes.ptr", b.c), format!("({}).bytes.len", b.c))));
                        continue;
                    }
                    // A reference to a number, `bool` or `str` is formatted
                    // as the value it refers to (D-0042).
                    let at = self.probe_type(a, None)?;
                    let deref_arg;
                    let a = match &at {
                        Type::Ref(inner, _) if matches!(&**inner, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str) => {
                            deref_arg = std::rc::Rc::new(Expr { kind: ExprKind::Deref(Box::new(a.clone())), line: a.line, col: a.col });
                            self.kept.push(deref_arg.clone());
                            &*deref_arg
                        }
                        _ => a,
                    };
                    let v = self.lower_expr(a, None)?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(never());
                    }
                    let v = self.into_temp(v, a.line)?;
                    let x = v.c.clone();
                    pargs.push(match &v.ty {
                        Type::Int(t) if t.bitwidth(64) <= 64 => Some(FmtArg::Int(t.signed(), t.bitwidth(64), x.clone())),
                        Type::Str => Some(FmtArg::Text(format!("({}).p", x), format!("({}).n", x))),
                        Type::Bool => Some(FmtArg::Text(format!("(const uint8_t *)(({}) ? \"true\" : \"false\")", x), format!("(({}) ? 4u : 5u)", x))),
                        Type::Named(n, _) if n == "std::StringView" => Some(FmtArg::Text(format!("(const uint8_t *)({}).bytes.data", x), format!("({}).bytes.len", x))),
                        Type::Ref(inner, _) if !matches!(&**inner, Type::Named(n, _) if self.g.items.enums.contains_key(n)) => {
                            let sc = self.g.ctype(&Type::Named("std::String".into(), vec![]))?;
                            Some(FmtArg::Text(format!("(({} *)({}).p)->bytes.ptr", sc, x), format!("(({} *)({}).p)->bytes.len", sc, x)))
                        }
                        _ => None,
                    });
                    inits.push(match &v.ty {
                        Type::Int(t) => {
                            let wide = if t.signed() { format!("(cb_u128)(cb_i128)({})", x) } else { format!("(cb_u128)({})", x) };
                            format!("{{ {}u, {}u, (uint64_t)({} >> 64), (uint64_t)({}), 0.0, 0, 0 }}", t.signed() as u32, t.bitwidth(64), wide, wide)
                        }
                        Type::F64 => format!("{{ 2u, 64u, 0, 0, {}, 0, 0 }}", x),
                        Type::F32 => format!("{{ 2u, 32u, 0, 0, (double)({}), 0, 0 }}", x),
                        Type::Str => format!("{{ 3u, 0u, 0, 0, 0.0, ({}).p, ({}).n }}", x, x),
                        Type::Bool => format!("{{ 3u, 0u, 0, 0, 0.0, (const uint8_t *)(({}) ? \"true\" : \"false\"), ({}) ? 4u : 5u }}", x, x),
                        // D-0053: a view's bytes, read through its slice's reference.
                        Type::Named(n, _) if n == "std::StringView" => {
                            let loc = self.loc(a.line);
                            self.emit(&format!("cb_read(cb_load_ref(&({}).bytes.src), NULL, 0, {});", x, loc));
                            format!("{{ 3u, 0u, 0, 0, 0.0, (const uint8_t *)({}).bytes.data, ({}).bytes.len }}", x, x)
                        }
                        // D-0100: an enum prints as its variant's name.
                        Type::Named(n, _) if self.g.items.enums.contains_key(n) => {
                            let names: Vec<String> = self.g.items.enums[n].variants.iter().map(|v| v.name.clone()).collect();
                            self.variant_name_text(&format!("({}).tag", x), &names)
                        }
                        Type::Ref(inner, _) if matches!(&**inner, Type::Named(n, _) if self.g.items.enums.contains_key(n)) => {
                            let Type::Named(n, _) = &**inner else { unreachable!() };
                            let names: Vec<String> = self.g.items.enums[n].variants.iter().map(|v| v.name.clone()).collect();
                            let loc = self.loc(a.line);
                            self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", x, loc));
                            let et = self.g.ctype(inner)?;
                            self.variant_name_text(&format!("(({} *)({}).p)->tag", et, x), &names)
                        }
                        Type::Ref(..) => {
                            // A `&String`: its bytes, read through the reference.
                            let loc = self.loc(a.line);
                            self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", x, loc));
                            let sc = self.g.ctype(&Type::Named("std::String".into(), vec![]))?;
                            format!("{{ 3u, 0u, 0, 0, 0.0, (({} *)({}).p)->bytes.ptr, (({} *)({}).p)->bytes.len }}", sc, x, sc, x)
                        }
                        _ => return internal("a format argument of an unformattable type"),
                    });
                }
                let lit = match &strip_parens(&args[0]).kind {
                    ExprKind::StrLit(f) => fmt_plan(f.as_bytes(), &pargs),
                    _ => None,
                };
                if let Some(ops) = lit {
                    return Ok(self.fmt_in_place(&fv.c, &ops, &pargs));
                }
                let arr = self.g.fresh("fa");
                let out = self.g.fresh("fs");
                if inits.is_empty() {
                    self.emit(&format!("cb_str {}; cb_format(({}).p, ({}).n, NULL, 0, &{});", out, fv.c, fv.c, out));
                } else {
                    self.emit(&format!("cb_fmt_arg {}[] = {{ {} }};", arr, inits.join(", ")));
                    self.emit(&format!("cb_str {}; cb_format(({}).p, ({}).n, {}, {}, &{});", out, fv.c, fv.c, arr, inits.len(), out));
                }
                Ok(V { c: out, ty: Type::Str, obj: None })
            }
            "std::Result::map_err" => {
                // `map_err(r, f)`: `Ok(v)` passes through, `Err(e)` becomes
                // `Err(f(e))`; the source's contents relocate either way.
                let rv = self.lower_expr(&args[0], None)?;
                if matches!(rv.ty, Type::Never) {
                    return Ok(never());
                }
                let rt = self.into_temp(rv, line)?;
                let (t_ty, e_ty) = match &rt.ty {
                    Type::Named(n, a) if n == "std::Result" && a.len() == 2 => (a[0].clone(), a[1].clone()),
                    _ => return internal("map_err on a non-Result"),
                };
                let fv = self.lower_expr(&args[1], None)?;
                let e2 = match &fv.ty {
                    Type::Fn(_, r) => (**r).clone(),
                    Type::Closure(id) => self.g.closures[id].ret.clone(),
                    _ => return internal("map_err's function"),
                };
                let out_ty = Type::Named("std::Result".into(), vec![t_ty.clone(), e2.clone()]);
                let oc = self.g.ctype(&out_ty)?;
                let o = self.g.fresh("me");
                self.emit(&format!("{} {};", oc, o));
                self.emit(&format!("if ({}.tag == 0u) {{", rt.c));
                self.ind += 1;
                self.emit(&format!("{}.tag = 0u;", o));
                let pv = V { c: format!("{}.u.v0", rt.c), ty: t_ty.clone(), obj: None };
                let pv = if matches!(t_ty, Type::Ref(..)) { self.read_ref_slot(&pv.c, &t_ty) } else { pv };
                self.store(&format!("{}.u.v0", o), &t_ty, &pv)?;
                self.ind -= 1;
                self.emit("} else {");
                self.ind += 1;
                // The Err payload becomes the call's argument.
                let ev = V { c: format!("{}.u.v1", rt.c), ty: e_ty.clone(), obj: None };
                let ev = if matches!(e_ty, Type::Ref(..)) { self.read_ref_slot(&ev.c, &e_ty) } else { ev };
                let ev = if self.is_res(&e_ty) {
                    let t = self.g.fresh("t");
                    let c = self.g.ctype(&e_ty)?;
                    self.emit(&format!("{} {} = {};", c, t, ev.c));
                    if self.g.decided {
                        // D-0286: handed over as it is, with no object.
                        self.inert_c.insert(t.clone());
                        V { c: t, ty: e_ty.clone(), obj: None }
                    } else {
                        let tid = self.tid(&e_ty)?;
                        let oo = self.g.fresh("o");
                        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", oo, t, tid, loc));
                        V { c: t, ty: e_ty.clone(), obj: Some(oo) }
                    }
                } else {
                    self.into_temp(ev, line)?
                };
                if self.is_res(&e_ty) {
                    self.give_arg(&ev, "map_err's error")?;
                } else if self.has_refs(&e_ty) && !matches!(e_ty, Type::Ref(..)) {
                    let tid = self.tid(&e_ty)?;
                    self.emit(&format!("cb_send_datum(&{}, {}u);", ev.c, tid));
                }
                let res = match &fv.ty {
                    Type::Fn(..) => {
                        let fh = self.into_temp(fv.clone(), line)?;
                        let thunk = self.g.thunk(&fv.ty)?;
                        self.finish_call(format!("{}({}, {})", thunk, fh.c, ev.c), e2.clone(), line)?
                    }
                    Type::Closure(id) => {
                        let id = *id;
                        let info = self.g.closures[&id].clone();
                        let Some(co) = &fv.obj else { return internal("closure without an object") };
                        let root = self.g.fresh("root");
                        self.emit(&format!("uint64_t {} = cb_bind({});", root, co));
                        let tok = self.g.fresh("tok");
                        self.emit(&format!("uint64_t {} = cb_borrow({}, NULL, 0, CB_EXCLUSIVE, {});", tok, root, loc));
                        let self_ref = format!("((cb_ref){{ (void *)&({}), {} }})", fv.c, tok);
                        self.finish_call(format!("{}({}, {})", info.cname, self_ref, ev.c), e2.clone(), line)?
                    }
                    _ => unreachable!(),
                };
                let res = self.into_temp(res, line)?;
                self.emit(&format!("{}.tag = 1u;", o));
                self.store(&format!("{}.u.v1", o), &e2, &res)?;
                if self.is_res(&e2) {
                    self.emit(&format!("cb_absorb({});", res.obj.as_ref().expect("resource result has object")));
                }
                self.ind -= 1;
                self.emit("}");
                if let Some(ro) = &rt.obj {
                    self.emit(&format!("cb_end_moved_out({});", ro));
                }
                self.register_temp(o, out_ty, line)
            }
            _ => internal(&format!("unknown callee `{}`", name)),
        }
    }

    // Intrinsics std's own code uses on places: foreach, clone, lending, swapping, key order, hash and equality.
    fn lower_intrinsic_places(&mut self, name: &str, args: &[Expr], expected: Option<&Type>, line: usize, loc: String) -> R<V> {
        match name {
            // `foreach` (D-0042): the expansion for the type of the hidden
            // binding the call names first (coby's src/each.rs).
            n if coby::each::NAMES.contains(&n) => {
                let t0 = self.probe_type(&args[0], None)?;
                match coby::each::expand(n, &t0, args, line, args.first().map_or(0, |a| a.col)) {
                    Ok(x) => {
                        // Kept alive: `probes` is keyed by address.
                        let x = std::rc::Rc::new(x);
                        self.kept.push(x.clone());
                        self.lower_expr(&x, expected)
                    }
                    Err(d) => internal(&format!("foreach over {:?}: {}", t0, d)),
                }
            }
            // `swap_places(a, b)` (D-0048): the bytes of the two places
            // exchange, and so do the reference slots' tokens a value with
            // references keeps beside its bytes.
            // D-0116: `clone(&x)`: a plain `T` is `*r`; a resource `T` is a
            // call of its `T::clone`, declared or derived, at `T`'s arguments.
            "clone" => {
                // `&p` probed as `p`: a direct slice's element has no path
                // to borrow (`Gen::direct_refs_of`).
                let ty = match &args[0].kind {
                    ExprKind::Borrow(Mode::Shared, place) => self.probe_type(place, None)?,
                    _ => {
                        let at = self.probe_type(&args[0], None)?;
                        let Type::Ref(inner, _) = &at else { return internal("clone of a non-reference") };
                        (**inner).clone()
                    }
                };
                if !self.g.is_resource(&ty) {
                    // `clone(&p)` of a place `p`: a read of `p` -- the same
                    // check a shared borrow's formation and its read make,
                    // with no path formed (`[T-Clone]` of a plain value is
                    // its read).
                    if let ExprKind::Borrow(Mode::Shared, place) = &args[0].kind {
                        return self.lower_expr(place, expected);
                    }
                    let d = Expr { kind: ExprKind::Deref(Box::new(args[0].clone())), line, col: args[0].col };
                    return self.lower_expr(&d, expected);
                }
                let Type::Named(n, targs) = &ty else { return internal("clone of a resource that is not a named type") };
                let key = format!("{}::clone", n);
                let Some(f) = self.g.items.fns.get(&key).cloned() else { return internal("clone of a type with no clone") };
                return self.lower_user_call(&key, &f, targs, args, expected, line, None);
            }
            // D-0213 `[Thread-Lend]` (std-only, `Channel::send`): the
            // exclusive references the place holds are lent.
            "lend_places" => {
                let at = self.probe_type(&args[0], None)?;
                let Type::Ref(inner, _) = &at else { return internal("lend of a non-reference") };
                let ty = (**inner).clone();
                let a = self.lower_expr(&args[0], None)?;
                let a = self.into_temp(a, line)?;
                if self.has_refs(&ty) {
                    let c = self.g.ctype(&ty)?;
                    let loc = self.loc(line);
                    // D-0235: the place is written by `[Write]`.
                    self.emit(&format!("cb_write(({}).tok, NULL, 0, 0u, {});", a.c, loc));
                    self.emit(&format!("cb_lend(({}).p, sizeof({}), {});", a.c, c, loc));
                }
                Ok(unit())
            }
            "swap_places" => {
                let at = self.probe_type(&args[0], None)?;
                let Type::Ref(inner, _) = &at else { return internal("swap of a non-reference") };
                let ty = (**inner).clone();
                let a = self.lower_expr(&args[0], None)?;
                let a = self.into_temp(a, line)?;
                let b = self.lower_expr(&args[1], None)?;
                let b = self.into_temp(b, line)?;
                let c = self.g.ctype(&ty)?;
                let loc = self.loc(line);
                // `[Swap-Places]` needs both places still there; the same
                // place twice is left alone, and otherwise each place is
                // written by `[Write]` (D-0235): the token's thread and no
                // conflicting live path, as for `*a = …`.
                self.emit(&format!("cb_valid(({}).tok, {});", a.c, loc));
                self.emit(&format!("cb_valid(({}).tok, {});", b.c, loc));
                let (pa, pb) = (format!("(({} *)({}).p)", c, a.c), format!("(({} *)({}).p)", c, b.c));
                let t = self.g.fresh("sw");
                self.emit(&format!("if ({} != {}) {{", pa, pb));
                self.emit(&format!("cb_write(({}).tok, NULL, 0, 0u, {});", a.c, loc));
                self.emit(&format!("cb_write(({}).tok, NULL, 0, 0u, {});", b.c, loc));
                self.emit(&format!("{} {}; memcpy(&{}, {}, sizeof({}));", c, t, t, pa, c));
                if self.has_refs(&ty) {
                    // Each place's reference slots move with its bytes: a
                    // destination's old ones are forgotten before the new
                    // ones are copied in, and the temporary's after; a slot
                    // left behind (`None` swapped over `Some(r)`) kept `r`'s
                    // path counted as held.
                    let tid = self.tid(&ty)?;
                    self.emit(&format!("cb_copy_datum(&{}, {}, {}u);", t, pa, tid));
                    self.emit(&format!("cb_env_forget((uint8_t *){}, sizeof({}));", pa, c));
                    self.emit(&format!("memcpy({}, {}, sizeof({})); cb_copy_datum({}, {}, {}u);", pa, pb, c, pa, pb, tid));
                    self.emit(&format!("cb_env_forget((uint8_t *){}, sizeof({}));", pb, c));
                    self.emit(&format!("memcpy({}, &{}, sizeof({})); cb_copy_datum({}, &{}, {}u);", pb, t, c, pb, t, tid));
                    self.emit(&format!("cb_env_forget((uint8_t *)&{}, sizeof({}));", t, c));
                } else {
                    self.emit(&format!("memcpy({}, {}, sizeof({}));", pa, pb, c));
                    self.emit(&format!("memcpy({}, &{}, sizeof({}));", pb, t, c));
                }
                self.emit("}");
                Ok(unit())
            }
            // `rule.stdlib.hashmap` (spec/21 §1a, D-0041): a key's hash and
            // equality over its bytes, read through the key's reference.
            // D-0062: the key types' order.
            // `key_less_at(v, i, j)`: `key_less` of elements `i` and `j` of the
            // `Vec` `v` refers to, read in place. `v` is borrowed shared for
            // the call, so no other path can write them; one access check
            // on `v`, none per element.
            "key_less_at" => {
                let vt = self.probe_type(&args[0], None)?;
                let Type::Ref(inner, _) = &vt else { return internal("key_less_at of a non-reference") };
                let Type::Named(_, targs0) = &**inner else { return internal("key_less_at of a non-Vec") };
                let elem = targs0.first().cloned().unwrap_or(Type::Void);
                let vc = self.g.ctype(inner)?;
                let r = self.lower_expr(&args[0], None)?;
                let r = self.into_temp(r, line)?.c;
                let i = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let i = self.into_temp(i, line)?.c;
                let j = self.lower_expr(&args[2], Some(&Type::Int(IntTy::Usize)))?;
                let j = self.into_temp(j, line)?.c;
                self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", r, loc));
                let len = format!("(({} *)({}).p)->len", vc, r);
                self.emit(&format!("cb_index_check((uint64_t){}, {}, {}); cb_index_check((uint64_t){}, {}, {});", i, len, loc, j, len, loc));
                let at = |k: &str| format!("(({} *)({}).p)->ptr[{}]", vc, r, k);
                // A non-key element: the call is on a path `std` never takes
                // for that type (`PriorityQueue::before` for a queue made by
                // `new_by`, D-0137), since the front end checks every call
                // that reaches the key order (`[Sort-Not-Key]`,
                // `[Priority-Queue-Not-Key]`). It lowers to the evaluator's
                // dynamic verdict, as `[Match]` on a non-enum does.
                let key = match &elem {
                    Type::Named(n, _) if n == "std::String" => Some(format!("cb_bytes_less((const void *){}.bytes.ptr, {}.bytes.len, (const void *){}.bytes.ptr, {}.bytes.len)", at(&i), at(&i), at(&j), at(&j))),
                    // D-0110: a struct or enum key.
                    Type::Named(..) => self.g.key_cmp_fn(&elem).ok().map(|f| format!("((uint8_t)({}(&{}, &{}) < 0))", f, at(&i), at(&j))),
                    Type::Str => Some(format!("cb_bytes_less((const void *){}.p, {}.n, (const void *){}.p, {}.n)", at(&i), at(&i), at(&j), at(&j))),
                    Type::Int(_) | Type::Bool => Some(format!("((uint8_t)({} < {}))", at(&i), at(&j))),
                    // D-0181: a float's order, a NaN after every other value.
                    Type::F32 | Type::F64 => Some(format!("((uint8_t)({a} < {b} || ({b} != {b} && {a} == {a})))", a = at(&i), b = at(&j))),
                    _ => None,
                };
                let c = match key {
                    Some(c) => c,
                    None => {
                        self.emit(&format!("cb_fault(\"diag.type-mismatch\", {});", loc));
                        "((uint8_t)0)".to_string()
                    }
                };
                return Ok(V { c, ty: Type::Bool, obj: None });
            }
            "key_less" => {
                let t = self.probe_type(&args[0], None)?;
                if let Some(kt) = composite_key(&t) {
                    let f = self.g.key_cmp_fn(&kt)?;
                    let a = self.lower_key_ptr(&args[0], &kt, line)?;
                    let b = self.lower_key_ptr(&args[1], &kt, line)?;
                    return Ok(V { c: format!("((uint8_t)({}({}, {}) < 0))", f, a, b), ty: Type::Bool, obj: None });
                }
                let text = matches!(&t, Type::Ref(inner, _) if matches!(&**inner, Type::Str) || matches!(&**inner, Type::Named(n, _) if n == "std::String"));
                if text {
                    let a = self.lower_key_bytes(&args[0], line)?;
                    let b = self.lower_key_bytes(&args[1], line)?;
                    return Ok(V { c: format!("cb_bytes_less({}, {}, {}, {})", a.0, a.1, b.0, b.1), ty: Type::Bool, obj: None });
                }
                let mut vs = Vec::new();
                let mut float = false;
                for a in args {
                    let d = std::rc::Rc::new(Expr { kind: ExprKind::Deref(Box::new(a.clone())), line: a.line, col: a.col });
                    self.kept.push(d.clone());
                    let v = self.lower_expr(&d, None)?;
                    float = matches!(v.ty, Type::F32 | Type::F64);
                    vs.push(self.into_temp(v, line)?.c);
                }
                if float {
                    // D-0181: a NaN after every other value.
                    return Ok(V { c: format!("((uint8_t)({a} < {b} || ({b} != {b} && {a} == {a})))", a = vs[0], b = vs[1]), ty: Type::Bool, obj: None });
                }
                Ok(V { c: format!("((uint8_t)({} < {}))", vs[0], vs[1]), ty: Type::Bool, obj: None })
            }
            "key_hash" | "key_eq" => {
                let t = self.probe_type(&args[0], None)?;
                if let Some(kt) = composite_key(&t) {
                    let mut ps = Vec::new();
                    for a in args {
                        ps.push(self.lower_key_ptr(a, &kt, line)?);
                    }
                    return Ok(if name == "key_hash" {
                        let f = self.g.key_hash_fn(&kt)?;
                        V { c: format!("{}(14695981039346656037ull, {})", f, ps[0]), ty: Type::Int(IntTy::U64), obj: None }
                    } else {
                        let f = self.g.key_cmp_fn(&kt)?;
                        V { c: format!("((uint8_t)({}({}, {}) == 0))", f, ps[0], ps[1]), ty: Type::Bool, obj: None }
                    });
                }
                let mut keys = Vec::new();
                for a in args {
                    keys.push(self.lower_key_bytes(a, line)?);
                }
                let c = if name == "key_hash" {
                    format!("cb_hash_bytes({}, {})", keys[0].0, keys[0].1)
                } else {
                    format!("cb_bytes_eq({}, {}, {}, {})", keys[0].0, keys[0].1, keys[1].0, keys[1].1)
                };
                let ty = if name == "key_hash" { Type::Int(IntTy::U64) } else { Type::Bool };
                Ok(V { c, ty, obj: None })
            }
            _ => internal(&format!("unknown callee `{}`", name)),
        }
    }

    pub(super) fn lower_operands(&mut self, l: &Expr, r: &Expr, expected: Option<&Type>) -> R<(Option<V>, Option<V>)> {
        let exp = match expected {
            Some(Type::Named(n, a)) if n == "std::Option" && a.len() == 1 => Some(a[0].clone()),
            Some(t) => Some(t.clone()),
            None => None,
        };
        if is_bare_literal(l) && !is_bare_literal(r) {
            let rv = self.lower_expr(r, exp.as_ref())?;
            if matches!(rv.ty, Type::Never) {
                return Ok((None, None));
            }
            let rv = self.into_temp(rv, r.line)?;
            let lv = self.lower_expr(l, Some(&rv.ty))?;
            let lv = self.into_temp(lv, l.line)?;
            Ok((Some(lv), Some(rv)))
        } else {
            let lv = self.lower_expr(l, exp.as_ref())?;
            if matches!(lv.ty, Type::Never) {
                return Ok((None, None));
            }
            let lv = self.into_temp(lv, l.line)?;
            let rv = self.lower_expr(r, Some(&lv.ty))?;
            if matches!(rv.ty, Type::Never) {
                return Ok((None, None));
            }
            let rv = self.into_temp(rv, r.line)?;
            Ok((Some(lv), Some(rv)))
        }
    }
}

// D-0297: a `$fmt` argument as a literal format written in place takes it:
// an integer of at most 64 bits (signed, its width, the C value), or text
// (its bytes' pointer and length).
pub(super) enum FmtArg {
    Int(bool, u32, String),
    Text(String, String),
}

// One piece of a literal format written in place: bytes of the format
// itself (offset, length), or an argument's text: a decimal or hexadecimal
// integer padded to a width (`how`: `cb_fmt_pad`'s), or text as it is.
pub(super) enum FmtOp {
    Lit(usize, usize),
    Dec(usize, usize, u32),
    Hex(usize, bool, usize, u32),
    Text(usize),
}

// The pieces of the literal format `f` when each of its specifiers is one
// written in place -- `%v`, `%d`, `%i` or `%u` of an integer and `%x` or
// `%X`, with a width and the `-` and `0` flags, and `%v` or `%s` of text
// with no width -- else `None` (`cb_format` renders it). The front end has
// checked `f` against its arguments.
pub(super) fn fmt_plan(f: &[u8], args: &[Option<FmtArg>]) -> Option<Vec<FmtOp>> {
    let mut ops = Vec::new();
    let mut k = 0;
    let mut i = 0;
    let mut start = 0;
    while i < f.len() {
        if f[i] != b'%' {
            i += 1;
            continue;
        }
        if i > start {
            ops.push(FmtOp::Lit(start, i - start));
        }
        i += 1;
        if f.get(i) == Some(&b'%') {
            ops.push(FmtOp::Lit(i, 1));
            i += 1;
            start = i;
            continue;
        }
        let (mut left, mut zero) = (false, false);
        while let Some(&c) = f.get(i) {
            match c {
                b'-' => left = true,
                b'0' => zero = true,
                b'+' | b' ' | b'#' => return None,
                _ => break,
            }
            i += 1;
        }
        let mut w = 0;
        while let Some(&c) = f.get(i).filter(|c| c.is_ascii_digit()) {
            w = w * 10 + (c - b'0') as usize;
            i += 1;
        }
        let conv = *f.get(i)?;
        i += 1;
        let how = if left { 1 } else if zero { 2 } else { 0 };
        let op = match (conv, args.get(k)?.as_ref()?) {
            (b'v' | b'd' | b'i' | b'u', FmtArg::Int(..)) => FmtOp::Dec(k, w, how),
            (b'x' | b'X', FmtArg::Int(..)) => FmtOp::Hex(k, conv == b'X', w, how),
            (b'v' | b's', FmtArg::Text(..)) if w == 0 => FmtOp::Text(k),
            _ => return None,
        };
        ops.push(op);
        k += 1;
        start = i;
    }
    if i > start {
        ops.push(FmtOp::Lit(start, i - start));
    }
    if k != args.len() {
        return None;
    }
    Some(ops)
}

impl<'g, 'a> Fx<'g, 'a> {
    // D-0297: the text of a literal format, written in place (`fmt_plan`'s
    // pieces): into the frame when its length has a small bound, else into
    // the runtime's space for its length; the same bytes `cb_format` makes.
    pub(super) fn fmt_in_place(&mut self, f: &str, ops: &[FmtOp], args: &[Option<FmtArg>]) -> V {
        if ops.is_empty() {
            let out = self.g.fresh("fs");
            self.emit(&format!("cb_str {} = {{ ({}).p, 0 }};", out, f));
            return V { c: out, ty: Type::Str, obj: None };
        }
        if let [FmtOp::Lit(0, _)] = ops {
            let out = self.g.fresh("fs");
            self.emit(&format!("cb_str {} = {};", out, f));
            return V { c: out, ty: Type::Str, obj: None };
        }
        let mut bound = 0;
        let mut texts = Vec::new();
        for op in ops {
            match op {
                FmtOp::Lit(_, n) => bound += n,
                FmtOp::Dec(_, w, _) => bound += (*w).max(20),
                FmtOp::Hex(_, _, w, _) => bound += (*w).max(16),
                FmtOp::Text(k) => {
                    if let Some(FmtArg::Text(_, n)) = &args[*k] {
                        texts.push(n.clone());
                    }
                }
            }
        }
        let fp = self.g.fresh("fp");
        let fk = self.g.fresh("fk");
        let out = self.g.fresh("fs");
        if texts.is_empty() && bound <= 256 {
            self.emit(&format!("uint8_t {}[{}];", fp, bound));
        } else {
            let mut total = format!("(uint64_t){}u", bound);
            for n in &texts {
                total.push_str(&format!(" + {}", n));
            }
            self.emit(&format!("uint8_t *{} = cb_fmt_space({});", fp, total));
        }
        self.emit(&format!("uint64_t {} = 0;", fk));
        for op in ops {
            let (k, render, w, how) = match op {
                FmtOp::Lit(at, n) => {
                    self.emit(&format!("memcpy({} + {}, ({}).p + {}u, {}u);", fp, fk, f, at, n));
                    self.emit(&format!("{} += {}u;", fk, n));
                    continue;
                }
                FmtOp::Text(k) => {
                    let Some(FmtArg::Text(p, n)) = &args[*k] else { continue };
                    self.emit(&format!("if ({} != 0) memcpy({} + {}, {}, {});", n, fp, fk, p, n));
                    self.emit(&format!("{} += {};", fk, n));
                    continue;
                }
                FmtOp::Dec(k, w, how) => (*k, None, *w, *how),
                FmtOp::Hex(k, upper, w, how) => (*k, Some(*upper), *w, *how),
            };
            let Some(FmtArg::Int(signed, bits, x)) = &args[k] else { continue };
            let r = match render {
                None if *signed => format!("cb_fmt_i64({} + {}, (int64_t)({}))", fp, fk, x),
                None => format!("cb_fmt_u64({} + {}, (uint64_t)({}))", fp, fk, x),
                Some(upper) => format!("cb_fmt_hex({} + {}, (uint64_t)(uint{}_t)({}), {}u)", fp, fk, bits, x, upper as u32),
            };
            if w > 0 {
                self.emit(&format!("{} += cb_fmt_pad({} + {}, {}, {}u, {}u);", fk, fp, fk, r, w, how));
            } else {
                self.emit(&format!("{} += {};", fk, r));
            }
        }
        self.emit(&format!("cb_str {} = {{ {}, {} }};", out, fp, fk));
        V { c: out, ty: Type::Str, obj: None }
    }
}
