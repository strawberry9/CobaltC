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
                m.contain(&mut s2, t, crate::machine::Fault { diag: "diag.deadlock", line });
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
            let mut h2 = hist.clone();
            h2.push((t, a.func, a.pc));
            match m.step(s.clone(), t) {
                Step::Next(s2) => stack.push((s2, h2)),
                Step::Blocked => {}
                Step::Fault(s2, f) => {
                    let o = Outcome { out: s2.out.clone(), end: End::Fault { diag: f.diag.to_string(), phase: "dynamic", line: f.line } };
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
