// cobmodel: an independent executable model of CobaltC's alias and thread
// rules (D-0219). See impl/model/README.md.
//
//   cobmodel check FILE...            the outcome set of each program
//   cobmodel enum --size N --out DIR  write the fragment's programs up to size N
//   cobmodel compare DIR --coby P --cobc P [--explore N] [--cc CC]...
//                                     run the tools and report disagreements

mod ast;
mod compare;
mod enumerate;
mod explore;
mod machine;
mod parse;
mod print;
mod statics;

use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage: cobmodel check [--literal-dead] [--as-licensed [--pre-d0236] [--pre-d0243]] [--witness] FILE...\n       cobmodel enum --size N --out DIR [--max K]\n       cobmodel compare DIR --coby PATH --cobc PATH [--explore N] [--cc CC]... [--jobs J]");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else { return usage() };
    match cmd.as_str() {
        "check" => {
            let mut literal = false;
            let mut show_witness = false;
            let mut licensed = false;
            let mut status = 0;
            let mut false_proofs = 0;
            for f in &args[1..] {
                if f == "--literal-dead" {
                    literal = true;
                    continue;
                }
                if f == "--as-licensed" {
                    licensed = true;
                    continue;
                }
                if f == "--pre-d0236" {
                    statics::PRE_D0236.store(true, std::sync::atomic::Ordering::Relaxed);
                    continue;
                }
                if f == "--pre-d0243" {
                    statics::PRE_D0243.store(true, std::sync::atomic::Ordering::Relaxed);
                    continue;
                }
                if f == "--witness" {
                    show_witness = true;
                    continue;
                }
                let src = match std::fs::read_to_string(f) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("{}: {}", f, e);
                        status = 2;
                        continue;
                    }
                };
                match parse::parse(&src) {
                    Ok(p) => {
                        let r = explore::run_with(&p, !literal, licensed);
                        println!("{}: {} outcome(s), {} states", f, r.outcomes.len(), r.states);
                        if r.outcomes.iter().any(|o| o.text().contains("model.proven-but-false")) {
                            false_proofs += 1;
                            status = 1;
                        }
                        for o in &r.outcomes {
                            println!("  {}", o.text());
                            if show_witness {
                                if let Some(w) = r.witness.get(o) {
                                    let st = statics::check(&p);
                                    let code = machine::compile(&p, &st.ends);
                                    for (t, fi, pc) in w {
                                        if *t == u32::MAX {
                                            println!("      [every thread waits: the ends of the chains of waits fault]");
                                            continue;
                                        }
                                        let (ins, line) = &code.funcs[*fi].1[*pc];
                                        println!("      t{} {}:{} line {} {:?}", t, code.funcs[*fi].0.name, pc, line, ins);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{}: {}", f, e);
                        status = 2;
                    }
                }
            }
            if licensed {
                println!("as licensed: {} program(s) with an access the flow table proves while clash refutes it", false_proofs);
            }
            ExitCode::from(status)
        }
        "trace" => trace_main(&args[1..]),
        "enum" => enumerate::main(&args[1..]),
        "compare" => compare::main(&args[1..]),
        _ => usage(),
    }
}

// D-0270: `cobmodel trace FILE [--out TRACE] [--max N]`: one run of the
// program, the lowest-numbered enabled thread stepping each time, its steps
// written as the mechanised fragment's actions (`proofs/Replay.lean`
// replays them through the proven machine). A statically rejected program
// has no run and writes an empty trace.
fn trace_main(args: &[String]) -> ExitCode {
    let mut file: Option<String> = None;
    let mut out: Option<String> = None;
    let mut max: usize = 200_000;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                out = args.get(i + 1).cloned();
                i += 2;
            }
            "--max" => {
                max = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(max);
                i += 2;
            }
            a => {
                file = Some(a.to_string());
                i += 1;
            }
        }
    }
    let Some(file) = file else {
        eprintln!("usage: cobmodel trace FILE [--out TRACE] [--max N]");
        return ExitCode::from(2);
    };
    let src = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {}", file, e);
            return ExitCode::from(2);
        }
    };
    let prog = match parse::parse(&src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}: {}", file, e);
            return ExitCode::from(2);
        }
    };
    let st = statics::check(&prog);
    let mut lines: Vec<String> = Vec::new();
    let outcome: String;
    if let Some(e) = st.first() {
        outcome = format!("{} (static) line {}", e.diag, e.line);
    } else {
        let code = machine::compile(&prog, &st.ends);
        let m = machine::Machine { code: &code, spare_result: true };
        machine::trace_start();
        let mut s = machine::State::new(&code);
        let pf = s.threads[&0].acts[0].param_frame;
        machine::tr(format!("ben 0 {}", pf));
        let mut steps = 0;
        loop {
            if steps >= max {
                outcome = "too-long".to_string();
                break;
            }
            steps += 1;
            let en: Vec<machine::Tid> = s.threads.keys().copied().filter(|t| m.enabled(&s, *t)).collect();
            let Some(t) = en.first().copied() else {
                outcome = "deadlock (every thread waits)".to_string();
                break;
            };
            match m.step(s.clone(), t) {
                machine::Step::Next(s2) => s = s2,
                machine::Step::Blocked => {}
                machine::Step::Fault(_, f) => {
                    outcome = format!("{} (dynamic) line {}", f.diag, f.line);
                    machine::tr("end".to_string());
                    break;
                }
                machine::Step::Exit(s2) => {
                    outcome = if s2.out.is_empty() { "ok".to_string() } else { format!("[{}] ok", s2.out.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(" ")) };
                    break;
                }
            }
        }
        lines = machine::trace_take();
    }
    let text = format!("# {}: {}\n{}\n", file, outcome, lines.join("\n"));
    match out {
        Some(path) => {
            if let Err(e) = std::fs::write(&path, &text) {
                eprintln!("{}: {}", path, e);
                return ExitCode::from(2);
            }
        }
        None => print!("{}", text),
    }
    println!("{}: {} ({} action(s))", file, outcome, lines.len());
    ExitCode::SUCCESS
}
