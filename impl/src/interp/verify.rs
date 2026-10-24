// Whole-state checks of the invariants (D-0261). With `COBALTC_VERIFY`
// set, the interpreter re-derives, from the whole state and
// independently of the bookkeeping the per-access checks rely on, what
// those checks assume: at every alias check, that the references the
// holder index finds for the object are exactly the ones a scan of every
// live object finds (`inv.alias-validity` is a property of accesses,
// D-0018 and D-0107: two conflicting paths may be held, as a call's
// parameters, until one is used, so the invariant is re-derived where it
// is checked, not of the state between statements); and after every
// statement (or every Nth, `COBALTC_VERIFY=N`), that the holder index
// lists every holder, that every live guard's mutex is locked and every
// locked mutex has its guard (`inv.concurrency-validity`), and that no
// live object is the value of two bindings (`inv.resource-authority`);
// never more often than the state's size allows (`BUDGET`). A
// violation is a defect of this interpreter or of the rules, never of
// the program: it panics naming what it found, and the program that
// reached it is the counterexample. The scan is linear in the state, so
// it is a mode, not the default.

use super::*;
use std::sync::OnceLock;

// The stride: `COBALTC_VERIFY=1` (or any non-number) checks after every
// statement, `COBALTC_VERIFY=N` after every Nth; 0 or unset is off.
fn verify_stride() -> u64 {
    static STRIDE: OnceLock<u64> = OnceLock::new();
    *STRIDE.get_or_init(|| match std::env::var("COBALTC_VERIFY") {
        Ok(v) if v.is_empty() || v == "0" => 0,
        Ok(v) => v.trim().parse::<u64>().unwrap_or(1).max(1),
        Err(_) => 0,
    })
}

// How many live objects one check may scan per event between checks:
// a check costs a scan of every object, so it is made no more often than
// every (objects / BUDGET)th event, whatever the stride asks, and the
// mode costs a bounded factor on a program however large its state.
const BUDGET: u64 = 64;

// One live reference or guard the state holds: where it is held, what it
// reaches, and its identity.
struct Occurrence {
    holder: Option<u64>, // an object, or `None` for raw storage's side table
    target: u64,
    path: Vec<Proj>, // with a slice's range as its last step
    token: u64,
    is_guard: bool,
}

fn collect(v: &Value, holder: Option<u64>, out: &mut Vec<Occurrence>) {
    match v {
        Value::Ref { obj, path, token, range, .. } => {
            let mut p = path.clone();
            if let Some((lo, hi)) = range {
                p.push(Proj::Range(*lo, *hi));
            }
            out.push(Occurrence { holder, target: *obj, path: p, token: *token, is_guard: false });
        }
        Value::Guard { obj, path, token, .. } => {
            out.push(Occurrence { holder, target: *obj, path: path.clone(), token: *token, is_guard: true });
        }
        Value::Struct(fs) | Value::Array(fs) => fs.iter().for_each(|f| collect(f, holder, out)),
        Value::Enum(_, p) => collect(p, holder, out),
        _ => {}
    }
}

fn show_path(p: &[Proj]) -> String {
    let mut s = String::new();
    for step in p {
        match step {
            Proj::Field(i) => s.push_str(&format!(".{}", i)),
            Proj::Index(i) => s.push_str(&format!("[{}]", i)),
            Proj::Payload => s.push_str(".payload"),
            Proj::Range(l, h) => s.push_str(&format!("[{}..{}]", l, h)),
        }
    }
    if s.is_empty() {
        s.push_str("(whole)");
    }
    s
}

impl Interp {
    // Whether this event (a statement's end, an alias check) is one the
    // mode checks: every Nth for `COBALTC_VERIFY=N`, and no more often
    // than the state's size allows (`BUDGET`).
    pub(super) fn verify_due(&self) -> bool {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let stride = verify_stride();
        if stride == 0 {
            return false;
        }
        let n = stride.max(self.objects.len() as u64 / BUDGET);
        COUNT.fetch_add(1, Ordering::Relaxed) % n == n - 1
    }

    fn verify_fail(&self, what: &str) -> ! {
        panic!("coby verify: {} (thread {}, after the statement at line {})", what, self.current_thread, self.current_line);
    }

    // Every live occurrence of a reference or guard, in every live object
    // and in raw storage's side table.
    fn occurrences(&self) -> Vec<Occurrence> {
        let mut out = Vec::new();
        for (id, obj) in self.objects.iter() {
            collect(&obj.value, Some(*id), &mut out);
        }
        for v in self.arena_side.values() {
            collect(v, None, &mut out);
        }
        out
    }

    pub(super) fn verify_invariants(&self) {
        let occ = self.occurrences();
        self.verify_holder_index(&occ);
        self.verify_locks(&occ);
        self.verify_ownership();
    }

    // At an alias check of `target`: the distinct references the holder
    // index finds are exactly those a scan of every live object finds.
    // `clash` and `solitary` decide from the index; a reference the index
    // misses is a conflict never seen, and one it keeps past its holder's
    // end is a conflict that is not there.
    pub(super) fn verify_refs_to(&self, target: u64) {
        let indexed = self.distinct_refs_to(target);
        let mut seen: IdSet = Default::default();
        let mut full: Vec<(Vec<Proj>, Mode, u64, bool)> = Vec::new();
        for obj in self.objects.values() {
            Self::scan_distinct(&obj.value, target, &mut seen, &mut full);
        }
        for v in self.arena_side.values() {
            Self::scan_distinct(v, target, &mut seen, &mut full);
        }
        let mut a: Vec<u64> = indexed.iter().map(|r| r.2).collect();
        let mut b: Vec<u64> = full.iter().map(|r| r.2).collect();
        a.sort_unstable();
        b.sort_unstable();
        if a != b {
            let missing: Vec<u64> = b.iter().filter(|t| !a.contains(t)).copied().collect();
            let extra: Vec<u64> = a.iter().filter(|t| !b.contains(t)).copied().collect();
            self.verify_fail(&format!(
                "holder index at an alias check of object {}: the index finds tokens {:?} but a scan of every object finds {:?} (missing from the index: {:?}; found only through it: {:?})",
                target, a, b, missing, extra
            ));
        }
    }

    // The per-access checks find the references to an object through
    // `holders_by_target`; an occurrence the index does not know is one
    // `clash` and `solitary` never see.
    fn verify_holder_index(&self, occ: &[Occurrence]) {
        for o in occ {
            let Some(h) = o.holder else { continue };
            if !self.holders_by_target.get(&o.target).map_or(false, |hs| hs.contains(&h)) {
                self.verify_fail(&format!(
                    "holder index: object {} holds a {} to object {} {} (token {}) that `holders_by_target` does not list",
                    h,
                    if o.is_guard { "guard" } else { "reference" },
                    o.target,
                    show_path(&o.path),
                    o.token
                ));
            }
        }
    }

    // `inv.concurrency-validity`, the lock part: every live guard's mutex
    // is locked, and every locked mutex has a live guard.
    fn verify_locks(&self, occ: &[Occurrence]) {
        let locks = self.shared.lock().unwrap().cobalt_locks.clone();
        let mut guarded: Vec<(u64, Vec<Proj>)> = Vec::new();
        for o in occ.iter().filter(|o| o.is_guard) {
            if !self.object_live(o.target) {
                continue;
            }
            // A guard's path projects into the mutex: the mutex's own place
            // is the path less its last step.
            let Some((_, mutex_path)) = o.path.split_last() else {
                self.verify_fail(&format!("locks: a guard (token {}) with an empty path on object {}", o.token, o.target));
            };
            let key = (o.target, mutex_path.to_vec());
            if !locks.contains_key(&key) {
                self.verify_fail(&format!("locks: a live guard (token {}) of the mutex at object {} {} while that mutex is not locked", o.token, o.target, show_path(mutex_path)));
            }
            guarded.push(key);
        }
        for (key, holder) in &locks {
            if !self.object_live(key.0) {
                continue;
            }
            if !guarded.contains(key) {
                self.verify_fail(&format!("locks: the mutex at object {} {} is locked by thread {} but no live guard holds it", key.0, show_path(&key.1), holder));
            }
        }
    }

    // `inv.resource-authority`, the frames' part: a binding names a live
    // object unless its value was moved away; no object is the current
    // value of two bindings. (A frame's `owned` list keeps the id of an
    // object that moved out through one of its bindings, to find it
    // still alive at the frame's exit, so the list alone is no owner.)
    fn verify_ownership(&self) {
        let mut owners: IdMap<(u64, usize, String)> = Default::default();
        for (tid, frames) in self.frames_by_thread.iter() {
            for (depth, f) in frames.iter().enumerate() {
                for (name, obj) in &f.bindings {
                    if f.moved.contains(name) {
                        continue;
                    }
                    // An object that ended while its binding stayed was
                    // moved out or taken apart (a destructuring, a pattern):
                    // the binding is then moved-from, whether or not marked.
                    if !self.object_live(*obj) {
                        continue;
                    }
                    // A capture cell is a per-call alias of the capture's
                    // own storage (`bind_closure_captures`), not an owner.
                    if self.capture_cells.contains(obj) {
                        continue;
                    }
                    if let Some((t2, d2, n2)) = owners.insert(*obj, (*tid, depth, name.clone())) {
                        self.verify_fail(&format!("ownership: object {} is the value of two bindings: `{}` (thread {}, frame {}) and `{}` (thread {}, frame {})", obj, n2, t2, d2, name, tid, depth));
                    }
                }
            }
        }
    }
}
