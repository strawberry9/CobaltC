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
mod frames;
mod machine;
mod parse;
mod print;
mod statics;

use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage: cobmodel check [--literal-dead] [--as-licensed [--pre-d0236] [--pre-d0243]] [--pre-d0282] [--witness] FILE...\n       cobmodel certify [--witness] CERT FILE\n       cobmodel statics [--pre-d0282] FILE...\n       cobmodel enum --size N --out DIR [--max K]\n       cobmodel compare DIR --coby PATH --cobc PATH [--explore N] [--cc CC]... [--jobs J]");
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
                if f == "--pre-d0282" {
                    statics::PRE_D0282.store(true, std::sync::atomic::Ordering::Relaxed);
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
        "certify" => certify_main(&args[1..]),
        "trace" => trace_main(&args[1..]),
        "statics" => statics_main(&args[1..]),
        "enum" => enumerate::main(&args[1..]),
        "compare" => compare::main(&args[1..]),
        _ => usage(),
    }
}

// D-0299: `cobmodel certify [--witness] CERT FILE`: the certificate `cobc
// --certificate CERT FILE` (or `coby --certificate CERT --check FILE`)
// wrote, checked against every interleaving of FILE. Each body the
// certificate calls decided is run as a compiler runs it, keeping no record
// of its references: an alias or temporal fault the rules find at one of
// its steps is the outcome `model.decided-but-false`, and the exit status
// is 1. The static verdicts must agree too: a program the certificate calls
// rejected is one the model's statics reject with the same diagnostic, and
// a program it accepts is one they accept. The certificate's bodies are
// matched by name; `std`'s bodies, which the fragment has as rules, and a
// generic instantiation's arguments are left out.
fn certify_main(args: &[String]) -> ExitCode {
    let mut show_witness = false;
    let mut files: Vec<&String> = Vec::new();
    for a in args {
        if a == "--witness" {
            show_witness = true;
        } else {
            files.push(a);
        }
    }
    let [cert, file] = files[..] else { return usage() };
    let text = match std::fs::read_to_string(cert) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {}", cert, e);
            return ExitCode::from(2);
        }
    };
    if !text.starts_with("CobaltC certificate 1\n") {
        eprintln!("{}: not a CobaltC certificate (version 1)", cert);
        return ExitCode::from(2);
    }
    let mut decided: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut user_bodies = 0usize;
    let mut all = false;
    let mut rejected: Option<String> = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("verdict: ") {
            if v == "decided" {
                all = true;
            } else if let Some(d) = v.strip_prefix("rejected: ") {
                rejected = Some(d.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("fn ") {
            let Some((name, verdict)) = rest.rsplit_once(": ") else { continue };
            if name.contains("::") {
                continue;
            }
            user_bodies += 1;
            if verdict == "decided" {
                decided.insert(name.split('<').next().unwrap_or(name).to_string());
            }
        }
    }
    let src = match std::fs::read_to_string(file) {
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
    match (st.first(), &rejected) {
        (Some(e), Some(d)) if e.diag == d => {
            println!("{}: rejected by both the certificate and the model ({} (static) line {}); nothing runs", file, e.diag, e.line);
            return ExitCode::SUCCESS;
        }
        (Some(e), Some(d)) => {
            println!("{}: NOT certified: the static verdicts differ: the certificate says {}, the model {} (static) line {}", file, d, e.diag, e.line);
            return ExitCode::from(1);
        }
        (Some(e), None) => {
            println!("{}: NOT certified: the static verdicts differ: the certificate accepts the program, the model rejects it ({} (static) line {})", file, e.diag, e.line);
            return ExitCode::from(1);
        }
        (None, Some(d)) => {
            println!("{}: NOT certified: the static verdicts differ: the certificate says {}, the model accepts the program", file, d);
            return ExitCode::from(1);
        }
        (None, None) => {}
    }
    let r = explore::run_certified(&prog, true, &decided, all);
    let bad = r.outcomes.iter().any(|o| o.text().contains("model.decided-but-false"));
    let scope = if all { "the whole program".to_string() } else { format!("{} of {} bodies", decided.len(), user_bodies) };
    println!("{}: {}: {} decided, {} outcome(s), {} states", file, if bad { "NOT certified" } else { "certified" }, scope, r.outcomes.len(), r.states);
    for o in &r.outcomes {
        println!("  {}", o.text());
        if show_witness {
            if let Some(w) = r.witness.get(o) {
                let code = machine::compile(&prog, &st.ends);
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
    ExitCode::from(if bad { 1 } else { 0 })
}

// `cobmodel statics [--pre-d0282] FILE...`: each program's static verdict
// alone, `ok` or the first finding (`diag (static) line N`), without the
// exploration -- for comparing with `coby --check` over a large corpus.
fn statics_main(args: &[String]) -> ExitCode {
    let mut status = 0;
    for f in args {
        if f == "--pre-d0282" {
            statics::PRE_D0282.store(true, std::sync::atomic::Ordering::Relaxed);
            continue;
        }
        let prog = match std::fs::read_to_string(f).map_err(|e| e.to_string()).and_then(|s| parse::parse(&s).map_err(|e| e.to_string())) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{}: {}", f, e);
                status = 2;
                continue;
            }
        };
        match statics::check(&prog).first() {
            Some(e) => println!("{}: {} (static) line {}", f, e.diag, e.line),
            None => println!("{}: ok", f),
        }
    }
    ExitCode::from(status)
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
