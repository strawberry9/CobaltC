// One function's C: the plain, direct, frozen and dual bodies.

use super::*;

impl<'a> Gen<'a> {
    pub(super) fn lower_fn(&mut self, name: &str, targs: &[Type], mask: u64) -> R<()> {
        let f = self.items.fns.get(name).cloned().expect("requested fn exists");
        // The static pass checks the program's items and the `std` items
        // they reach; what it records (view operations, temporaries,
        // match hints) is what this lowering reads. A `std` function
        // the lowering itself asks for (`printf`'s helpers, `Vec`'s
        // growth, a destructor) is checked now, before it is lowered.
        if !self.items.checked_fns.lock().unwrap().contains(&(name.to_string(), targs.to_vec())) {
            if let Err(d) = coby::typecheck::check_more(&self.items, name, targs) {
                return internal(&format!("`{}` is lowered but does not check: {}", name, d));
            }
        }
        // Temporaries are numbered per function, so a function's text is
        // the same whatever else the program contains (cache.rs).
        self.uid = 0;
        let cname = if mask == 0 {
            self.fn_names[&(name.to_string(), targs.to_vec())].clone()
        } else {
            self.variants[&(name.to_string(), targs.to_vec(), mask)].clone()
        };
        let subst = subst_of(&f.type_params, targs);
        let ret = apply(&f.ret, &subst);
        let ret_c = self.reg_ctype(&ret)?;
        let mut params_c = Vec::new();
        for p in &f.params {
            let pty = apply(&p.ty, &subst);
            let pc = self.param_ctype(&pty)?;
            params_c.push(format!("{} p_{}", pc, san(&p.name)));
        }
        // `static`: nothing outside the program's own C calls it by name
        // (the runtime reaches it through pointers, and foreign code is
        // only called, `rule.trust.extern-code`), so the C compiler may
        // inline it wherever it is called, and drop it where it is not.
        // Not `main`: inlined into `cb_program`, which only C's `main`
        // calls, GCC would compile the whole program as code run once,
        // for size (a `div` for a `%` by a constant, say).
        // External linkage: a function may be compiled in a translation
        // unit of its own and reused from the object cache (cache.rs).
        let sig = format!("{} {}({})", ret_c, cname, if params_c.is_empty() { "void".to_string() } else { params_c.join(", ") });
        writeln!(self.protos, "{};", sig).unwrap();

        // D-0274: no native body in the bootstrap form.
        if mask == 0 && !self.boot(5) {
            self.native_ctx = Some((name.to_string(), f.clone(), targs.to_vec(), ret_c.clone(), sig.clone()));
            let native = self.native_vec_index(name, &f, targs, &cname);
            self.native_ctx = None;
            if let Some(body) = native? {
                write!(self.fns, "{}\n{{\n{}}}\n\n", sig, body).unwrap();
                // D-0200: the `__raw` form a caller binding the result
                // objectless calls (`raw_ret_ok`): the native's own, or the
                // native called and its result's record ended at once.
                if targs.is_empty() && name != "main" && self.raw_ret_ok(name) {
                    let open = sig.find('(').expect("a signature");
                    let raw_sig = format!("{} {}__raw{}", ret_c, cname, &sig[open..]);
                    writeln!(self.protos, "{};", raw_sig).unwrap();
                    let raw = match self.native_raw_body(name, &body) {
                        Some(b) => b,
                        None => {
                            let args: Vec<String> = f.params.iter().map(|p| format!("p_{}", san(&p.name))).collect();
                            format!("    {} r = {}({});\n    cb_forget_obj(cb_recv(&r));\n    return r;\n", ret_c, cname, args.join(", "))
                        }
                    };
                    write!(self.fns, "{}\n{{\n{}}}\n\n", raw_sig, raw).unwrap();
                }
                // `sort`'s unchecked form only for its integer and `bool`
                // body (`native_vec_sort`; `lower_confining_call` asks for
                // no other).
                let nc_shape = name != "std::Vec::sort" || matches!(targs, [Type::Int(_) | Type::Bool]);
                // D-0192: the form `lower_vec_checked_once` calls, its checks
                // through the vector's path made at the call site.
                if ONCE_CALLS.contains(&name) {
                    let open = sig.find('(').expect("a signature");
                    let once_sig = format!("{} {}_once{}", ret_c, cname, &sig[open..]);
                    writeln!(self.protos, "{};", once_sig).unwrap();
                    write!(self.fns, "{}\n{{\n{}}}\n\n", once_sig, once_variant(&body)).unwrap();
                }
                if CONFINING_CALLS.contains(&name) && nc_shape {
                    let open = sig.find('(').expect("a signature");
                    let nc_sig = format!("{} {}_nc{}", ret_c, cname, &sig[open..]);
                    writeln!(self.protos, "{};", nc_sig).unwrap();
                    write!(self.fns, "{}\n{{\n{}}}\n\n", nc_sig, nc_variant(&body)).unwrap();
                }
                return Ok(());
            }
        }

        if mask == 0 && name != "main" {
            if let Some(dual) = self.dual_eligible(&f, &subst)? {
                return self.lower_fn_dual(name, &f, targs, &cname, &sig, &ret_c, &dual);
            }
            if let Some((b, fields, direct_b)) = self.field_dual_eligible(&f, &subst)? {
                return self.lower_fn_field_dual(name, &f, targs, &cname, &sig, &ret_c, &b, &fields, direct_b);
            }
            // CHG-0237: frozen shared reference parameters: a frozen body and a
            // checked one, chosen at entry (`lower_fn_frozen`).
            {
                let direct = self.direct_ref_params(&f, &subst)?;
                let frozen: Vec<String> = self.unstored_ref_params(&f, &subst, &direct)?.into_iter().filter(|n| f.params.iter().any(|p| &p.name == n && matches!(apply(&p.ty, &subst), Type::Ref(_, Mode::Shared)))).collect();
                if !frozen.is_empty() && !self.all_checks {
                    self.lower_fn_frozen(name, &f, targs, &cname, &sig, &ret_c, &frozen, false)?;
                    // D-0196's `__raw` form, frozen-dual the same way.
                    if targs.is_empty() && self.raw_ret_ok(name) {
                        let open = sig.find('(').expect("a signature");
                        let raw_name = format!("{}__raw", cname);
                        let raw_sig = format!("{} {}{}", ret_c, raw_name, &sig[open..]);
                        writeln!(self.protos, "{};", raw_sig).unwrap();
                        self.uid = 0;
                        self.lower_fn_frozen(name, &f, targs, &raw_name, &raw_sig, &ret_c, &frozen, true)?;
                    }
                    return Ok(());
                }
            }
        }
        // D-0209: a function with direct reference parameters (D-0188) runs
        // their reads unchecked only when its entry can vouch for them
        // (`lower_fn_direct`); under `--all-checks`, every body is checked.
        if mask == 0 && name != "main" && !self.all_checks {
            let direct = self.direct_ref_params(&f, &subst)?;
            if !direct.is_empty() {
                self.lower_fn_direct(name, &f, targs, &cname, &sig, &ret_c, &direct, false)?;
                if targs.is_empty() && self.raw_ret_ok(name) {
                    let open = sig.find('(').expect("a signature");
                    let raw_name = format!("{}__raw", cname);
                    let raw_sig = format!("{} {}{}", ret_c, raw_name, &sig[open..]);
                    writeln!(self.protos, "{};", raw_sig).unwrap();
                    self.uid = 0;
                    self.lower_fn_direct(name, &f, targs, &raw_name, &raw_sig, &ret_c, &direct, true)?;
                }
                return Ok(());
            }
        }
        self.emit_fn_body(name, &f, targs, mask, &sig, None)?;
        // D-0196: the `__raw` form beside it, for callers that bind its
        // result objectless.
        if mask == 0 && targs.is_empty() && name != "main" && self.raw_ret_ok(name) {
            let open = sig.find('(').expect("a signature");
            let raw_sig = format!("{} {}__raw{}", ret_c, cname, &sig[open..]);
            writeln!(self.protos, "{};", raw_sig).unwrap();
            self.uid = 0;
            self.pending_raw_ret = true;
            self.emit_fn_body(name, &f, targs, 0, &raw_sig, None)?;
        }
        Ok(())
    }

    // D-0209: `NAME__dir`, its direct parameters' reads unchecked (D-0188's
    // proof: the caller formed and checked the borrow, nothing in the body
    // forms a path from it or hands it on), and `NAME__chk`, every access
    // checked through the token it came with. `NAME` runs the first when
    // this thread is alone (`cb_live_threads == 1`, read inline) -- then no
    // other thread can release a reclaimed object a parameter reaches,
    // `spec/08` §4 `[Standing-Local]` H5 -- or when `cb_direct_ok` finds
    // each direct path valid over fresh storage. Before D-0209 the direct
    // body was the only one, and a thread growing a vector released the
    // element another thread's direct parameter read
    // (`conf.direct-param-element-grown-by-other-thread`). A direct body
    // longer than `FROZEN_LINES` is not kept: the checked body alone is.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_fn_direct(&mut self, name: &str, f: &FnDecl, targs: &[Type], cname: &str, sig: &str, ret_c: &str, direct: &HashSet<String>, raw: bool) -> R<()> {
        let dir = format!("{}__dir", cname);
        let chk = format!("{}__chk", cname);
        let tail = &sig[sig.find('(').expect("a signature")..];
        let dir_sig = format!("{} {}{}", ret_c, dir, tail);
        let chk_sig = format!("{} {}{}", ret_c, chk, tail);
        let none: HashSet<String> = HashSet::new();
        let start = self.fns.len();
        self.pending_raw_ret = raw;
        self.pending_tested = true;
        self.emit_fn_body(name, f, targs, 0, &dir_sig, None)?;
        if code_lines(&self.fns[start..]) > FROZEN_LINES {
            self.fns.truncate(start);
            self.uid = 0;
            self.pending_raw_ret = raw;
            self.emit_fn_body(name, f, targs, 0, sig, Some(&none))?;
            // `NAME__dir` exists all the same, for a call site that vouches
            // for its arguments itself (`Fx::direct_local_call`): the
            // checked body, forwarded.
            let args: Vec<String> = f.params.iter().map(|p| format!("p_{}", san(&p.name))).collect();
            writeln!(self.protos, "{};", dir_sig).unwrap();
            let fwd = if ret_c == "void" { format!("    {}({});\n", cname, args.join(", ")) } else { format!("    return {}({});\n", cname, args.join(", ")) };
            write!(self.fns, "{}\n{{\n{}}}\n\n", dir_sig, fwd).unwrap();
            return Ok(());
        }
        writeln!(self.protos, "{};", dir_sig).unwrap();
        writeln!(self.protos, "{};", chk_sig).unwrap();
        self.uid = 0;
        self.pending_raw_ret = raw;
        self.emit_fn_body(name, f, targs, 0, &chk_sig, Some(&none))?;
        let subst = subst_of(&f.type_params, targs);
        let mut b = String::new();
        let mut toks = Vec::new();
        let mut args = Vec::new();
        for p in &f.params {
            let pc = format!("p_{}", san(&p.name));
            args.push(pc.clone());
            if direct.contains(&p.name) && matches!(apply(&p.ty, &subst), Type::Ref(..)) {
                toks.push(format!("{}.tok", pc));
            }
        }
        let call = |n: &str| format!("{}({})", n, args.join(", "));
        // The plain dispatcher asks only while other threads run (H5); its
        // `__arrive` twin always asks (D-0219), for a call that passes an
        // existing reference on (`Fx::vouched_args`): that one may arrive with
        // a live descendant, which the direct body would write past (H3).
        let (test, test_arrive) = if toks.is_empty() {
            ("1".to_string(), "1".to_string())
        } else {
            writeln!(b, "    uint64_t toks[{}] = {{ {} }};", toks.len(), toks.join(", ")).unwrap();
            (format!("__atomic_load_n(&cb_live_threads, __ATOMIC_ACQUIRE) == 1 || cb_direct_ok({}, toks)", toks.len()), format!("cb_direct_ok({}, toks)", toks.len()))
        };
        let pre = b.clone();
        let tail = |b: &mut String, test: &str| {
            if ret_c == "void" {
                writeln!(b, "    if ({}) {{ {}; return; }}", test, call(&dir)).unwrap();
                writeln!(b, "    {};", call(&chk)).unwrap();
            } else {
                writeln!(b, "    if ({}) return {};", test, call(&dir)).unwrap();
                writeln!(b, "    return {};", call(&chk)).unwrap();
            }
        };
        tail(&mut b, &test);
        let mut b2 = pre;
        tail(&mut b2, &test_arrive);
        // A program function's dispatcher is inlined at every call
        // (`localize`): one load, a compare and a branch there, and the
        // direct twin called itself (a recursion through the dispatcher
        // otherwise paid a call for the dispatcher and one for the twin at
        // each level).
        self.inline_sigs.insert(sig.to_string());
        write!(self.fns, "{}\n{{\n{}}}\n\n", sig, b).unwrap();
        self.emit_arrive_twin(cname, sig, &b2, true);
        Ok(())
    }

    // D-0219: `NAME__arrive`, the dispatcher `NAME` with the arrival test
    // its call sites ask for when an argument is an existing reference passed
    // on; registered so that the call sites' markers resolve to it.
    pub(super) fn emit_arrive_twin(&mut self, cname: &str, sig: &str, body: &str, inline: bool) {
        let twin = format!("{}__arrive", cname);
        let tsig = sig.replacen(&format!("{}(", cname), &format!("{}(", twin), 1);
        writeln!(self.protos, "{};", tsig).unwrap();
        if inline {
            self.inline_sigs.insert(tsig.clone());
        }
        write!(self.fns, "{}\n{{\n{}}}\n\n", tsig, body).unwrap();
        self.arrive_twins.insert(cname.to_string());
    }

    // CHG-0237: `NAME__frz`, its frozen parameters' reads unchecked, and
    // `NAME__chk`, as any body; `NAME` tests each frozen parameter at entry
    // (`cb_frozen_ok`: its path valid, not lock-derived, no exclusive path
    // overlapping it; for a vector, no object over its buffer) and runs the
    // first when all pass. While the call runs nothing can make an
    // exclusive path to what they reach: the function's other parameters
    // hold no reference but shared ones (`unstored_ref_params`), and another
    // thread's would clash with the held paths these were formed through.
    // Where a test fails the checked body runs, faulting where and as it
    // always did. A frozen body longer than `FROZEN_LINES` is not kept.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_fn_frozen(&mut self, name: &str, f: &FnDecl, targs: &[Type], cname: &str, sig: &str, ret_c: &str, frozen: &[String], raw: bool) -> R<()> {
        let frz = format!("{}__frz", cname);
        let chk = format!("{}__chk", cname);
        let tail = &sig[sig.find('(').expect("a signature")..];
        let frz_sig = format!("{} {}{}", ret_c, frz, tail);
        let chk_sig = format!("{} {}{}", ret_c, chk, tail);
        let start = self.fns.len();
        self.frozen_mode = true;
        self.pending_raw_ret = raw;
        self.pending_tested = true; // D-0219: entered only through its dispatcher's test
        let r = self.emit_fn_body(name, f, targs, 0, &frz_sig, None);
        self.frozen_mode = false;
        r?;
        if code_lines(&self.fns[start..]) > FROZEN_LINES {
            self.fns.truncate(start);
            self.uid = 0;
            self.pending_raw_ret = raw;
            return self.emit_fn_body(name, f, targs, 0, sig, None);
        }
        writeln!(self.protos, "{};", frz_sig).unwrap();
        writeln!(self.protos, "{};", chk_sig).unwrap();
        self.uid = 0;
        self.pending_raw_ret = raw;
        let none: HashSet<String> = HashSet::new();
        self.emit_fn_body(name, f, targs, 0, &chk_sig, Some(&none))?;
        let subst = subst_of(&f.type_params, targs);
        let mut tests = Vec::new();
        let mut args = Vec::new();
        for p in &f.params {
            let pc = format!("p_{}", san(&p.name));
            args.push(pc.clone());
            if !frozen.contains(&p.name) {
                continue;
            }
            let t = apply(&p.ty, &subst);
            let Type::Ref(inner, _) = &t else { continue };
            let vec_elem = match &**inner {
                Type::Named(n, a) if n == "std::Vec" && a.len() == 1 => Some(a[0].clone()),
                _ => None,
            };
            match vec_elem {
                Some(el) if self.size_align(&el)?.0 > 0 => {
                    let vc = self.ctype(inner)?;
                    let size = self.size_align(&el)?.0;
                    tests.push(format!("cb_frozen_ok({}.tok, (const void *)(({} *){}.p)->ptr, (size_t)(({} *){}.p)->len * {}u)", pc, vc, pc, vc, pc, size));
                }
                _ => tests.push(format!("cb_frozen_ok({}.tok, NULL, 0)", pc)),
            }
        }
        // D-0209: a direct parameter beside the frozen ones (its reads
        // unchecked in both bodies' direct form) is tested as
        // `lower_fn_direct` tests it.
        let direct = self.direct_ref_params(f, &subst)?;
        let dtoks: Vec<String> = f.params.iter().filter(|p| direct.contains(&p.name) && matches!(apply(&p.ty, &subst), Type::Ref(..))).map(|p| format!("p_{}.tok", san(&p.name))).collect();
        let mut b = String::new();
        let mut tests_arrive = tests.clone();
        if !dtoks.is_empty() {
            writeln!(b, "    uint64_t dtoks[{}] = {{ {} }};", dtoks.len(), dtoks.join(", ")).unwrap();
            tests.push(format!("(__atomic_load_n(&cb_live_threads, __ATOMIC_ACQUIRE) == 1 || cb_direct_ok({}, dtoks))", dtoks.len()));
            tests_arrive.push(format!("cb_direct_ok({}, dtoks)", dtoks.len()));
        }
        let call = |n: &str| format!("{}({})", n, args.join(", "));
        let pre = b.clone();
        let tail = |b: &mut String, test: &str| {
            if ret_c == "void" {
                writeln!(b, "    if ({}) {{ {}; return; }}", test, call(&frz)).unwrap();
                writeln!(b, "    {};", call(&chk)).unwrap();
            } else {
                writeln!(b, "    if ({}) return {};", test, call(&frz)).unwrap();
                writeln!(b, "    return {};", call(&chk)).unwrap();
            }
        };
        tail(&mut b, &tests.join(" && "));
        write!(self.fns, "{}\n{{\n{}}}\n\n", sig, b).unwrap();
        if !dtoks.is_empty() {
            let mut b2 = pre;
            tail(&mut b2, &tests_arrive.join(" && "));
            self.emit_arrive_twin(cname, sig, &b2, false);
        }
        Ok(())
    }

    // CHG-0237: a value that holds no buffer an element object could lie
    // over and no reference: plain data, a `String` (its bytes are never
    // reached element by element outside `std`), or a struct or enum made
    // of such. Handed to a reader without a path, reads through it need no
    // check once its container passed `cb_frozen_ok`.
    pub(super) fn flat_value(&self, t: &Type) -> bool {
        self.flat_value_in(t, 0)
    }

    pub(super) fn flat_value_in(&self, t: &Type, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        if self.plain_data(t) {
            return true;
        }
        match t {
            Type::Named(n, a) if n == "std::String" && a.is_empty() => true,
            Type::Named(n, a) => {
                if let Some(sd) = self.struct_decl(n) {
                    let sub = subst_of(&sd.type_params, a);
                    sd.fields.iter().all(|f| self.flat_value_in(&apply(&f.ty, &sub), depth + 1))
                } else if let Some(ed) = self.enum_decl(n) {
                    let sub = subst_of(&ed.type_params, a);
                    ed.variants.iter().all(|v| v.payload.as_ref().map_or(true, |p| self.flat_value_in(&apply(p, &sub), depth + 1)))
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    // A function with several reference parameters, one at least
    // exclusive, all direct in their uses (`dual_direct_params`), none of
    // them handed on, and every other parameter a plain value: then the
    // only data in flight to it at its entry is its slices', in order.
    pub(super) fn dual_eligible(&self, f: &FnDecl, subst: &HashMap<String, Type>) -> R<Option<Vec<String>>> {
        if self.all_checks {
            return Ok(None);
        }
        let Some(dual) = self.dual_direct_params(&f.params, &f.body, subst)? else { return Ok(None) };
        for p in &f.params {
            let t = apply(&p.ty, subst);
            if dual.contains(&p.name) {
                if !direct_uses(&f.body, &p.name, &|_, _| false) {
                    return Ok(None);
                }
            } else if !self.plain_data(&t) || self.has_refs(&t) || self.holds_fn(&t) {
                return Ok(None);
            }
        }
        Ok(Some(dual))
    }

    // The dual body: `NAME__fast`, its reference parameters direct (read
    // and written unchecked), and `NAME__slow`, as any body; `NAME` checks
    // the incoming paths once (`cb_paths_disjoint`: valid, over live
    // objects, pairwise disjoint where one is exclusive) and runs the
    // first when they pass. Nothing either body does can change what the
    // check found (`dual_direct_params`), so where it passes no access the
    // slow body would make could fault, and where it fails the slow body
    // runs and faults where and as it always did. A body whose fast half
    // is longer than `DUAL_LINES` is emitted once, as any body: the
    // second copy would cost more in code than the checks it saves.
    pub(super) fn lower_fn_dual(&mut self, name: &str, f: &FnDecl, targs: &[Type], cname: &str, sig: &str, ret_c: &str, dual: &[String]) -> R<()> {
        let fast = format!("{}__fast", cname);
        let slow = format!("{}__slow", cname);
        let tail = &sig[sig.find('(').expect("a signature")..];
        let fast_sig = format!("{} {}{}", ret_c, fast, tail);
        let slow_sig = format!("{} {}{}", ret_c, slow, tail);
        let set: HashSet<String> = dual.iter().cloned().collect();
        let start = self.fns.len();
        self.pending_tested = true; // D-0219: entered only through its dispatcher's test
        self.emit_fn_body(name, f, targs, 0, &fast_sig, Some(&set))?;
        // D-0192: the bare form, for calls whose arguments the call site
        // proves disjoint (`lower_dual_bare_call`): the fast half, its slices
        // pure direct (the caller holds their paths), its own element test.
        let bare_sig = format!("{} {}__bare{}", ret_c, cname, tail);
        if code_lines(&self.fns[start..]) > DUAL_LINES {
            self.fns.truncate(start);
            self.uid = 0;
            self.emit_fn_body(name, f, targs, 0, sig, None)?;
            writeln!(self.protos, "{};", bare_sig).unwrap();
            self.uid = 0;
            self.pending_bare = true;
            return self.emit_fn_body(name, f, targs, 0, &bare_sig, Some(&set));
        }
        writeln!(self.protos, "{};", bare_sig).unwrap();
        self.uid = 0;
        self.pending_bare = true;
        self.emit_fn_body(name, f, targs, 0, &bare_sig, Some(&set))?;
        writeln!(self.protos, "{};", fast_sig).unwrap();
        writeln!(self.protos, "{};", slow_sig).unwrap();
        self.emit_fn_body(name, f, targs, 0, &slow_sig, None)?;
        let subst = subst_of(&f.type_params, targs);
        // The dispatcher peeks at the slices' data in flight; exactly
        // they are (`dual_eligible`), which `cb_peek_datum_tok` asserts.
        let n_slices = f.params.iter().filter(|p| set.contains(&p.name) && matches!(apply(&p.ty, &subst), Type::Slice(..))).count();
        let mut b = String::new();
        writeln!(b, "    uint64_t toks[{}];", dual.len()).unwrap();
        writeln!(b, "    uint8_t excl[{}];", dual.len()).unwrap();
        let mut k = 0usize;
        let mut i = 0usize;
        let mut args = Vec::new();
        let mut live: Vec<String> = Vec::new();
        for p in &f.params {
            let pc = format!("p_{}", san(&p.name));
            args.push(pc.clone());
            let t = apply(&p.ty, &subst);
            let ex = matches!(t, Type::Ref(_, Mode::Exclusive) | Type::Slice(_, Mode::Exclusive)) as u8;
            match t {
                Type::Ref(..) if set.contains(&p.name) => {
                    writeln!(b, "    toks[{}] = {}.tok; excl[{}] = {};", i, pc, i, ex).unwrap();
                    i += 1;
                }
                Type::Slice(inner, _) if set.contains(&p.name) => {
                    writeln!(b, "    toks[{}] = cb_peek_datum_tok({}, {}); excl[{}] = {};", i, k, n_slices, i, ex).unwrap();
                    // A slice over cells another live object lies over (an
                    // element of a `Vec` that has one): its reads meet that
                    // object's paths, which no entry test sees (`spec/08` §4).
                    let size = self.size_align(&inner)?.0;
                    live.push(format!("cb_live_in({}.data, {}.len * {}u)", pc, pc, size));
                    k += 1;
                    i += 1;
                }
                _ => {}
            }
        }
        let call = |n: &str| format!("{}({})", n, args.join(", "));
        let test_of = |f: &str| std::iter::once(format!("{}({}, toks, excl)", f, dual.len())).chain(live.iter().map(|l| format!("!{}", l))).collect::<Vec<_>>().join(" && ");
        let pre = b.clone();
        let tail = |b: &mut String, test: &str| {
            if ret_c == "void" {
                writeln!(b, "    if ({}) {{ {}; return; }}", test, call(&fast)).unwrap();
                writeln!(b, "    {};", call(&slow)).unwrap();
            } else {
                writeln!(b, "    if ({}) return {};", test, call(&fast)).unwrap();
                writeln!(b, "    return {};", call(&slow)).unwrap();
            }
        };
        tail(&mut b, &test_of("cb_paths_disjoint"));
        write!(self.fns, "{}\n{{\n{}}}\n\n", sig, b).unwrap();
        let mut b2 = pre;
        tail(&mut b2, &test_of("cb_paths_arrive"));
        self.emit_arrive_twin(cname, sig, &b2, false);
        Ok(())
    }

    // D-0189: a function whose one reference parameter `b` is to a struct
    // with `Vec` fields of plain, non-zero-sized elements, its other
    // parameters plain values, whose body loops and uses each such field
    // only as a confined vector is used (`field_confined_uses`): the
    // fields so used, with their element types.
    pub(super) fn field_dual_eligible(&self, f: &FnDecl, subst: &HashMap<String, Type>) -> R<Option<(String, Vec<(String, Type)>, bool)>> {
        if self.all_checks {
            return Ok(None);
        }
        let mut refp: Option<(&Param, Type)> = None;
        for p in &f.params {
            let t = apply(&p.ty, subst);
            if self.has_refs(&t) || self.holds_fn(&t) {
                if refp.is_some() {
                    return Ok(None);
                }
                refp = Some((p, t));
            } else if !self.plain_data(&t) {
                return Ok(None);
            }
        }
        let Some((p, t)) = refp else { return Ok(None) };
        let Type::Ref(inner, _) = &t else { return Ok(None) };
        let Type::Named(sn, sa) = &**inner else { return Ok(None) };
        let Some(sd) = self.struct_decl(sn) else { return Ok(None) };
        let sub = subst_of(&sd.type_params, sa);
        let mut vec_fields = Vec::new();
        for fd in &sd.fields {
            if let Type::Named(vn, va) = apply(&fd.ty, &sub) {
                if vn == "std::Vec" && va.len() == 1 && self.plain_data(&va[0]) && self.size_align(&va[0])?.0 > 0 {
                    vec_fields.push((fd.name.clone(), va[0].clone()));
                }
            }
        }
        if vec_fields.is_empty() || !has_loop(&f.body) {
            return Ok(None);
        }
        let names: Vec<String> = vec_fields.iter().map(|(n, _)| n.clone()).collect();
        let Some((used, unborrowed)) = field_confined_uses(&f.body, &p.name, &names) else { return Ok(None) };
        // `b` itself unchecked in the fast half too: nothing under it is
        // borrowed, and every field it has that is not plain is a confined
        // vector (`plain_rest`).
        let plain_rest = sd.fields.iter().all(|fd| {
            let t = apply(&fd.ty, &sub);
            used.contains(&fd.name) || (self.plain_data(&t) && !self.has_refs(&t) && !self.holds_fn(&t))
        });
        let fields: Vec<(String, Type)> = vec_fields.into_iter().filter(|(n, _)| used.contains(n)).collect();
        if fields.is_empty() {
            return Ok(None);
        }
        Ok(Some((p.name.clone(), fields, unborrowed && plain_rest)))
    }

    // D-0189's dual body, as D-0188's (`lower_fn_dual`): `NAME__fast`, the
    // fields confined (no check on their elements or lengths, their calls
    // `nc_variant`s), and `NAME__slow`, as any body. The dispatcher runs the
    // first when `b`'s path is valid, over a live object and not
    // lock-derived (`cb_paths_disjoint`) and no element of the fields has a
    // live object (`cb_live_in`): then, as for a confined vector, nothing
    // but `b` reaches the fields or their elements (it is the function's
    // one reference; another thread's step cannot change that, `spec/08`
    // §4), and nothing the body does gives an element an object or a path.
    pub(super) fn lower_fn_field_dual(&mut self, name: &str, f: &FnDecl, targs: &[Type], cname: &str, sig: &str, ret_c: &str, b: &str, fields: &[(String, Type)], direct_b: bool) -> R<()> {
        let fast = format!("{}__fast", cname);
        let slow = format!("{}__slow", cname);
        let tail = &sig[sig.find('(').expect("a signature")..];
        let fast_sig = format!("{} {}{}", ret_c, fast, tail);
        let slow_sig = format!("{} {}{}", ret_c, slow, tail);
        let subst = subst_of(&f.type_params, targs);
        let bp = f.params.iter().find(|p| p.name == b).expect("the reference parameter");
        let Type::Ref(inner, mode) = apply(&bp.ty, &subst) else { return internal("a field dual body without a reference") };
        let sc = self.ctype(&inner)?;
        let pc = format!("p_{}", san(b));
        let mut fc = HashMap::new();
        let mut live = Vec::new();
        for (fname, elem) in fields {
            let vc = format!("(*({} *){}.p).{}", sc, pc, san(fname));
            let size = self.size_align(elem)?.0;
            live.push(format!("!cb_live_in((void *)({}).ptr, ({}).len * {}u)", vc, vc, size));
            fc.insert((b.to_string(), fname.clone()), (vc, elem.clone()));
        }
        let start = self.fns.len();
        self.pending_field_confined = Some(fc);
        // `b`'s other fields, all plain and never borrowed, unchecked too:
        // `b` is the one path to them, valid by the dispatcher's test.
        let direct: HashSet<String> = if direct_b { std::iter::once(b.to_string()).collect() } else { HashSet::new() };
        self.pending_tested = true; // D-0219: entered only through its dispatcher's test
        self.emit_fn_body(name, f, targs, 0, &fast_sig, if direct_b { Some(&direct) } else { None })?;
        self.pending_field_confined = None;
        if code_lines(&self.fns[start..]) > DUAL_LINES {
            self.fns.truncate(start);
            self.uid = 0;
            return self.emit_fn_body(name, f, targs, 0, sig, None);
        }
        writeln!(self.protos, "{};", fast_sig).unwrap();
        writeln!(self.protos, "{};", slow_sig).unwrap();
        self.emit_fn_body(name, f, targs, 0, &slow_sig, None)?;
        let args: Vec<String> = f.params.iter().map(|p| format!("p_{}", san(&p.name))).collect();
        let call = |n: &str| format!("{}({})", n, args.join(", "));
        let mut d = String::new();
        writeln!(d, "    uint64_t toks[1] = {{ {}.tok }};", pc).unwrap();
        writeln!(d, "    uint8_t excl[1] = {{ {} }};", (mode == Mode::Exclusive) as u8).unwrap();
        let pre = d.clone();
        let tail = |d: &mut String, test: &str| {
            if ret_c == "void" {
                writeln!(d, "    if ({}) {{ {}; return; }}", test, call(&fast)).unwrap();
                writeln!(d, "    {};", call(&slow)).unwrap();
            } else {
                writeln!(d, "    if ({}) return {};", test, call(&fast)).unwrap();
                writeln!(d, "    return {};", call(&slow)).unwrap();
            }
        };
        tail(&mut d, &format!("cb_paths_disjoint(1, toks, excl) && {}", live.join(" && ")));
        write!(self.fns, "{}\n{{\n{}}}\n\n", sig, d).unwrap();
        let mut d2 = pre;
        tail(&mut d2, &format!("cb_paths_arrive(1, toks, excl) && {}", live.join(" && ")));
        self.emit_arrive_twin(cname, sig, &d2, false);
        Ok(())
    }

    // The body of `name` at `targs` under the signature `sig`; `direct`
    // overrides which reference parameters are direct (the dual body's
    // fast half).
    pub(super) fn emit_fn_body(&mut self, name: &str, f: &FnDecl, targs: &[Type], mask: u64, sig: &str, direct: Option<&HashSet<String>>) -> R<()> {
        if std::env::var("COBC_DEBUG_FN").is_ok() {
            eprintln!("FN {} raw={}", name, self.pending_raw_ret);
        }
        let cfn = sig.split('(').next().unwrap_or(sig).rsplit(' ').next().unwrap_or(sig);
        let mut h: u32 = 0x811c9dc5;
        for b in cfn.bytes() {
            h = (h ^ b as u32).wrapping_mul(0x0100_0193);
        }
        self.closure_base = (h as u64) << 24;
        let subst = subst_of(&f.type_params, targs);
        let ret = apply(&f.ret, &subst);
        let param_tys: Vec<(String, Type)> = f.params.iter().map(|p| (p.name.clone(), apply(&p.ty, &subst))).collect();
        let fn_bound: HashSet<String> = bound_names(&f.body).into_iter().chain(f.params.iter().map(|p| p.name.clone())).collect();
        let pinned = pinned_names(&f.body, &|segs: &[String], j: usize| self.pure_direct_ref_arg(segs, j, &fn_bound));
        let owned_vecs: Vec<String> = f.params.iter().filter(|p| matches!(&p.ty, Type::Named(n, a) if n == "std::Vec" && a.len() == 1)).map(|p| p.name.clone()).collect();
        // D-0274: the bootstrap form keeps every local's object and check, so
        // these analyses are not made. `pinned_names` stays: the base
        // discharge of plain locals (`spec/14` §6).
        let (confined_names, objectless_names, elem_ref_ids, quiet_ids) = if self.boot(4) {
            (HashSet::new(), HashSet::new(), HashSet::new(), HashSet::new())
        } else {
            let names: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
            let confined_names = self.confined_vecs(&f.body, &names, &owned_vecs);
            let objectless_names = self.objectless_strings(&f.body, &names);
            let elem_ref_ids = self.elem_ref_lets(&f.body, &names);
            let quiet_ids = self.quiet_locals(&f.body, &names, &elem_ref_ids);
            (confined_names, objectless_names, elem_ref_ids, quiet_ids)
        };
        // CHG-0368: the stage-4 analyses' answers, for comparing the code
        // generator written in CobaltC with this one (`COBC_DUMP_ANALYSES`).
        // At any level: in the bootstrap form they are made only to be shown.
        if std::env::var_os("COBC_DUMP_ANALYSES").is_some() && self.probing == 0 {
            let names: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
            let c = self.confined_vecs(&f.body, &names, &owned_vecs);
            let o = self.objectless_strings(&f.body, &names);
            let e = self.elem_ref_lets(&f.body, &names);
            let q = self.quiet_locals(&f.body, &names, &e);
            dump_analyses(name, &f.body, &c, &o, &e, &q);
            // The stage-1 analyses, past their `--all-checks` gate.
            let mut cf: Vec<String> = confined_fields(&f.body, &names).into_iter().map(|(h, fl)| format!("{}.{}", h, fl)).collect();
            cf.sort();
            let mut rc: Vec<String> = read_confined_vecs(&f.body, &names).into_iter().collect();
            rc.sort();
            let pd: Vec<String> = (0..f.params.len()).filter(|&j| self.pure_direct_in(name, j, &std::cell::RefCell::new(Vec::new()))).map(|j| j.to_string()).collect();
            let dual = match self.dual_direct_params(&f.params, &f.body, &subst) {
                Ok(Some(d)) => d.join(" "),
                Ok(None) => "-".to_string(),
                Err(_) => "!".to_string(),
            };
            let mut dr: Vec<String> = self.direct_refs_core(&f.params, &f.body, &subst, true).map(|d| d.into_iter().collect()).unwrap_or_default();
            dr.sort();
            eprintln!("ANALYSES {} confined_fields [{}]", name, cf.join(" "));
            eprintln!("ANALYSES {} read_confined [{}]", name, rc.join(" "));
            eprintln!("ANALYSES {} pure_direct [{}]", name, pd.join(" "));
            eprintln!("ANALYSES {} dual [{}]", name, dual);
            eprintln!("ANALYSES {} direct [{}]", name, dr.join(" "));
        }
        let all_checks = self.all_checks;
        let mut fx = Fx { g: self, subst, scopes: vec![HashMap::new()], out: String::new(), ret: ret.clone(), cleanup: Vec::new(), loops: Vec::new(), loop_broken: Vec::new(), ind: 1, self_ref: None, assigned: assigned_names(&f.body), rebind_frames: HashMap::new(), holder_roots: HashMap::new(), no_overflow: HashSet::new(), proven_next: false, pinned, confined_names, confined_fields: if all_checks { HashSet::new() } else { confined_fields(&f.body, &f.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>()) }, read_confined: if all_checks { HashSet::new() } else { read_confined_vecs(&f.body, &f.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>()) }, objectless_names, cur_let: None, raw_ret: false, raw_tail: None, raw_sinks: HashSet::new(), raw_substring: None, raw_return_path: false, raw_call: None, objectless_c: HashSet::new(), moved_flag: HashMap::new(), bare_trims: HashSet::new(), elem_ref_ids, quiet_ids, quiet_roots: HashSet::new(), drop_lines: Vec::new(), confined: HashSet::new(), confined_params: HashMap::new(), probes: HashMap::new(), kept: Vec::new(), dollar: Vec::new(), place_write: false,
            fused_scrut: None, map_acc: None, map_acc_used: false, compound_elem: false, datum_tok: None, datum_tok_out: None, map_get_acc: None, map_get_acc_out: None, elem_mode: None, temp_root: false, frame_line: None, vec_index_exprs: HashMap::new(), index_sugar: HashSet::new(), propagate_scrutinee: false, elem_alias: HashMap::new(), alias_kept: Vec::new(), stmt_match: false, branch_scope: false, objectless: false, inert_c: HashSet::new(), obj_temps: HashSet::new(), notes: 0, root_frame: 0, direct_params: HashSet::new(), entry_tested: false, direct_refs: HashSet::new(), direct_live: HashMap::new(), fast_half: false, elem_reader: false, frozen_on: false, pred_body: false, field_confined: HashMap::new(), pure_slices: HashSet::new(), pure_views: HashSet::new(), unstored: HashMap::new(), unstored_params: HashSet::new(), held_locals: HashSet::new(), held_c: HashSet::new(), unminted_binders: HashMap::new() };
        fx.push_frame();
        fx.root_frame = fx.g.nscope;
        fx.fast_half = direct.is_some();
        fx.entry_tested = std::mem::replace(&mut fx.g.pending_tested, false);
        let bare = std::mem::replace(&mut fx.g.pending_bare, false);
        fx.raw_ret = std::mem::replace(&mut fx.g.pending_raw_ret, false);
        if bare {
            // The bare form: no dispatcher has tested its slices (`cb_live_in`
            // runs at entry).
            fx.fast_half = false;
        }
        if let Some(fc) = fx.g.pending_field_confined.take() {
            fx.field_confined = fc;
        }
        fx.direct_params = match direct {
            Some(d) => d.clone(),
            None => fx.g.direct_ref_params(&f, &fx.subst)?,
        };
        fx.unstored_params = fx.g.unstored_ref_params(&f, &fx.subst, &fx.direct_params)?;
        fx.frozen_on = fx.g.frozen_mode;
        fx.held_locals = if fx.g.boot(4) { HashSet::new() } else { held_locals(&f.body) };
        fx.pure_slices = f.params.iter().enumerate().filter(|(i, p)| matches!(p.ty, Type::Slice(..)) && fx.g.pure_direct(name, *i)).map(|(_, p)| p.name.clone()).collect();
        if bare {
            // ... and none of its slices is received: the caller holds each
            // one's path (`lower_dual_bare_call`).
            let sub = fx.subst.clone();
            let slices: Vec<String> = f.params.iter().filter(|p| matches!(apply(&p.ty, &sub), Type::Slice(..))).map(|p| p.name.clone()).collect();
            fx.pure_slices.extend(slices);
        }
        fx.pure_views = f.params.iter().enumerate().filter(|(i, _)| fx.g.pure_view(name, *i)).map(|(_, p)| p.name.clone()).collect();
        for (j, p) in f.params.iter().enumerate() {
            if mask & (1 << j) != 0 {
                // A confined argument (`Fx::bind_confined`): no binding
                // object; its uses reach the vector through `p.p`.
                let pty = fx.sub(&p.ty);
                let Type::Ref(inner, _) = &pty else { return internal("confined parameter of a non-reference type") };
                let Type::Named(_, a) = &**inner else { return internal("confined parameter of a non-Vec type") };
                let vec_c = fx.g.ctype(inner)?;
                let pc = format!("p_{}", san(&p.name));
                fx.confined_params.insert(p.name.clone(), (format!("(*({} *){}.p)", vec_c, pc), format!("{}.tok", pc), a[0].clone()));
            } else {
                fx.bind_param(p)?;
            }
        }
        let v = fx.lower_block_body(&f.body, Some(&ret), None)?;
        if matches!(v.ty, Type::Never) {
            fx.emit("__builtin_unreachable();");
        } else {
            fx.emit_return(v)?;
        }
        while !fx.cleanup.is_empty() {
            fx.end_diverged();
        }
        let prologue = fx.notes_finish();
        // D-0274: in the bootstrap form no text pass elides anything.
        let body = if fx.g.boot(1) {
            resolve_scopes(&fx.out)
        } else {
            elide_implied_checks(&drop_dead_locations(&resolve_scopes(&fold_zero_tokens(&drop_quiet_checks(&fx.out, &fx.quiet_roots))), self.line_directives))
        };
        // D-0077: a call too deep for the stack faults here, at the call.
        // A body that calls nothing (`calls_nothing`) cannot go deeper:
        // its caller's check, and the floor's margin, cover its frame.
        let check = if !self.boot(1) && calls_nothing(&body) { "" } else { "    cb_stack_check();\n" };
        let (entry, exit) = self.overaligned_params(&param_tys)?;
        write!(self.fns, "{}\n{{\n{}{}{}{}{}}}\n\n", sig, entry, check, prologue, body, exit).unwrap();
        Ok(())
    }

    // D-0255: a parameter whose type is aligned beyond 16 bytes (`struct S
    // align(64)`) is copied at entry into an aligned local, which the body
    // uses under the parameter's name (a `#define` to the end of the body).
    // The Windows x64 convention passes such a parameter as a pointer to a
    // copy the caller makes, and MinGW-w64's GCC does not align that copy,
    // so `&p` was misaligned; a local is aligned. Elsewhere it is one copy
    // more. The signature is unchanged. Returns the body's first and last
    // lines.
    pub(super) fn overaligned_params(&mut self, params: &[(String, Type)]) -> R<(String, String)> {
        let (mut entry, mut exit) = (String::new(), String::new());
        for (name, ty) in params {
            if self.size_align(ty)?.1 <= 16 {
                continue;
            }
            let pc = format!("p_{}", san(name));
            let ct = self.param_ctype(ty)?;
            entry.push_str(&format!("    {} {}__al = {};\n#define {} {}__al\n", ct, pc, pc, pc, pc));
            exit.push_str(&format!("#undef {}\n", pc));
        }
        Ok((entry, exit))
    }
}

// CHG-0368: one line per analysis of a function body being lowered, its
// answers sorted: names as they are, nodes as `e:LINE:COL` (an expression) or
// `a:LINE:COL` (a match arm, by its body).
fn dump_analyses(name: &str, body: &Block, confined: &HashSet<String>, objectless: &HashSet<usize>, elem_refs: &HashSet<usize>, quiet: &HashSet<usize>) {
    // A program line as the code generator written in CobaltC numbers it
    // (its own file's, not after the prelude's).
    let n = prelude::line_count();
    let ln = |l: usize| if l > n { l - n } else { l };
    let mut pos: HashMap<usize, String> = HashMap::new();
    visit_exprs(body, &mut |e| {
        pos.insert(e as *const Expr as usize, format!("e:{}:{}", ln(e.line), e.col));
        if let ExprKind::Match(_, arms) = &e.kind {
            for a in arms {
                pos.insert(a as *const Arm as usize, format!("a:{}:{}", ln(a.body.line), a.body.col));
            }
        }
    });
    let nodes = |s: &HashSet<usize>| {
        let mut v: Vec<String> = s.iter().map(|k| match pos.get(k) {
            Some(p) => p.clone(),
            // `objectless_strings` tags a binding it also reports compared
            // (`|1`) or indexed (`|2`).
            // A candidate moved away is also there as `!id` (`+m`).
            None if pos.contains_key(&!*k) => format!("{}+m", pos[&!*k]),
            None => match pos.get(&(k & !3)) {
                Some(p) => format!("{}+{}", p, k & 3),
                None => if *k == 0 { "0".to_string() } else { format!("?{:x}", k) },
            },
        }).collect();
        v.sort();
        v.join(" ")
    };
    let mut c: Vec<&String> = confined.iter().collect();
    c.sort();
    let held = {
        let mut h: Vec<String> = held_locals(body).into_iter().collect();
        h.sort();
        h.join(" ")
    };
    eprintln!("ANALYSES {} confined [{}]", name, c.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" "));
    eprintln!("ANALYSES {} objectless [{}]", name, nodes(objectless));
    eprintln!("ANALYSES {} elem_refs [{}]", name, nodes(elem_refs));
    eprintln!("ANALYSES {} quiet [{}]", name, nodes(quiet));
    eprintln!("ANALYSES {} held [{}]", name, held);
}
