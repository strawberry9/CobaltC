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
        "enum" => enumerate::main(&args[1..]),
        "compare" => compare::main(&args[1..]),
        _ => usage(),
    }
}
