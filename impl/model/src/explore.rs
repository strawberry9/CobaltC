// Every outcome the rules allow a program: the static pass first (its
// first finding in source order is the program's outcome), then every
// interleaving of the threads' steps (`[Thread-Step]`, `outcome:
// unspecified`), explored depth-first with states deduplicated.
//
// Reduction: a step that touches nothing another thread can observe (the
// operand stack, a fresh statement scope or frame, a binding's own lookup)
// commutes with every step of every other thread, so it is taken without
// branching.

use crate::ast::Program;
use crate::machine::{Code, Ins, Machine, State, Status, Step, Tid};
use crate::statics;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashSet};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum End {
    Ok,
    Fault { diag: String, phase: &'static str, line: usize },
    TooBig,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Outcome {
    pub out: Vec<i64>,
    pub end: End,
}

impl Outcome {
    pub fn text(&self) -> String {
        let out: Vec<String> = self.out.iter().map(|k| k.to_string()).collect();
        let end = match &self.end {
            End::Ok => "ok".to_string(),
            End::Fault { diag, phase, line } => format!("{} ({}) line {}", diag, phase, line),
            End::TooBig => "too-big".to_string(),
        };
        if out.is_empty() {
            end
        } else {
            format!("[{}] {}", out.join(" "), end)
        }
    }
}

pub struct Report {
    pub outcomes: BTreeSet<Outcome>,
    // One schedule reaching each outcome: (thread, function, instruction).
    pub witness: std::collections::BTreeMap<Outcome, Vec<(Tid, usize, usize)>>,
    pub states: usize,
    #[allow(dead_code)]
    pub static_reject: bool, // decided before any step ran
}

fn local(ins: &Ins) -> bool {
    matches!(ins, Ins::Int(_) | Ins::Add | Ins::Gt | Ins::Pop | Ins::Jump(_) | Ins::JumpIfFalse(_) | Ins::StmtEnter | Ins::BlockEnter | Ins::HoldLit | Ins::Var(_) | Ins::Licensed(_))
}

pub const STATE_LIMIT: usize = 3_000_000;

pub fn run(prog: &Program, spare_result: bool) -> Report {
    run_with(prog, spare_result, false)
}

// D-0241: `as_licensed`, every access the discharge table proves is marked,
// and one that `clash` refutes all the same ends in the outcome
// `model.proven-but-false`.
pub fn run_with(prog: &Program, spare_result: bool, as_licensed: bool) -> Report {
    let st = statics::check(prog);
    if let Some(e) = st.first() {
        let mut outcomes = BTreeSet::new();
        outcomes.insert(Outcome { out: vec![], end: End::Fault { diag: e.diag.to_string(), phase: "static", line: e.line } });
        return Report { outcomes, witness: Default::default(), states: 0, static_reject: true };
    }
    let code = crate::machine::compile_with(prog, &st.ends, if as_licensed { Some(&st.proven) } else { None });
    explore(&code, spare_result)
}

// D-0299 (`cobmodel certify`): the program explored with the certificate's
// verdicts applied (`machine::compile_certified`): an alias or temporal
// fault at a step of a body the certificate calls decided is the outcome
// `model.decided-but-false`.
pub fn run_certified(prog: &Program, spare_result: bool, decided: &std::collections::BTreeSet<String>, all: bool) -> Report {
    let st = statics::check(prog);
    if let Some(e) = st.first() {
        let mut outcomes = BTreeSet::new();
        outcomes.insert(Outcome { out: vec![], end: End::Fault { diag: e.diag.to_string(), phase: "static", line: e.line } });
        return Report { outcomes, witness: Default::default(), states: 0, static_reject: true };
    }
    let code = crate::machine::compile_certified(prog, &st.ends, decided, all);
    explore(&code, spare_result)
}

// The faults a program with no record of references cannot raise: a
// conflict between references, or an access through one to an object that
// has ended. A bound, a resource's state or an uninitialised object is a
// check of a value, which such a program still makes.
pub const UNRECORDED: &[&str] = &["diag.aliasing-conflict", "diag.stale-binding", "diag.destroy-while-aliased", "diag.move-while-aliased"];

pub fn explore(code: &Code, spare_result: bool) -> Report {
    let m = Machine { code, spare_result };
    let mut outcomes = BTreeSet::new();
    let mut seen: HashSet<u64> = HashSet::new();
    let mut witness = std::collections::BTreeMap::new();
    let mut stack: Vec<(State, Vec<(Tid, usize, usize)>)> = vec![(State::new(code), vec![])];
    while let Some((s, hist)) = stack.pop() {
        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        if !seen.insert(h.finish()) {
            continue;
        }
        if seen.len() > STATE_LIMIT {
            let o = Outcome { out: s.out.clone(), end: End::TooBig };
            witness.entry(o.clone()).or_insert(hist.clone());
            outcomes.insert(o);
            break;
        }
        let en: Vec<Tid> = s.threads.keys().copied().filter(|t| m.enabled(&s, *t)).collect();
        if en.is_empty() {
            // Every live thread waits and none can go on. The main thread
            // waiting alone on a channel is `[Channel-Deadlock]`; otherwise
            // `[Deadlock]` (b): each thread at the end of a chain of waits
            // (one waiting on a channel or an atomic) faults.
            let running_spawned = s.threads.iter().any(|(k, x)| *k != 0 && x.status == Status::Running);
            let ends: Vec<Tid> = s.threads.keys().copied().filter(|t| s.threads[t].status == Status::Running && m.waits_at_chain_end(&s, *t)).collect();
            if !running_spawned && m.waits_on_channel(&s, 0) {
                let o = Outcome { out: s.out.clone(), end: End::Fault { diag: "diag.channel-deadlock".into(), phase: "dynamic", line: m.line(&s, 0) } };
                witness.entry(o.clone()).or_insert(hist.clone());
                outcomes.insert(o);
                continue;
            }
            if ends.is_empty() {
                let o = Outcome { out: s.out.clone(), end: End::Fault { diag: "diag.deadlock".into(), phase: "dynamic", line: m.line(&s, 0) } };
                witness.entry(o.clone()).or_insert(hist.clone());
                outcomes.insert(o);
                continue;
            }
            // The ends fault together; one that is not contained (the main
            // thread, or a lent one) ends the program, and which of them is
            // reported depends on which faults first.
            let uncontained: Vec<Tid> = ends.iter().copied().filter(|t| *t == 0 || s.threads[t].lent).collect();
            if !uncontained.is_empty() {
                for t in uncontained {
                    let o = Outcome { out: s.out.clone(), end: End::Fault { diag: "diag.deadlock".into(), phase: "dynamic", line: m.line(&s, t) } };
                    witness.entry(o.clone()).or_insert(hist.clone());
                    outcomes.insert(o);
                }
                continue;
            }
            let mut s2 = s.clone();
            for t in ends {
                let line = m.line(&s2, t);
                m.contain(&mut s2, t, crate::machine::Fault { diag: "diag.deadlock", line, tok: None, value: false });
            }
            let mut h2 = hist.clone();
            h2.push((u32::MAX, 0, 0));
            stack.push((s2, h2));
            continue;
        }
        // A thread whose next step is local goes alone.
        let choice: Vec<Tid> = match en.iter().find(|t| {
            let a = s.threads[t].acts.last().unwrap();
            local(&code.funcs[a.func].1[a.pc].0)
        }) {
            Some(t) => vec![*t],
            None => en,
        };
        for t in choice {
            let a = s.threads[&t].acts.last().unwrap();
            let decided_here = code.decided.get(a.func).copied().unwrap_or(false);
            let mut h2 = hist.clone();
            h2.push((t, a.func, a.pc));
            match m.step(s.clone(), t) {
                Step::Next(s2) => stack.push((s2, h2)),
                Step::Blocked => {}
                Step::Fault(s2, f) => {
                    // D-0299: the step was a decided body's, the fault one of
                    // a reference's record, not of a value's own state, and
                    // the access through a frame-held path -- or the whole
                    // program is decided, and keeps no record at all.
                    let unrecorded = decided_here && !f.value && UNRECORDED.contains(&f.diag) && (code.all_decided || f.tok.map_or(false, |a| s2.frame_held(a, t)));
                    let diag = if unrecorded { "model.decided-but-false".to_string() } else { f.diag.to_string() };
                    let o = Outcome { out: s2.out.clone(), end: End::Fault { diag, phase: "dynamic", line: f.line } };
                    witness.entry(o.clone()).or_insert(h2);
                    outcomes.insert(o);
                }
                Step::Exit(s2) => {
                    let o = Outcome { out: s2.out.clone(), end: End::Ok };
                    witness.entry(o.clone()).or_insert(h2);
                    outcomes.insert(o);
                }
            }
        }
    }
    let _ = Status::Running;
    Report { outcomes, witness, states: seen.len(), static_reject: false }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    // D-0299: a certificate's claim checked against the run. The program is
    // `sample/hand-b01.cb`: `r` is redirected to `x` on one path of a loop,
    // so the write to `x` after it is a conflict the static pass leaves to
    // the run (`fn main: left`), and the run finds it. A certificate that
    // called `main` decided would be refuted: the same fault, as
    // `model.decided-but-false`.
    const REDIRECTED: &str = "import std;\n\nfn main()\n{\n    i32 x = 1;\n    i32 y = 2;\n    auto r = &mut y;\n    i32 i = 0;\n    while (2 > i)\n    {\n        if (i > 0)\n        {\n            r = &mut x;\n            break;\n        }\n        i = i + 1;\n    }\n    x = 5;\n    *r = *r + 1;\n    printf(\"%v\\n\", x);\n    printf(\"%v\\n\", y);\n}\n";

    fn outcomes(decided: &[&str], all: bool) -> Vec<String> {
        let p = parse::parse(REDIRECTED).expect("parses");
        let set: std::collections::BTreeSet<String> = decided.iter().map(|s| s.to_string()).collect();
        run_certified(&p, true, &set, all).outcomes.iter().map(|o| o.text()).collect()
    }

    #[test]
    fn a_body_left_to_the_run_faults_as_the_rules_say() {
        assert_eq!(outcomes(&[], false), vec!["diag.aliasing-conflict (dynamic) line 18".to_string()]);
    }

    #[test]
    fn a_false_decided_claim_is_refuted() {
        assert_eq!(outcomes(&["main"], false), vec!["model.decided-but-false (dynamic) line 18".to_string()]);
        assert_eq!(outcomes(&[], true), vec!["model.decided-but-false (dynamic) line 18".to_string()]);
    }

    // `sample/three-p001018.cb`: `bump_hold` is decided, but its access goes
    // through a reference kept in data (`h.r`, a parameter's field), which
    // the run records in every function: the fault is the rules' own.
    const THROUGH_DATA: &str = "import std;\n\nstruct Hold\n{\n    ref<i32, exclusive> r;\n}\n\nfn bump_hold(Hold h)\n{\n    *h.r = *h.r + 1;\n}\n\nfn main()\n{\n    i32 x = 1;\n    i32 y = 2;\n    Hold h1 = Hold { .r = &mut x };\n    auto t1 = spawn(bump_hold, h1);\n    bump_hold(h1);\n    printf(\"%v\\n\", x);\n    printf(\"%v\\n\", y);\n}\n";

    // `sample/hand-o02.cb`: a `fn` value called after its closure was moved
    // out (`[Fn-Value-Empty-Call]`) is a check of the value's own state,
    // which a program keeping no record of references still makes.
    const EMPTY_CALL: &str = "import std;\n\nfn call_twice(fn() : void f)\n{\n    auto g = f;\n    g();\n    f();\n}\n\nfn main()\n{\n    i32 x = 1;\n    Vec<i32> v1 = Vec::new();\n    auto mk = move [v1]() { Vec::push(&mut v1, 1); };\n    call_twice(mk);\n    printf(\"%v\\n\", x);\n}\n";

    fn outcomes_of(src: &str, decided: &[&str], all: bool) -> Vec<String> {
        let p = parse::parse(src).expect("parses");
        let set: std::collections::BTreeSet<String> = decided.iter().map(|s| s.to_string()).collect();
        run_certified(&p, true, &set, all).outcomes.iter().map(|o| o.text()).collect()
    }

    #[test]
    fn an_access_through_data_in_a_decided_body_is_the_runs() {
        assert_eq!(outcomes_of(THROUGH_DATA, &["bump_hold"], false), vec!["diag.aliasing-conflict (dynamic) line 10".to_string(), "[3 2] ok".to_string()]);
        // Were the whole program decided, nothing would be recorded.
        assert_eq!(outcomes_of(THROUGH_DATA, &[], true), vec!["model.decided-but-false (dynamic) line 10".to_string(), "[3 2] ok".to_string()]);
    }

    #[test]
    fn a_value_state_check_is_not_a_record() {
        assert_eq!(outcomes_of(EMPTY_CALL, &[], true), vec!["diag.stale-binding (dynamic) line 7".to_string()]);
    }

    #[test]
    fn a_decided_claim_about_another_body_changes_nothing() {
        assert_eq!(outcomes(&["other"], false), vec!["diag.aliasing-conflict (dynamic) line 18".to_string()]);
    }
}
