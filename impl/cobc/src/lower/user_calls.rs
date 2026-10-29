// Fx: calls of the program's own functions.

use super::*;

// The outcome of an argument's direct path (`lower_user_arg_direct`).
enum DirectArg {
    Arg(V),
    Diverged,
}

impl<'g, 'a> Fx<'g, 'a> {
    pub(super) fn lower_user_call(&mut self, name: &str, f: &FnDecl, targs: &[Type], args: &[Expr], expected: Option<&Type>, line: usize, access: Option<bool>) -> R<V> {
        let site = self.pc_site.take();
        if in_prelude(f) {
            self.at(line);
        }
        // D-0274: in the bootstrap form every call takes the general path,
        // but `e[k]`, whose check is element k's (`lower_vec_index_pre`).
        if self.g.boot(3) {
            if let Some(v) = self.lower_vec_index_pre(name, f, args, line, access)? {
                return Ok(v);
            }
        }
        if !self.g.boot(3) {
            // `String::append(s, x)` with a `str`: the prelude's body is
            // `append_native(s, x)`, which hands both on to `append_str`. The
            // call goes to that helper directly: the body's frame and its
            // bindings are not observable when `x` is a value.
            if name == "std::String::append" && in_prelude(f) && args.len() == 2 {
                let t = match targs {
                    [t] => self.sub(t),
                    _ => self.probe_type(&args[1], None)?,
                };
                // Only a `str`, which is a value: for a view or a `&String`, the
                // body's binding of `x` holds its path while the helper checks
                // `s`, and `String::append(&mut s, String::as_view(&s))` must
                // clash there.
                let h = match &t {
                    Type::Str => "std::String::append_str",
                    _ => "",
                };
                if let Some(hf) = self.g.items.fns.get(h).cloned() {
                    return self.lower_user_call(h, &hf, &[], args, expected, line, access);
                }
            }
            if let Some(v) = self.lower_substring(name, f, args, line)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_map_fresh_key(name, f, args, line)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_push_fresh(name, f, args, line)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_set_insert_fresh(name, f, args, line)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_channel_send_fresh(name, f, args, line)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_box_new(name, f, targs, args, expected, line)? {
                return Ok(v);
            }
            // `Box::get(b)` / `Box::get_mut(b)` on an unminted arm binder: the
            // native body (`native_vec_index`'s Box case) inline, its read of
            // the box's pointer through the binder's base and projection.
            if (name == "std::Box::get" || name == "std::Box::get_mut") && in_prelude(f) && args.len() == 1 {
                if let Some(bind) = path_name(&args[0]).and_then(|n| self.lookup(n)) {
                    if let Some((base, projs)) = self.unminted_binders.get(&bind.c).cloned() {
                        let Type::Ref(bt, _) = &bind.ty else { return internal("a box binder") };
                        let Type::Named(_, ba) = &**bt else { return internal("a box binder's type") };
                        let [elem] = ba.as_slice() else { return internal("a box's type") };
                        let bc = self.g.ctype(bt)?;
                        let tid = self.tid(elem)?;
                        let mode = if name == "std::Box::get" { Mode::Shared } else { Mode::Exclusive };
                        let md = if mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                        let mut all = projs.clone();
                        all.push("{ CB_FIELD, 0u }".to_string());
                        let a = self.g.fresh("a");
                        let r = self.g.fresh("r");
                        if !base.is_empty() {
                            if mode == Mode::Exclusive {
                                // D-0222: `_ = &mut *b;`, the box exclusively.
                                if projs.is_empty() {
                                    self.emit(&format!("cb_borrow_check({}, NULL, 0, CB_EXCLUSIVE, CB_NOLOC, 0);", base));
                                } else {
                                    self.emit(&format!("cb_borrow_check({}, (cb_proj[]){{ {} }}, {}, CB_EXCLUSIVE, CB_NOLOC, 0);", base, projs.join(", "), projs.len()));
                                }
                            }
                            self.emit(&format!("cb_read({}, (cb_proj[]){{ {} }}, {}, CB_NOLOC, 0);", base, all.join(", "), all.len()));
                        }
                        self.emit(&format!("void *{} = (void *)(({} *){})->ptr;", a, bc, bind.c));
                        self.emit(&format!("cb_ref {} = (cb_ref){{ {}, cb_elem_borrow({}, {}u, {}) }};", r, a, a, tid, md));
                        self.emit("cb_recv_ref();");
                        return Ok(V { c: r, ty: Type::Ref(Box::new(elem.clone()), mode), obj: None });
                    }
                }
            }
            if let Some(v) = self.lower_confined_call(name, f, args, line)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_push_checked_once(name, f, args)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_vec_checked_once(name, f, args, line)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_dual_bare_call(name, f, args, line)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_pred_literal(name, f, args, line)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_from_fn_literal(name, f, args, line)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_sort_by_literal(name, f, args)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_vec_index_pre(name, f, args, line, access)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_vec_len_pre(name, f, args)? {
                return Ok(v);
            }
            if let Some(v) = self.lower_as_view(name, f, args, line)? {
                return Ok(v);
            }
        }
        // Confined vectors passed to confining parameters
        // (`Gen::confining_table`): the call goes to the variant compiled
        // for them, and each is passed as its address and path, with no
        // borrow formed (`bind_confined`: nothing could fail on it).
        let conf = self.g.confining.get(name).cloned().unwrap_or_default();
        let mut mask = 0u64;
        let mut fresh = 0u64;
        for (j, a) in args.iter().enumerate().take(64) {
            if conf.get(j) == Some(&true) && self.confined_local(a).is_some() {
                mask |= 1 << j;
            } else if conf.get(j) == Some(&true) && matches!(f.params[j].ty, Type::Ref(_, Mode::Shared)) && self.fresh_shared_vec_arg(a, j, args)? {
                mask |= 1 << j;
                fresh |= 1 << j;
            }
        }
        let mut m: HashMap<String, Type> = HashMap::new();
        for (p, t) in f.type_params.iter().zip(targs.iter()) {
            m.insert(p.clone(), self.sub(t));
        }
        if let Some(exp) = expected {
            unify(&f.ret, exp, &f.type_params, &mut m);
        }
        let mut argv = Vec::new();
        // D-0222: `HashMap::get_mut(&mut x, k)`, `x` a local checked
        // exclusively here (`cb_borrow_unminted`): its body's own exclusive
        // check repeats this one (`native_map_op`'s `_x` variants).
        let mut map_excl_checked = false;
        let mut weakened: Vec<String> = Vec::new();
        // D-0079: an unsuffixed literal whose parameter type is not yet
        // known is lowered after the other arguments (it has no effects),
        // at the type they and the expected type fixed.
        let mut deferred: Vec<usize> = Vec::new();
        let mut order: Vec<usize> = (0..f.params.len().min(args.len())).collect();
        let mut k = 0;
        while k < order.len() {
            let j = order[k];
            k += 1;
            let (p, a) = (&f.params[j], &args[j]);
            if !deferred.contains(&j) && is_bare_num_literal(a) && !resolved(&apply(&p.ty, &m), &f.type_params) && mask & (1 << j) == 0 {
                deferred.push(j);
                order.push(j);
                argv.push(V { c: String::new(), ty: Type::Void, obj: None });
                continue;
            }
            // A payload-less variant (`None`) fixes no type parameter
            // either: it waits for the other arguments, deferred literals
            // included (`Option::unwrap_or(None, 7)`), having no effects.
            // So does a generic fn item named as a value, which needs the
            // type its parameter gets (`twice(id, 10)`).
            let bare_variant = matches!(&a.kind, ExprKind::Path(segs, t) if t.is_empty() && self.lookup(&segs[0]).is_none()
                && (self.variant_of(segs).is_some() || self.g.items.fns.get(&segs.join("::")).is_some_and(|g| !g.type_params.is_empty())));
            if bare_variant && !resolved(&apply(&p.ty, &m), &f.type_params) && mask & (1 << j) == 0 && deferred.iter().filter(|&&d| d == j).count() < 2 {
                let first = !deferred.contains(&j);
                deferred.push(j);
                order.push(j);
                if first {
                    argv.push(V { c: String::new(), ty: Type::Void, obj: None });
                }
                continue;
            }
            // A shared borrow formed here (`fresh_shared_vec_arg`): its
            // path, checked as it is formed, is the parameter's.
            if fresh & (1 << j) != 0 {
                let v = self.lower_expr(a, None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                unify(&p.ty, &v.ty, &f.type_params, &mut m);
                put_arg(&mut argv, j, v);
                continue;
            }
            if mask & (1 << j) != 0 {
                let (vec, tok, elem) = self.confined_local(a).expect("confined argument");
                let Type::Ref(_, mode) = &p.ty else { return internal("confining parameter of a non-reference type") };
                let ty = Type::Ref(Box::new(Type::Named("std::Vec".to_string(), vec![elem])), mode.clone());
                unify(&p.ty, &ty, &f.type_params, &mut m);
                put_arg(&mut argv, j, V { c: format!("((cb_ref){{ (void *)&({}), {} }})", vec, tok), ty, obj: None });
                continue;
            }
            let mut hint = apply(&p.ty, &m);
            // A type parameter this argument cannot fix by itself
            // (`f(Vec::new(), g)`): the later arguments' types fix it
            // first, as coby's inference does. They are probed, not
            // lowered, so the evaluation order is unchanged.
            if !resolved(&hint, &f.type_params) && !is_bare_num_literal(a) {
                for l in (j + 1)..f.params.len().min(args.len()) {
                    if !probe_safe(&args[l]) || mask & (1 << l) != 0 {
                        continue;
                    }
                    let lh = apply(&f.params[l].ty, &m);
                    let lexp = if resolved(&lh, &f.type_params) { Some(lh) } else { None };
                    let Ok(lt) = self.probe_with(|fx| fx.lower_expr(&args[l], lexp.as_ref())) else { continue };
                    let lt = match (&f.params[l].ty, &lt) {
                        (Type::Fn(..), Type::Closure(id)) => match self.g.closures.get(id) {
                            Some(info) => Type::Fn(info.params.clone(), Box::new(info.ret.clone())),
                            None => lt,
                        },
                        _ => lt,
                    };
                    unify(&f.params[l].ty, &lt, &f.type_params, &mut m);
                }
                hint = apply(&p.ty, &m);
            }
            let exp = if resolved(&hint, &f.type_params) { Some(hint) } else { None };
            match self.lower_user_arg_direct(name, f, args, j, a, p, &argv, &mut m, &mut map_excl_checked)? {
                Some(DirectArg::Arg(v)) => {
                    // CHG-0356: the argument's type fixes the callee's type
                    // parameters whichever direct path lowered it (the
                    // reader element `&m[j]` of `HashSet::len` did not).
                    unify(&p.ty, &v.ty, &f.type_params, &mut m);
                    put_arg(&mut argv, j, v);
                    continue;
                }
                Some(DirectArg::Diverged) => return Ok(never()),
                None => {}
            }
            let is_index = j == 1 && (name == "std::Vec::index_shared" || name == "std::Vec::index_exclusive");
            let v = match argv.first() {
                Some(r0) if is_index => {
                    let vt = match &r0.ty {
                        Type::Ref(t, _) => (**t).clone(),
                        t => t.clone(),
                    };
                    let vc = self.g.ctype(&vt)?;
                    let len = format!("((({} *)({}).p))->len", vc, r0.c);
                    self.lower_index_arg(len, a)?
                }
                _ => {
                    // D-0286: a by-value resource argument of a decided
                    // program goes over with no object where it can: an
                    // inert literal built with none, a call's result taken
                    // with none (`arm_inert`), an objectless local moved
                    // with none (`read_place`).
                    let by_value = self.g.decided && !matches!(p.ty, Type::Ref(..) | Type::Slice(..) | Type::Fn(..)) && exp.as_ref().map_or(true, |t| self.is_res(t));
                    if by_value {
                        self.objectless = self.inert_expr(a)?;
                        self.arm_inert(a, false);
                        self.raw_return_path = matches!(strip_parens(a).kind, ExprKind::Path(..));
                    }
                    let v = self.lower_expr(a, exp.as_ref());
                    if by_value {
                        self.objectless = false;
                        self.inert_result = None;
                        self.raw_return_path = false;
                    }
                    v?
                }
            };
            if matches!(v.ty, Type::Never) {
                return Ok(never());
            }
            // A closure fixes a `fn(…) : R` parameter's type parameters by
            // its signature (D-0073).
            let unify_ty = match (&p.ty, &v.ty) {
                (Type::Fn(..), Type::Closure(id)) => match self.g.closures.get(id) {
                    Some(info) => Type::Fn(info.params.clone(), Box::new(info.ret.clone())),
                    None => v.ty.clone(),
                },
                _ => v.ty.clone(),
            };
            unify(&p.ty, &unify_ty, &f.type_params, &mut m);
            // Then it moves into a callable box of that type, as it would
            // had the type been known before (`[Callable-Closure]`).
            let v = match (&p.ty, &v.ty) {
                (Type::Fn(..), Type::Closure(_)) => {
                    let dst = apply(&p.ty, &m);
                    self.box_closure(v, dst, a.line)?
                }
                _ => v,
            };
            let v_ty_before = v.ty.clone();
            let t = self.into_temp(v, a.line)?;
            let t = self.weaken_arg(t, &p.ty, a.line)?;
            // CHG-0358: a reference binding passed whole (`f(r)`, its token
            // the binding's own) to a function whose result holds a
            // reference: the result may be that very path, which the call's
            // statement then owns (`cb_recv_ref`) and ends with it, ending
            // the binding's path too. A path derived for the call is the
            // result's instead. Only a result that can hold a reference to
            // the argument's own referent type can be that path
            // (`String::view(s, …)`'s `StringView` is a slice, a path of its
            // own).
            let ret_now = apply(&f.ret, &m);
            let t = match (&t.ty, &strip_parens(a).kind) {
                (Type::Ref(inner, md), ExprKind::Path(segs, targs)) if segs.len() == 1 && targs.is_empty() && v_ty_before == t.ty && self.may_carry_ref_to(&ret_now, inner, &f.type_params) => {
                    let loc = self.loc(a.line);
                    let mdc = if *md == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                    let tok = self.g.fresh("tok");
                    self.emit(&format!("uint64_t {} = cb_borrow(({}).tok, NULL, 0, {}, {});", tok, t.c, mdc, loc));
                    let r = self.g.fresh("r");
                    self.emit(&format!("cb_ref {} = ((cb_ref){{ ({}).p, {} }});", r, t.c, tok));
                    V { c: r, ty: Type::Ref(inner.clone(), md.clone()), obj: None }
                }
                _ => t,
            };
            // A slice formed for the call (written `&e[i .. j]` there, or
            // reborrowed shared for it, D-0049) is held only by the callee's
            // parameter, as a reference argument is: it ends at the call's
            // return, as in coby, not with the statement.
            let formed_here = matches!(strip_parens(a).kind, ExprKind::SliceOf(..));
            let reborrowed = matches!((&v_ty_before, &t.ty), (Type::Slice(_, Mode::Exclusive), Type::Slice(_, Mode::Shared)));
            if matches!(t.ty, Type::Slice(..)) && (formed_here || reborrowed) {
                if let Some(o) = &t.obj {
                    weakened.push(o.clone());
                }
            }
            put_arg(&mut argv, j, t);
        }
        self.at_after_arg_calls(f, args, line);
        if let Some(pcs) = site.and_then(|s| coby::tier::point_checks_at(self.g.items, s)) {
            self.emit_point_checks(&pcs, &argv, line)?;
        }
        let inst: Vec<Type> = f.type_params.iter().map(|p| m.get(p).cloned()).collect::<Option<_>>().unwrap_or_default();
        if inst.len() != f.type_params.len() {
            return internal(&format!("cannot infer type arguments of `{}`", name));
        }
        if let Some(write) = access {
            if let Some(v) = self.native_elem_access(name, f, &inst, &argv, write, line)? {
                return Ok(v);
            }
        }
        let cname = self.g.request_variant(name, inst.clone(), mask)?;
        // D-0209: the direct twin itself where every direct argument is the
        // caller's own local (`direct_local_call`).
        let cname = if self.direct_local_call(name, f, &inst, mask, args)? { format!("{}__dir", cname) } else { cname };
        // D-0219: a call whose reference arguments the dispatcher's cheap test
        // cannot vouch for asks its `__arrive` twin, which tests each as it
        // arrives (`spec/08` §4 H3).
        let cname = if !cname.ends_with("__dir") && !self.vouched_args(f, &inst, args) { format!("{}{}", cname, ARRIVE_MARK) } else { cname };
        let fsub = subst_of(&f.type_params, &inst);
        // Resources and reference data go in flight in argument order; a
        // slice for a pure direct parameter stays (`Gen::pure_direct`).
        for (j, (p, t)) in f.params.iter().zip(argv.iter()).enumerate() {
            let pty = apply(&p.ty, &fsub);
            if self.is_res(&pty) {
                self.give_arg(t, &format!("`{}` argument {} line {}", name, j, line))?;
            } else if matches!(pty, Type::Slice(..)) && self.g.pure_direct(name, j) {
            } else if self.g.pure_view(name, j) {
                // Not sent (`Gen::pure_view`): a temporary view ends after
                // the call, where the callee would have ended it.
                if let Some(o) = &t.obj {
                    weakened.push(o.clone());
                }
            } else if self.has_refs(&pty) && !matches!(pty, Type::Ref(..)) {
                let tid = self.tid(&pty)?;
                self.emit(&format!("cb_send_datum(&{}, {}u);", t.c, tid));
            }
        }
        let ret = apply(&f.ret, &fsub);
        if self.map_acc == Some(args.as_ptr() as usize) && name == "std::HashMap::entry" && self.native_map_passthrough(name, f, args, 0) {
            self.map_acc = None;
            // The `_acc` variant: the value's address, its element access
            // made (`lower_block_body`'s compound pair).
            let Type::Ref(vt, _) = &ret else { return internal("entry's result") };
            let vc = self.g.ctype(vt)?;
            let pv = self.g.fresh("pv");
            self.emit(&format!("{} *{} = {}_acc({});", vc, pv, cname, argv.iter().map(|v| v.c.clone()).collect::<Vec<_>>().join(", ")));
            self.map_acc_used = true;
            return Ok(V { c: pv, ty: ret, obj: None });
        }
        let mut call = format!("{}{}({})", cname, if map_excl_checked { "_x" } else { "" }, argv.iter().map(|v| v.c.clone()).collect::<Vec<_>>().join(", "));
        // CHG-0237: a vector passed fresh (`fresh_shared_vec_arg`) is read
        // unchecked by the confined variant only where no object lies over
        // its elements: an element held by reference has an object of its
        // own, which the borrow of the vector does not meet, and a read of
        // that element must (`[Read-Conflict]`). Where one does, the
        // ordinary variant runs and checks.
        if fresh != 0 {
            let slow = self.g.request_variant(name, inst.clone(), mask & !fresh)?;
            let mut tests = Vec::new();
            for (j, (p, t)) in f.params.iter().zip(argv.iter()).enumerate() {
                if fresh & (1 << j) == 0 {
                    continue;
                }
                let pty = apply(&p.ty, &fsub);
                let Type::Ref(inner, _) = &pty else { continue };
                let Type::Named(_, ea) = &**inner else { continue };
                let size = self.g.size_align(&ea[0])?.0;
                let vc = self.g.ctype(inner)?;
                tests.push(format!("cb_live_in((void *)(({} *)({}).p)->ptr, (({} *)({}).p)->len * {}u)", vc, t.c, vc, t.c, size));
            }
            let args_c = argv.iter().map(|v| v.c.clone()).collect::<Vec<_>>().join(", ");
            call = format!("(({}) ? {}({}) : {}({}))", tests.join(" || "), slow, args_c, cname, args_c);
        }
        let acc_v = match &ret {
            Type::Named(_, oa) if self.map_get_acc == Some(args.as_ptr() as usize) && matches!(name, "std::HashMap::get_mut" | "std::HashMap::get") && one_ref_option(&ret) && self.native_map_passthrough(name, f, args, 0) => match oa.as_slice() {
                [Type::Ref(vt, _)] if self.g.plain_data(vt) => Some((**vt).clone()),
                _ => None,
            },
            _ => None,
        };
        let v = if let Some(vt) = acc_v {
            // The `_acc` variant: the value's address, its element access
            // made; the arm's read and write through it need no check
            // (`direct_refs`), and no path is formed.
            self.map_get_acc = None;
            let vc = self.g.ctype(&vt)?;
            let pv = self.g.fresh("pv");
            let x = if map_excl_checked { "_x" } else { "" };
            self.emit(&format!("{} *{} = {}_acc{}({});", vc, pv, cname, x, argv.iter().map(|v| v.c.clone()).collect::<Vec<_>>().join(", ")));
            let t = self.g.fresh("t");
            let oc = self.g.ctype(&ret)?;
            self.emit(&format!("{} {};", oc, t));
            self.emit(&format!("{}.tag = {} ? 0u : 1u;", t, pv));
            self.emit(&format!("{}.u.v0 = (void *){};", t, pv));
            self.map_get_acc_out = Some(pv);
            V { c: t, ty: ret, obj: None }
        } else if self.datum_tok == Some(args.as_ptr() as usize) && one_ref_option(&ret) {
            self.datum_tok = None;
            let t = self.g.fresh("t");
            let c = self.g.ctype(&ret)?;
            self.emit(&format!("{} {} = {};", c, t, call));
            let tk = self.g.fresh("tk");
            self.emit(&format!("uint64_t {} = cb_recv_datum_tok();", tk));
            self.datum_tok_out = Some(tk);
            V { c: t, ty: ret, obj: None }
        } else {
            // D-0286: the consumer took this call's result inert (`arm_inert`).
            if let Some((_, last)) = self.inert_result.filter(|(a, _)| *a == args.as_ptr() as usize) {
                self.inert_result = None;
                self.inert_now = Some(last);
            }
            let v = self.finish_call(call, ret, line);
            self.inert_now = None;
            v?
        };
        // A slice reborrowed for a shared parameter (D-0049), or a
        // temporary view for a pure view parameter, served only the call:
        // it ends now, as a reference argument's path does.
        for o in weakened {
            self.emit(&format!("cb_absorb({});", o));
        }
        Ok(v)
    }

    // An argument's direct paths: a borrow or view passed with no path
    // minted, or with its checks only, where the parameter allows it
    // (`Gen::pure_direct`, `pure_view`, `reader_param`, the native
    // operations' passthroughs). `None`: the general path lowers it.
    #[allow(clippy::too_many_arguments)]
    fn lower_user_arg_direct(&mut self, name: &str, f: &FnDecl, args: &[Expr], j: usize, a: &Expr, p: &Param, argv: &[V], m: &mut HashMap<String, Type>, map_excl_checked: &mut bool) -> R<Option<DirectArg>> {
        if self.g.boot(3) {
            return Ok(None);
        }
        // D-0258: `&NAME` of a static, `&K` of an addressed constant, as an
        // argument: the global's address with token 0 (`statics.rs`), as
        // `lower_expr` gives it.
        if let ExprKind::Borrow(_, inner) = &strip_parens(a).kind {
            if let Some(v) = self.global_borrow(inner)? {
                // The argument's type fixes the callee's type parameters,
                // as an ordinary argument's does below.
                unify(&p.ty, &v.ty, &f.type_params, m);
                return Ok(Some(DirectArg::Arg(v)));
            }
        }
        // `&e[lo .. hi]` formed for a pure direct slice parameter: the
        // borrow's checks only (`lower_slice_of_as`).
        if let ExprKind::SliceOf(smode, sbase, slo, shi) = &strip_parens(a).kind {
            let sa = strip_parens(a);
            let temp = self.g.items.temp_borrows.lock().unwrap().contains(&(sa as *const Expr as usize));
            if matches!(&p.ty, Type::Slice(_, pm) if pm == smode) && self.view_op(sa).is_none() && !temp && self.g.pure_direct(name, j) {
                let v = self.lower_slice_of_as(smode, sbase, slo, shi, sa.line, false)?;
                return Ok(Some(DirectArg::Arg(v)));
            }
        }
        // A literal viewed (`StringView::of(L)`, D-0134) for a pure view
        // parameter: a bare view of the literal's bytes (no path, no
        // tracked slot, no temporary object). The callee keeps nothing
        // and reads only the bytes, and no path can ever write a `str`'s.
        if let Some(lit) = literal_view_arg(a) {
            if self.g.pure_view(name, j) {
                let lv = self.lower_expr(lit, Some(&Type::Str))?;
                let vt = Type::Named("std::StringView".to_string(), vec![]);
                let svc = self.g.ctype(&vt)?;
                let t = self.g.fresh("t");
                self.emit(&format!("{} {};", svc, t));
                self.emit(&format!("{}.bytes.src = (void *)({}).p;", t, lv.c));
                self.emit(&format!("{}.bytes.data = (uint8_t *)({}).p;", t, lv.c));
                self.emit(&format!("{}.bytes.len = ({}).n;", t, lv.c));
                return Ok(Some(DirectArg::Arg(V { c: t, ty: vt, obj: None })));
            }
        }
        // `String::as_view(x)` for a pure view parameter, the call's
        // views and references all shared, and nothing else among the
        // arguments evaluated (`inert_view_arg`): the view's borrows'
        // checks only, and a bare view (no path, no tracked slot, no
        // temporary object), as `&e[lo .. hi]` for a pure direct slice
        // above. While the call runs no exclusive path to the text can
        // be formed: no argument forms one, and the callee has only
        // shared references and reads the view only through its bytes.
        // D-0200: also an exclusive reference parameter to a value made
        // only of scalars (`File::write_text(&mut f, …)`), its argument
        // `&mut p` evaluated before the view: no text lies in such a
        // value, so the view's bytes and its referent cannot overlap,
        // and through it the callee reaches nothing else. (An overlap
        // would fault at the callee's first access through either,
        // D-0107, which an unminted view could not show.)
        if let Some(x) = as_view_arg(a) {
            // CHG-0237: the text of a frozen parameter's field (`&p.f…`):
            // no path can clash with it while the call runs, whatever
            // the other arguments are; the callee keeps nothing of it.
            let frozen = matches!(&strip_parens(x).kind, ExprKind::Borrow(Mode::Shared, pl) if self.frozen_root(pl));
            if frozen && self.g.pure_view(name, j) {
                let v = self.lower_as_view_unminted(x, a.line)?;
                return Ok(Some(DirectArg::Arg(v)));
            }
            let excl_before = |i: usize, o: &Expr| i < j && matches!(&strip_parens(o).kind, ExprKind::Borrow(Mode::Exclusive, x) if path_name(x).is_some());
            if self.g.pure_view(name, j) && self.view_safe_params(f) && args.iter().enumerate().all(|(i, o)| i == j || inert_view_arg(o) || excl_before(i, o)) {
                let v = self.lower_as_view_unminted(x, a.line)?;
                return Ok(Some(DirectArg::Arg(v)));
            }
        }
        // A view place for a pure view parameter (`Gen::pure_view`):
        // its read's check, then its bytes, as a slice place's below.
        if is_place_expr(a) && root_bound(a, &|n| self.lookup(n).is_some()) && self.g.pure_view(name, j) {
            let pl = self.lower_place(a)?;
            if pl.raw.is_none() {
                if let Some(base) = &pl.base {
                    let loc = self.loc(a.line);
                    self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&pl.projs), loc));
                }
                let t = self.g.fresh("t");
                let c = self.g.ctype(&pl.ty)?;
                self.emit(&format!("{} {} = {};", c, t, pl.c));
                return Ok(Some(DirectArg::Arg(V { c: t, ty: pl.ty.clone(), obj: None })));
            }
        }
        // A slice place for a pure direct slice parameter: its read's
        // check, then its bytes; the path its slot holds stays where it
        // is, held for the call (`Gen::pure_direct`).
        if matches!(p.ty, Type::Slice(..)) && is_place_expr(a) && root_bound(a, &|n| self.lookup(n).is_some()) && self.g.pure_direct(name, j) {
            let pl = self.lower_place(a)?;
            if pl.raw.is_none() && matches!(pl.ty, Type::Slice(..)) {
                if let Some(base) = &pl.base {
                    let loc = self.loc(a.line);
                    self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&pl.projs), loc));
                }
                let t = self.g.fresh("t");
                let c = self.g.ctype(&pl.ty)?;
                self.emit(&format!("{} {} = {};", c, t, pl.c));
                return Ok(Some(DirectArg::Arg(V { c: t, ty: pl.ty.clone(), obj: None })));
            }
        }
        // A native map operation's map (or key) reference
        // (`native_map_op`): its body uses the path only for checks
        // through it, keeps none of it and returns nothing derived from
        // it. `&x` / `&mut x` of a local is checked as the borrow would
        // be (`cb_borrow_unminted`) and passed with `x`'s own root
        // path; no path is minted for the call (an unheld path no check
        // counts, D-0018, ending with the call).
        if let ExprKind::Borrow(bmode, place) = &a.kind {
            if self.native_map_passthrough(name, f, args, j) || self.append_passthrough(name, f, args, j) || self.reader_passthrough(name, f, args, j) {
                if let Some(bind) = path_name(place).and_then(|n| self.lookup(n)) {
                    // An objectless `String` for a reader: its address,
                    // token 0 (every check on it proven, `bind_value`).
                    if *bmode == Mode::Shared && self.objectless_c.contains(&bind.c) && self.reader_passthrough(name, f, args, j) {
                        self.objectless_use(&bind.c, a.line);
                        let ty = Type::Ref(Box::new(bind.ty.clone()), Mode::Shared);
                        unify(&p.ty, &ty, &f.type_params, m);
                        return Ok(Some(DirectArg::Arg(V { c: format!("((cb_ref){{ (void *)&({}), 0 }})", bind.c), ty, obj: None })));
                    }
                    if matches!(bind.kind, BindKind::Local) && !bind.root.is_empty() && !self.direct_refs.contains(&bind.c) && !self.unstored.contains_key(&bind.c) && !matches!(bind.ty, Type::Ref(..)) {
                        let loc = self.loc(a.line);
                        let md = if *bmode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                        self.emit(&format!("cb_borrow_unminted({}, NULL, 0, {}, {});", bind.root, md, loc));
                        // The key a local's value or a borrow of one: its
                        // evaluation, after this, forms no path into `x`.
                        let key_inert = |e: &Expr| match &strip_parens(e).kind {
                            ExprKind::Path(segs, t) => segs.len() == 1 && t.is_empty(),
                            ExprKind::Borrow(_, pl) => matches!(&strip_parens(pl).kind, ExprKind::Path(segs, t) if segs.len() == 1 && t.is_empty()),
                            _ => false,
                        };
                        if j == 0 && *bmode == Mode::Exclusive && name == "std::HashMap::get_mut" && args.get(1).map_or(false, key_inert) {
                            *map_excl_checked = true;
                        }
                        let ty = Type::Ref(Box::new(bind.ty.clone()), bmode.clone());
                        unify(&p.ty, &ty, &f.type_params, m);
                        return Ok(Some(DirectArg::Arg(V { c: format!("((cb_ref){{ (void *)&({}), {} }})", bind.c, bind.root), ty, obj: None })));
                    }
                }
            }
        }
        // `Box::get[_mut](b)`, `b` an unminted binder (`unminted_binders`),
        // for a direct parameter: the box's pointer read (checked
        // through the binder's base, if it has one), and the contents'
        // element access in the borrow's mode (`cb_elem_access`: checked
        // where an object lies over them -- a reference kept elsewhere
        // clashes there, where `cb_elem_borrow` would have faulted -- and
        // otherwise nothing to see), then the address alone: the callee
        // reads no path (tree-design.md).
        if let ExprKind::Call(gc, gargs) = &strip_parens(a).kind {
            let gname = match &gc.kind { ExprKind::Path(segs, _) => segs.join("::"), _ => String::new() };
            if (gname == "std::Box::get" || gname == "std::Box::get_mut") && gargs.len() == 1 && matches!(p.ty, Type::Ref(..)) && self.g.pure_direct(name, j) && !sibling_names_root(args, j, &gargs[0]) {
                if let Some(bind) = path_name(&gargs[0]).and_then(|n| self.lookup(n)) {
                    if let Some((base, bprojs)) = self.unminted_binders.get(&bind.c).cloned() {
                        if let Type::Ref(bt, _) = &bind.ty {
                            if let Type::Named(_, ba) = &**bt {
                                if let [elem] = ba.as_slice() {
                                    let bc = self.g.ctype(bt)?;
                                    let tid = self.tid(elem)?;
                                    let md = if gname == "std::Box::get" { "CB_SHARED" } else { "CB_EXCLUSIVE" };
                                    if !base.is_empty() {
                                        let mut all = bprojs.clone();
                                        all.push("{ CB_FIELD, 0u }".to_string());
                                        self.emit(&format!("cb_read({}, (cb_proj[]){{ {} }}, {}, CB_NOLOC, 0);", base, all.join(", "), all.len()));
                                    }
                                    let ad = self.g.fresh("ad");
                                    let loc = self.loc(a.line);
                                    // The call's location, as `lower_user_call` sets it for a
                                    // `std` function: where its element borrow would fault.
                                    self.at(a.line);
                                    self.emit(&format!("void *{} = (void *)(({} *){})->ptr;", ad, bc, bind.c));
                                    let esz = self.g.size_align(elem)?.0;
                                    self.emit(&format!("cb_elem_access({}, {}u, {}u, {}, 0, {});", ad, esz, tid, md, loc));
                                    let mode = if gname == "std::Box::get" { Mode::Shared } else { Mode::Exclusive };
                                    let ty = Type::Ref(Box::new(elem.clone()), mode);
                                    unify(&p.ty, &ty, &f.type_params, m);
                                    return Ok(Some(DirectArg::Arg(V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty, obj: None })));
                                }
                            }
                        }
                    }
                }
            }
        }
        // CHG-0237: `&x.f…` -- fields only, down to a shared reference
        // parameter that is never rebound (`frozen_root`) -- for a reader
        // parameter: the field's address, no path. The parameter's own
        // path, held for the call, excludes every clashing path; the
        // reader keeps nothing, so nothing outlives the call.
        if let ExprKind::Borrow(Mode::Shared, place) = &a.kind {
            if matches!(p.ty, Type::Ref(_, Mode::Shared)) && self.frozen_root(place) && self.g.reader_param(name, j) && {
                let t = self.probe_type(place, None)?;
                self.g.flat_value(&t)
            } {
                let pl = self.lower_place(place)?;
                let ad = self.g.fresh("ad");
                self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                let ty = Type::Ref(Box::new(pl.ty.clone()), Mode::Shared);
                unify(&p.ty, &ty, &f.type_params, m);
                return Ok(Some(DirectArg::Arg(V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty, obj: None })));
            }
        }
        // `&v[i]`/`&mut v[i]`, a plain element, for a pure direct
        // parameter: `v`'s checked read and the bounds fault as the
        // index call makes them, then the element checked in the
        // borrow's mode (`cb_elem_access`: where an object lies over
        // it; elsewhere the borrow would establish an ephemeral object
        // whose one path no check counts, D-0018, D-0023), and its
        // address. No element path is minted or retired.
        if let ExprKind::Borrow(bmode, place) = &a.kind {
            // `v[i]` as the index call it stands for.
            let rewritten = match &strip_parens(place).kind {
                ExprKind::Index(..) if matches!(p.ty, Type::Ref(..)) && (self.g.pure_direct(name, j) || (*bmode == Mode::Shared && self.g.reader_param(name, j))) => self.vec_index_call(strip_parens(place), false)?,
                _ => None,
            };
            let place_x: &Expr = match &rewritten {
                Some(x) => &**x,
                None => &**place,
            };
            if let ExprKind::Deref(inner) = &strip_parens(place_x).kind {
                let plain_elem = matches!(p.ty, Type::Ref(..)) && self.g.pure_direct(name, j) && !sibling_names_root(args, j, place) && {
                    let et = self.probe_type(place_x, None)?;
                    !self.is_res(&et) && !self.has_refs(&et) && !self.holds_fn(&et) && !matches!(et, Type::Closure(_))
                };
                // D-0192: `&v[i]`, a resource element holding no
                // reference (`String`), for a reader parameter: the same
                // checks, the element read in place for the call (the
                // reader keeps nothing and writes nothing), no other
                // argument naming the vector.
                let reader_elem = !plain_elem && *bmode == Mode::Shared && matches!(p.ty, Type::Ref(_, Mode::Shared)) && self.g.reader_param(name, j) && {
                    let et = self.probe_type(place_x, None)?;
                    let mut root = &**place;
                    loop {
                        match &strip_parens(root).kind {
                            ExprKind::Field(x, _) | ExprKind::Index(x, _) | ExprKind::Deref(x) => root = x,
                            _ => break,
                        }
                    }
                    let rn = path_name(strip_parens(root)).map(|r| r.to_string());
                    self.is_res(&et) && !self.has_refs(&et) && !self.holds_fn(&et) && !matches!(et, Type::Closure(_) | Type::Ref(..))
                        && rn.map_or(false, |r| !args.iter().enumerate().any(|(k, b)| k != j && mentions(b, &r)))
                };
                if plain_elem || reader_elem {
                    let m = if *bmode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                    let saved = self.elem_mode.replace(m);
                    self.elem_reader = reader_elem;
                    let r = self.elem_access_place(inner, *bmode == Mode::Exclusive, a.line);
                    self.elem_reader = false;
                    self.elem_mode = saved;
                    if let Some(pl) = r? {
                        if matches!(pl.ty, Type::Never) {
                            return Ok(Some(DirectArg::Diverged));
                        }
                        if pl.base.is_none() && pl.raw.is_none() {
                            let ad = self.g.fresh("ad");
                            self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                            let ty = Type::Ref(Box::new(pl.ty.clone()), bmode.clone());
                            return Ok(Some(DirectArg::Arg(V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty, obj: None })));
                        }
                        // The index call ran in full (not the prelude's
                        // own, say): its reference is the place's base.
                        if let Some(base) = pl.base.clone() {
                            let loc = self.loc(a.line);
                            self.emit(&format!("cb_borrow_unminted({}, {}, {}, {});", base, Self::projs_args(&pl.projs), m, loc));
                            let ad = self.g.fresh("ad");
                            self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                            let ty = Type::Ref(Box::new(pl.ty.clone()), bmode.clone());
                            return Ok(Some(DirectArg::Arg(V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty, obj: None })));
                        }
                        return internal("an element place with neither base nor plain access");
                    }
                }
            }
        }
        // `&p`/`&mut p` for a pure direct parameter (`Gen::pure_direct`):
        // the borrow's check, and its address with no path minted --
        // nothing reads it, and an unheld path no check counts (D-0018).
        // A place rooted in a binding only: a temporary (`&mk()`) lowered
        // here and then left to the general path below was evaluated
        // twice, its effects and its destruction with it.
        // D-0189: the key of `Vec::contains`, `index_of` or `binary_search`
        // for an integer or `bool` element type, whose native body reads
        // it once and keeps nothing (`native_vec_index_of` and its kin;
        // their fallback, the prelude's body, likewise): a pure direct
        // parameter in all but name. A temporary key (`&40`,
        // `&narrow<i64>(q)`), which nothing else can reach, is passed
        // as the address of its value, no object made; a place, as
        // below, after its borrow's check.
        // The key's type is the vector's element type (argument 0's):
        // a literal key is lowered at it, never at its own default.
        let key_elem = match argv.first().map(|r0| &r0.ty) {
            Some(Type::Ref(vt, _)) => match &**vt {
                Type::Named(n, ea) if n == "std::Vec" && ea.len() == 1 && matches!(ea[0], Type::Int(_) | Type::Bool) => Some(ea[0].clone()),
                _ => None,
            },
            _ => None,
        };
        let native_key = j == 1
            && in_prelude(f)
            && matches!(name, "std::Vec::contains" | "std::Vec::index_of" | "std::Vec::binary_search")
            && matches!(&a.kind, ExprKind::Borrow(Mode::Shared, _))
            && key_elem.is_some();
        if native_key {
            let ExprKind::Borrow(_, inner) = &a.kind else { unreachable!() };
            if !is_place_expr(inner) {
                let v = self.lower_expr(inner, key_elem.as_ref())?;
                if v.ty != *key_elem.as_ref().unwrap() {
                    return internal("a native search key of another type than the elements");
                }
                if matches!(v.ty, Type::Never) {
                    return Ok(Some(DirectArg::Diverged));
                }
                let kc = self.g.ctype(&v.ty)?;
                let k = self.g.fresh("key");
                self.emit(&format!("{} {} = {};", kc, k, v.c));
                let ty = Type::Ref(Box::new(v.ty.clone()), Mode::Shared);
                return Ok(Some(DirectArg::Arg(V { c: format!("((cb_ref){{ (void *)&{}, 0 }})", k), ty, obj: None })));
            }
        }
        if let ExprKind::Borrow(bmode, place) = &a.kind {
            if matches!(p.ty, Type::Ref(..)) && (native_key || self.g.pure_direct(name, j)) && is_place_expr(place) && root_bound(place, &|n| self.lookup(n).is_some()) && !sibling_names_root(args, j, place) {
                let pl = self.lower_place(place)?;
                // A local `pinned_names` left unpinned for this very
                // borrow: no object, every check on it proven
                // (`unchecked`); its address is the argument.
                if pl.base.is_none() && pl.raw.is_none() && pl.is_binding {
                    let ad = self.g.fresh("ad");
                    self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                    let ty = Type::Ref(Box::new(pl.ty.clone()), bmode.clone());
                    return Ok(Some(DirectArg::Arg(V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty, obj: None })));
                }
                if let (Some(base), None) = (pl.base.clone(), &pl.raw) {
                    let loc = self.loc(a.line);
                    let m = if *bmode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                    self.emit(&format!("cb_borrow_unminted({}, {}, {}, {});", base, Self::projs_args(&pl.projs), m, loc));
                    // The address taken now, as the borrow would be.
                    let ad = self.g.fresh("ad");
                    self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                    let ty = Type::Ref(Box::new(pl.ty.clone()), bmode.clone());
                    return Ok(Some(DirectArg::Arg(V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty, obj: None })));
                }
            }
        }
        Ok(None)
    }
}


// CHG-0342: whether another argument of the call (`args`, the argument at
// `j` excepted) names the binding at the root of `place` (`x` of `x`,
// `x.f`, `v[i]`, `(*r).f`), closures' bodies included. A vouched argument
// (no path minted, token 0: `lower_direct_arg`'s pure direct forms) is
// checked where its borrow is made, and the direct body makes no check at
// its entry; a later argument that lends the same local to a thread
// (`g(&mut x, spawn(f, &x))`) would then form a lasting path the body
// never sees. With another argument naming the root, the borrow is minted
// and the dispatcher's entry test (`cb_direct_ok`) sees it. A root that is
// not a one-segment name is not vouched for.
pub(super) fn sibling_names_root(args: &[Expr], j: usize, place: &Expr) -> bool {
    let mut x = strip_parens(place);
    loop {
        match &x.kind {
            ExprKind::Field(a, _) | ExprKind::Index(a, _) | ExprKind::Deref(a) => x = strip_parens(a),
            ExprKind::Path(segs, _) if segs.len() == 1 => return args.iter().enumerate().any(|(k, b)| k != j && mentions(b, &segs[0])),
            _ => return true,
        }
    }
}

impl<'g, 'a> Fx<'g, 'a> {
    // D-0283: a call's point checks, its arguments lowered: each access the
    // callee's summary declares through an argument a point check names is
    // checked here, as if made at the call, through the argument's path.
    fn emit_point_checks(&mut self, pcs: &[coby::tier::PointCheck], argv: &[V], line: usize) -> R<()> {
        let loc = self.loc(line);
        // The path of each argument the callee will hold.
        let tok_of = |v: &V| -> Option<String> {
            match &v.ty {
                Type::Ref(..) => Some(format!("({}).tok", v.c)),
                Type::Slice(..) => Some(format!("cb_load_ref(&({}).src)", v.c)),
                Type::Named(n, a) if n == "std::StringView" && a.is_empty() => Some(format!("cb_load_ref(&({}).bytes.src)", v.c)),
                _ => None,
            }
        };
        // A struct argument (D-0290: a frame-held aggregate): the paths of
        // the references kept in its fields, each checked as a read, as the
        // run does in `coby`.
        let mut kept: Vec<Vec<String>> = Vec::new();
        for v in argv {
            let mut out = Vec::new();
            let lvalue = !v.c.is_empty() && v.c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
            if lvalue {
                self.kept_ref_tokens(&v.c, &v.ty, &mut out, 0);
            }
            kept.push(out);
        }
        let mut held: Vec<String> = argv.iter().filter_map(tok_of).collect();
        held.extend(kept.iter().flatten().cloned());
        let held_c = if held.is_empty() { "NULL, 0".to_string() } else { format!("(uint64_t[]){{ {} }}, {}", held.join(", "), held.len()) };
        for pc in pcs {
            let Some(v) = argv.get(pc.arg) else { continue };
            let Some(tok) = tok_of(v) else {
                let msg = pc.message();
                for tok in &kept[pc.arg] {
                    self.emit(&format!("cb_point_check({}, {}, NULL, 0, 0u, (const uint8_t *)\"{}\", {}u, {});", held_c, tok, super::passes::c_escape(&msg), msg.len(), loc));
                }
                continue;
            };
            let mut projs = Vec::new();
            if let Type::Ref(inner, _) = &v.ty {
                let mut t = (**inner).clone();
                for fname in &pc.fields {
                    let Type::Named(n, _) = &t else { break };
                    let Some(sd) = self.g.items.structs.get(n) else { break };
                    if !sd.type_params.is_empty() {
                        break;
                    }
                    let Some(i) = sd.fields.iter().position(|fd| &fd.name == fname) else { break };
                    projs.push(format!("{{ CB_FIELD, {}u }}", i));
                    t = sd.fields[i].ty.clone();
                }
            }
            let ps = if projs.is_empty() { "NULL, 0".to_string() } else { format!("(cb_proj[]){{ {} }}, {}", projs.join(", "), projs.len()) };
            let excl = if pc.mode == Mode::Exclusive && matches!(v.ty, Type::Ref(..) | Type::Slice(..)) { 1 } else { 0 };
            let msg = pc.message();
            self.emit(&format!("cb_point_check({}, {}, {}, {}u, (const uint8_t *)\"{}\", {}u, {});", held_c, tok, ps, excl, super::passes::c_escape(&msg), msg.len(), loc));
        }
        Ok(())
    }

    // The paths of the references a struct value `c` of type `t` keeps in
    // its fields, nested structs' included.
    fn kept_ref_tokens(&self, c: &str, t: &Type, out: &mut Vec<String>, depth: usize) {
        if depth > 8 {
            return;
        }
        let Type::Named(n, args) = t else { return };
        if n.starts_with("std::") {
            return;
        }
        let Some(sd) = self.g.struct_decl(n) else { return };
        let sub = subst_of(&sd.type_params, args);
        for fd in &sd.fields {
            let fc = format!("{}.{}", c, san(&fd.name));
            match apply(&fd.ty, &sub) {
                Type::Ref(..) => out.push(format!("cb_load_ref(&{})", fc)),
                Type::Slice(..) => out.push(format!("cb_load_ref(&({}).src)", fc)),
                Type::Named(m, a) if m == "std::StringView" && a.is_empty() => out.push(format!("cb_load_ref(&({}).bytes.src)", fc)),
                ft @ Type::Named(..) => self.kept_ref_tokens(&fc, &ft, out, depth + 1),
                _ => {}
            }
        }
    }
}
