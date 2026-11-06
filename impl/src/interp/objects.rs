// Objects: creation, removal, references they hold, and type substitution.

use super::*;

impl Interp {
    // D-0258: the object of the static `name` (its qualified path), made on
    // the first use with its declared first value: `Atomic<T>` holds its
    // lane, `AtomicBool` an `Atomic<u8>` of 0 or 1. Owned by the main
    // thread (every thread's shared borrow of it clashes with nothing; it
    // is never destroyed).
    pub(super) fn static_object(&mut self, name: &str) -> Result<Option<u64>, Flow> {
        if let Some(&o) = self.static_objs.get(name) {
            return Ok(Some(o));
        }
        let Some(decl) = self.items.statics.get(name).cloned() else { return Ok(None) };
        let lane_value = |e: &Expr| -> Option<(bool, i128)> {
            match &e.kind {
                ExprKind::IntLit(v, _) => Some((false, *v as i128)),
                ExprKind::BoolLit(b) => Some((true, if *b { 1 } else { 0 })),
                ExprKind::Unary(UnOp::Neg, inner) => match &inner.kind {
                    ExprKind::IntLit(v, _) => Some((false, -(*v as i128))),
                    _ => None,
                },
                _ => None,
            }
        };
        let Some((_, v)) = lane_value(&decl.init) else { return Err(Flow::Fault("diag.type-mismatch".into())) };
        let value = match &decl.ty {
            Type::Named(n, a) if n == "std::Atomic" => match a.first() {
                Some(Type::Int(it)) => Value::Struct(vec![Value::Int(*it, v)]),
                _ => return Err(Flow::Fault("diag.type-mismatch".into())),
            },
            Type::Named(n, _) if n == "std::AtomicBool" => Value::Struct(vec![Value::Struct(vec![Value::Int(IntTy::U8, v)])]),
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        let saved = self.current_thread;
        self.current_thread = 0;
        let o = self.new_object(decl.ty.clone(), value);
        self.current_thread = saved;
        self.static_objs.insert(name.to_string(), o);
        Ok(Some(o))
    }

    pub fn new_object(&mut self, ty: Type, value: Value) -> u64 {
        let is_resource = self.is_resource(&ty);
        let id = self.fresh_id();
        let owner_thread = self.current_thread;
        if Self::value_has_ref(&value) {
            self.index_refs(id, &value);
        }
        self.objects.insert(id, Object { ty, value, is_resource, owner_thread });
        id
    }

    // Every removal of an object goes through here so the holder index
    // never outgrows `objects`.
    pub(super) fn remove_object(&mut self, id: &u64) -> Option<Object> {
        if let Some(ts) = self.targets_by_holder.remove(id) {
            for t in ts {
                if let Some(hs) = self.holders_by_target.get_mut(&t) {
                    hs.remove(id);
                    if hs.is_empty() {
                        self.holders_by_target.remove(&t);
                    }
                }
            }
        }
        self.objects.remove(id)
    }

    // Records `holder` under every object a reference or guard in `v`
    // targets.
    pub(super) fn index_refs(&mut self, holder: u64, v: &Value) {
        let mut targets = Vec::new();
        Self::collect_ref_targets(v, &mut targets);
        for t in targets {
            if self.holders_by_target.entry(t).or_default().insert(holder) {
                self.targets_by_holder.entry(holder).or_default().push(t);
            }
        }
    }

    pub(super) fn collect_ref_targets(v: &Value, out: &mut Vec<u64>) {
        match v {
            Value::Ref { obj, .. } | Value::Guard { obj, .. } => out.push(*obj),
            Value::Struct(fields) => fields.iter().for_each(|f| Self::collect_ref_targets(f, out)),
            Value::Array(items) => items.iter().for_each(|e| Self::collect_ref_targets(e, out)),
            Value::Enum(_, payload) => Self::collect_ref_targets(payload, out),
            _ => {}
        }
    }

    pub(super) fn value_has_ref(v: &Value) -> bool {
        match v {
            Value::Ref { .. } | Value::Guard { .. } => true,
            Value::Struct(fields) => fields.iter().any(Self::value_has_ref),
            Value::Array(items) => items.iter().any(Self::value_has_ref),
            Value::Enum(_, payload) => Self::value_has_ref(payload),
            _ => false,
        }
    }

    // D-0049: whether a value of a resource type owns anything now: a
    // `None` of an `Option<Box<T>>` does not, so it may be written over
    // (`live-resource-at`, spec/05). A type declared `resource` or with a
    // destructor always does, as do handles, guards and closures.
    pub(super) fn value_owns(&self, v: &Value, ty: &Type) -> bool {
        if !self.is_resource(ty) {
            return false;
        }
        match (ty, v) {
            (Type::Named(name, args), _) if !crate::typecheck::type_is_owner(&self.items, name) => {
                if let (Some(sd), Value::Struct(fs)) = (self.items.structs.get(name), v) {
                    let sub = self.subst_map(&sd.type_params, args);
                    return sd.fields.iter().zip(fs.iter()).any(|(f, fv)| self.value_owns(fv, &self.apply_subst(&f.ty, &sub)));
                }
                if let (Some(ed), Value::Enum(vi, payload)) = (self.items.enums.get(name), v) {
                    let sub = self.subst_map(&ed.type_params, args);
                    return match ed.variants.get(*vi).and_then(|var| var.payload.as_ref()) {
                        Some(p) => self.value_owns(payload, &self.apply_subst(p, &sub)),
                        None => false,
                    };
                }
                true
            }
            (Type::Array(inner, _), Value::Array(es)) => es.iter().any(|e| self.value_owns(e, inner)),
            _ => true,
        }
    }

    pub fn is_resource(&self, ty: &Type) -> bool {
        match ty {
            Type::Handle(_) | Type::Mutex(_) | Type::Guard(_) => true,
            Type::Array(inner, _) => self.is_resource(inner),
            Type::Named(name, args) => {
                if let Some(s) = self.items.structs.get(name) {
                    if s.resource {
                        return true;
                    }
                    if s.type_params.is_empty() {
                        return s.fields.iter().any(|f| self.is_resource(&f.ty));
                    }
                    s.fields.iter().any(|f| self.is_resource(&self.apply_subst_pos(&f.ty, &s.type_params, args)))
                } else if let Some(e) = self.items.enums.get(name) {
                    if e.resource {
                        return true;
                    }
                    e.variants.iter().any(|v| {
                        v.payload
                            .as_ref()
                            .map(|p| if e.type_params.is_empty() { self.is_resource(p) } else { self.is_resource(&self.apply_subst_pos(p, &e.type_params, args)) })
                            .unwrap_or(false)
                    })
                } else {
                    false
                }
            }
            Type::Closure(_) => false, // resolved specially at construction time; see closure objects
            _ => false,
        }
    }

    pub(super) fn subst_map(&self, params: &[String], args: &[Type]) -> HashMap<String, Type> {
        params.iter().cloned().zip(args.iter().cloned()).collect()
    }

    // `apply_subst` with the substitution given as the declaration's type
    // parameters and their arguments, side by side: what a field or payload
    // type of a generic struct or enum needs, without building a map.
    pub(super) fn apply_subst_pos(&self, ty: &Type, params: &[String], args: &[Type]) -> Type {
        match ty {
            Type::Named(name, targs) => {
                if targs.is_empty() {
                    if let Some(i) = params.iter().position(|p| p == name) {
                        if let Some(t) = args.get(i) {
                            return t.clone();
                        }
                    }
                }
                Type::Named(name.clone(), targs.iter().map(|a| self.apply_subst_pos(a, params, args)).collect())
            }
            Type::Ref(inner, m) => Type::Ref(Box::new(self.apply_subst_pos(inner, params, args)), m.clone()),
            Type::Slice(inner, m) => Type::Slice(Box::new(self.apply_subst_pos(inner, params, args)), m.clone()),
            Type::Rawptr(inner) => Type::Rawptr(Box::new(self.apply_subst_pos(inner, params, args))),
            Type::Array(inner, n) => Type::Array(Box::new(self.apply_subst_pos(inner, params, args)), *n),
            Type::Fn(ps, r) => Type::Fn(
                ps.iter().map(|p| self.apply_subst_pos(p, params, args)).collect(),
                Box::new(self.apply_subst_pos(r, params, args)),
            ),
            Type::Handle(inner) => Type::Handle(Box::new(self.apply_subst_pos(inner, params, args))),
            Type::Mutex(inner) => Type::Mutex(Box::new(self.apply_subst_pos(inner, params, args))),
            Type::Guard(inner) => Type::Guard(Box::new(self.apply_subst_pos(inner, params, args))),
            other => other.clone(),
        }
    }

    pub fn apply_subst(&self, ty: &Type, subst: &HashMap<String, Type>) -> Type {
        match ty {
            Type::Named(name, args) => {
                if args.is_empty() {
                    if let Some(t) = subst.get(name) {
                        return t.clone();
                    }
                }
                Type::Named(name.clone(), args.iter().map(|a| self.apply_subst(a, subst)).collect())
            }
            Type::Ref(inner, m) => Type::Ref(Box::new(self.apply_subst(inner, subst)), m.clone()),
            Type::Slice(inner, m) => Type::Slice(Box::new(self.apply_subst(inner, subst)), m.clone()),
            Type::Rawptr(inner) => Type::Rawptr(Box::new(self.apply_subst(inner, subst))),
            Type::Array(inner, n) => Type::Array(Box::new(self.apply_subst(inner, subst)), *n),
            Type::Fn(ps, r) => Type::Fn(
                ps.iter().map(|p| self.apply_subst(p, subst)).collect(),
                Box::new(self.apply_subst(r, subst)),
            ),
            Type::Handle(inner) => Type::Handle(Box::new(self.apply_subst(inner, subst))),
            Type::Mutex(inner) => Type::Mutex(Box::new(self.apply_subst(inner, subst))),
            Type::Guard(inner) => Type::Guard(Box::new(self.apply_subst(inner, subst))),
            other => other.clone(),
        }
    }
}
