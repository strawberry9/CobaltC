// Every row of spec/conformance.md, run through the interpreter and
// compared with the spec's own stated outcome *and phase* (`✗ diag.x
// (static)` must be rejected by the static pass; `(dynamic)` must be
// accepted statically and fault at run time; `→ v` / `ok` must run to
// completion). The rows themselves are assembled by tests/common/mod.rs,
// shared with the compiler's runner (cobc/tests/compiled_spec_rows.rs).

mod common;

use common::*;
use std::fs;
use std::path::Path;

fn observed(o: &coby::Outcome) -> Option<(String, &'static str)> {
    match o {
        coby::Outcome::Ok(_) => None,
        coby::Outcome::Static(d) => Some((d.clone(), "static")),
        coby::Outcome::Dynamic(d) => Some((d.clone(), "dynamic")),
    }
}

#[test]
fn every_spec_conformance_row_produces_its_stated_outcome_and_phase() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = fs::read_to_string(root.join("../spec/conformance.md")).expect("spec/conformance.md");
    let examples = fs::read_to_string(root.join("../spec/examples.md")).expect("spec/examples.md");
    let mut failures = Vec::new();
    let mut count = 0;
    for row in rows(&spec, &examples) {
        let bare = |d: &str| coby::diagnostics::split_location(d).0.to_string();
        let (got, shown) = match &row.program {
            Program::File(path) => {
                let full = root.join("..").join(path);
                let src = fs::read_to_string(&full).unwrap_or_else(|e| panic!("{}: cannot read {}: {}", row.id, full.display(), e));
                // A file only `cobc` can run (a `cobc-only:` header,
                // CHG-0039): `coby` must refuse it; cobc/tests/
                // compiled_spec_rows.rs checks its outcome.
                if src.lines().take_while(|l| l.starts_with("//")).any(|l| l.trim_start_matches('/').trim().starts_with("cobc-only:")) {
                    let (located, _) = coby::run_program_phased_located(&src, Some(&full));
                    count += 1;
                    let refused = matches!(&located, coby::OutcomeLocated::Dynamic(d) if d.starts_with("unsupported: "));
                    if !refused {
                        failures.push(format!("{}: cobc-only, so coby must refuse it; got {:?}", row.id, located));
                    }
                    continue;
                }
                // A file that reads standard input (`stdin-hex:`, D-0050)
                // runs as a process, given those bytes; its row says `ok`,
                // and its `expect-stdout-hex:` is what it must print.
                let field = |key: &str| src.lines().take_while(|l| l.starts_with("//")).find_map(|l| {
                    let body = l.trim_start_matches('/').trim();
                    body.strip_prefix(key).and_then(|r| r.strip_prefix(':')).map(|v| v.trim().to_string())
                });
                if let Some(h) = field("stdin-hex") {
                    use std::io::Write;
                    use std::process::{Command, Stdio};
                    let input: Vec<u8> = (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap()).collect();
                    let mut child = Command::new(env!("CARGO_BIN_EXE_coby"))
                        .current_dir(case_dir(&full))
                        .arg(&full)
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                        .expect("run coby");
                    let _ = child.stdin.take().unwrap().write_all(&input);
                    let out = child.wait_with_output().expect("run coby");
                    count += 1;
                    let hex: String = out.stdout.iter().map(|b| format!("{:02x}", b)).collect();
                    let want = field("expect-stdout-hex");
                    if !out.status.success() || want.map_or(false, |w| w != hex) || !matches!(row.exp, Exp::Ok(0)) {
                        failures.push(format!("{}: reading its input, coby exited {:?} printing {} ({})", row.id, out.status.code(), hex, String::from_utf8_lossy(&out.stderr).trim()));
                    }
                    continue;
                }
                // A file case runs in its own directory (CHG-0043); this
                // binary's one test is the only thing running in it.
                let here = std::env::current_dir().expect("working directory");
                std::env::set_current_dir(case_dir(&full)).expect("case directory");
                let (located, _) = coby::run_program_with_args(&src, Some(&full), case_args(&src));
                std::env::set_current_dir(here).expect("working directory");
                if !COBY_CALLS_C && matches!(&located, coby::OutcomeLocated::Dynamic(d) if d.contains(NO_C_CALLS)) {
                    eprintln!("skipped {}: it calls C, which coby does only on x86-64 Linux", row.id);
                    continue;
                }
                let got = match located {
                    coby::OutcomeLocated::Ok(s) => coby::Outcome::Ok(s),
                    coby::OutcomeLocated::Static(d) => coby::Outcome::Static(bare(&d)),
                    coby::OutcomeLocated::Dynamic(d) => coby::Outcome::Dynamic(bare(&d)),
                };
                (got, path.clone())
            }
            Program::Source(prog) => (coby::run_source_phased(prog), prog.clone()),
        };
        count += 1;
        let obs = observed(&got);
        let status = if let coby::Outcome::Ok(s) = got { s } else { 0 };
        let ok = agrees(&row.exp, obs.as_ref().map(|(d, p)| (d.as_str(), *p)), status);
        // `conf.cross-thread-write-conflict` is `outcome: unspecified`.
        let ok = ok || (row.id == "conf.cross-thread-write-conflict" && matches!(got, coby::Outcome::Ok(_)));
        if !ok {
            failures.push(format!("{}: expected {:?}, got {:?}\n--- program ---\n{}", row.id, row.exp, got, shown));
        }
    }
    eprintln!("{} spec rows checked", count);
    assert!(count > 100, "only {} rows assembled", count);
    assert!(failures.is_empty(), "{} of {} rows disagree with spec/conformance.md:\n{}", failures.len(), count, failures.join("\n\n"));
}
