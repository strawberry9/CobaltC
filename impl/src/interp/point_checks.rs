// D-0283: point checks at calls (`rule.control.frame-references`, spec/14
// §6). Where the static rule finds that what a callee may access through
// one argument meets another argument, or what a live holder of the caller
// borrows, only when values agree (two indexes, a `?` fact), the run checks
// those accesses when the call is made: a call that would conflict faults
// there, whether or not the callee then makes the access.

use super::*;
use crate::tier::PointCheck;

// A reference a value carries: its object, the place it reaches (a slice's
// range last), its mode, token and recorded ancestors.
type Reach = (u64, Vec<Proj>, Mode, u64, Option<Arc<Vec<u64>>>);

impl Interp {
    // After the parameters are bound: each access the callee's summary
    // declares through an argument a point check names is checked as if it
    // were made at the call, through the parameter's reference (its own
    // path excluded), against every live reference.
    pub(super) fn run_point_checks(&mut self, f: &FnDecl, pcs: &[PointCheck]) -> Result<(), Flow> {
        for pc in pcs {
            let Some(p) = f.params.get(pc.arg) else { continue };
            let Some((obj, false)) = self.lookup(&p.name) else { continue };
            let Some(v) = self.objects.get(&obj).map(|o| o.value.clone()) else { continue };
            let mut reaches = Vec::new();
            self.collect_reaches(&v, &mut reaches, 0);
            let direct = matches!(v, Value::Ref { range: None, .. });
            for (target, mut path, _, token, anc) in reaches {
                if direct && !pc.fields.is_empty() {
                    if let Value::Ref { pointee, .. } = &v {
                        self.extend_by_fields(&mut path, pointee, &pc.fields);
                    }
                }
                let mut chain = vec![token];
                if let Some(a) = &anc {
                    chain.extend(a.iter().copied());
                }
                let mode = if direct { pc.mode.clone() } else { Mode::Shared };
                // An empty range reaches nothing (`[Slice-Form]`).
                if empty_range(&path) {
                    continue;
                }
                if self.clash_with(target, &path, mode, &chain, true) {
                    return Err(Flow::Fault(crate::modres::named("diag.aliasing-conflict", pc.message())));
                }
            }
        }
        Ok(())
    }

    fn collect_reaches(&self, v: &Value, out: &mut Vec<Reach>, depth: u32) {
        if depth > 8 {
            return;
        }
        match v {
            Value::Ref { obj, path, mode, token, range, anc, .. } => {
                let mut p = path.clone();
                if let Some((lo, hi)) = range {
                    p.push(Proj::Range(*lo, *hi));
                }
                out.push((*obj, p, mode.clone(), *token, anc.clone()));
            }
            Value::Guard { obj, path, token, anc, .. } => out.push((*obj, path.clone(), Mode::Exclusive, *token, anc.clone())),
            Value::Struct(xs) | Value::Array(xs) => xs.iter().for_each(|x| self.collect_reaches(x, out, depth + 1)),
            Value::Enum(_, x) => self.collect_reaches(x, out, depth + 1),
            Value::Closure(c) => {
                if let Some(o) = self.objects.get(&c.id()) {
                    self.collect_reaches(&o.value, out, depth + 1);
                }
            }
            _ => {}
        }
    }

    // The place `fields` names below a referent of type `t`; it stops at a
    // field it cannot place (the whole is then compared, which reaches more).
    fn extend_by_fields(&self, path: &mut Vec<Proj>, t: &Type, fields: &[String]) {
        let mut t = t.clone();
        for f in fields {
            let Type::Named(n, _) = &t else { return };
            let Some(sd) = self.items.structs.get(n) else { return };
            if !sd.type_params.is_empty() {
                return;
            }
            let Some(i) = sd.fields.iter().position(|fd| &fd.name == f) else { return };
            path.push(Proj::Field(i));
            t = sd.fields[i].ty.clone();
        }
    }
}
