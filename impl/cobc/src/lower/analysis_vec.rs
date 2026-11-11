// Per-function analyses: which Vecs a body confines.

use super::*;

impl<'a> Gen<'a> {
    // Stage 3 (`COBC-PLAN.md` §10): the names of locals that may be
    // *confined* `Vec`s — declared with an initializer, and used only as
    // `&x`/`&mut x` passed straight to the prelude's `Vec::push`,
    // `Vec::len`, or `Vec::index_*` whose result is dereferenced at once
    // (a read in a value position, or the target of an assignment whose
    // value pushes nothing onto `x`), and at most moved out as the
    // function body's own result. Nothing then ever holds a path to such
    // a local, or to one of its elements, but its root: see
    // `bind_confined`. By name, across scopes and shadowing, like
    // `pinned_names`: a use of any binding of the name that is not one of
    // these rules the name out, which only errs toward checking.
    // `owned`: parameters that are `Vec`s taken by value, confined as a
    // local declared with the moved value would be (`Fx::bind_param`).
    pub(super) fn confined_vecs(&self, body: &Block, params: &[String], owned: &[String]) -> HashSet<String> {
        let (mut declared, bad) = self.scan_vec_uses(body, params, &self.confining, true);
        declared.extend(owned.iter().cloned());
        declared.difference(&bad).cloned().collect()
    }

    // Which parameters of each program function are *confining*: typed
    // `ref<Vec<_>, _>`, never re-bound in the body, and used only as a
    // confined local would be (`confined_vecs`) — the vector argument of
    // `Vec::push`/`Vec::len`/`*Vec::index_*` (`p`, `&*p`, `&mut *p`), or
    // passed on to a confining parameter, with no other argument of that
    // call passing it too — never returned, stored, captured or copied. A
    // confined vector passed there stays confined: for the call's
    // duration the parameter is the only path to it (its caller is
    // suspended, nothing else was handed it, no thread holds it), so the
    // variant compiled for such arguments (`Gen::request_variant`) checks
    // nothing through it. The greatest fixed point: parameters that only
    // pass the vector to one another (recursion) are confining.
    pub(super) fn confining_table(&self) -> HashMap<String, Vec<bool>> {
        let is_vec_ref = |t: &Type| matches!(t, Type::Ref(inner, _) if matches!(&**inner, Type::Named(n, a) if n == "std::Vec" && a.len() == 1));
        let mut t: HashMap<String, Vec<bool>> =
            self.items.fns.iter().filter(|(_, f)| !in_prelude(f)).map(|(n, f)| (n.clone(), f.params.iter().map(|p| is_vec_ref(&p.ty)).collect())).collect();
        loop {
            let mut changed = false;
            for (n, f) in self.items.fns.iter() {
                let Some(cur) = t.get(n) else { continue };
                if !cur.iter().any(|&b| b) {
                    continue;
                }
                let pnames: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
                let (declared, bad) = self.scan_vec_uses(&f.body, &pnames, &t, false);
                let new: Vec<bool> = f.params.iter().zip(cur).map(|(p, &c)| c && !bad.contains(&p.name) && !declared.contains(&p.name)).collect();
                if &new != cur {
                    t.insert(n.clone(), new);
                    changed = true;
                }
            }
            if !changed {
                return t;
            }
        }
    }

    // The scan behind `confined_vecs` and `confining_table`: the names a
    // body declares with an initializer, and the names it uses in any way
    // but the allowed ones. `fn_body`: a bare name as the body's own
    // result is allowed (a local moved out as the result); not for
    // parameters, where it would return the reference.
    pub(super) fn scan_vec_uses(&self, body: &Block, params: &[String], table: &HashMap<String, Vec<bool>>, fn_body: bool) -> (HashSet<String>, HashSet<String>) {
        vec_scan::scan(self, body, params, table, fn_body)
    }
}
