// Every row of spec/conformance.md, compiled and run by `cobc --run`
// (impl/COBC-PLAN.md §4, M5), once under each C compiler
// (`oracle::c_compilers`: GCC and Clang). The rows are the same programs
// tests/spec_rows.rs runs through the interpreter (tests/common/mod.rs
// assembles them for both), checked against the same outcome and
// phase, and — for rows that agree — against the interpreter's stdout
// and stderr (the diagnostic, location included).
// A row `cobc` cannot compile (exit 3, `unsupported: …`) is counted as
// skipped and listed. A row's file is fed its `stdin-hex:` bytes, if
// any, as the `coby` oracle is (D-0050); one only `cobc` can run (a
// `cobc-only:` header) is checked against its own header (CHG-0039).

#[path = "../../tests/common/mod.rs"]
mod common;
mod oracle;

use common::*;
use oracle::{output_differs, output_differs_from_header};
use std::fs;
use std::path::{Path, PathBuf};

enum Run {
    // The diagnostic and phase, or `None` for ok with the exit status.
    Outcome(Option<(String, &'static str)>, u8, Vec<u8>, Vec<u8>),
    Skipped(String),
    Failed(String),
}

fn run_cobc(cc: &str, path: &Path, stdin: Option<&[u8]>) -> Run {
    let mut cmd = oracle::cobc(cc);
    let src = fs::read_to_string(path).unwrap_or_default();
    cmd.current_dir(case_dir(path)).arg("--run").arg(path).args(case_args_os(&src).expect("cobc runs on Linux"));
    let out = match stdin {
        Some(input) => oracle::output_with_stdin(&mut cmd, input),
        None => cmd.output().expect("run cobc"),
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    let first = stderr.lines().next().unwrap_or("").to_string();
    // `ok(s)`: main's status, and nothing on stderr (CHG-0040).
    if out.status.success() || out.stderr.is_empty() {
        return Run::Outcome(None, out.status.code().unwrap_or(-1) as u8, out.stdout, out.stderr);
    }
    if out.status.code() == Some(3) {
        return Run::Skipped(first.trim_start_matches("unsupported: ").to_string());
    }
    if out.status.code() == Some(2) || first.contains("parse error at") || first.contains("lex error at") {
        return Run::Failed(first);
    }
    let diag = first.split_whitespace().find(|t| t.starts_with("diag.")).unwrap_or("?").to_string();
    let phase = if first.contains("(static)") {
        "static"
    } else if first.contains("(dynamic)") {
        "dynamic"
    } else {
        "?"
    };
    Run::Outcome(Some((diag, phase)), 0, out.stdout, out.stderr)
}

#[test]
fn every_spec_conformance_row_compiles_to_its_stated_outcome() {
    let mut failures = Vec::new();
    for (cc, (passed, skipped, fails)) in oracle::per_compiler(rows_under) {
        eprintln!("compiled spec rows [{}]: {} passed, {} skipped, {} failed", cc, passed, skipped.len(), fails.len());
        for s in &skipped {
            eprintln!("  skipped {}", s);
        }
        assert!(passed > 100, "[{}] only {} rows compiled and passed", cc, passed);
        failures.extend(fails.into_iter().map(|f| format!("[{}] {}", cc, f)));
    }
    assert!(failures.is_empty(), "{} compiled row(s) disagree with spec/conformance.md:\n{}", failures.len(), failures.join("\n"));
}

// One compiler's run of every row: passed, skipped, failures.
fn rows_under(cc: &str) -> (usize, Vec<String>, Vec<String>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let spec = fs::read_to_string(root.join("../spec/conformance.md")).expect("spec/conformance.md");
    let examples = fs::read_to_string(root.join("../spec/examples.md")).expect("spec/examples.md");
    let scratch: PathBuf = std::env::temp_dir().join(format!("cobc-rows-{}-{}", std::process::id(), cc.replace(|c: char| !c.is_ascii_alphanumeric(), "_")));
    fs::create_dir_all(&scratch).unwrap();
    let mut passed = 0;
    let mut skipped: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for row in rows(&spec, &examples) {
        let path = match &row.program {
            Program::File(p) => root.join("..").join(p),
            Program::Source(src) => {
                let p = scratch.join(format!("{}.cb", row.id.replace(|c: char| !c.is_ascii_alphanumeric(), "_")));
                fs::write(&p, src).unwrap();
                p
            }
        };
        let src = fs::read_to_string(&path).unwrap();
        let stdin = oracle::case_stdin(&src).or_else(|| oracle::is_cobc_only(&src).then(Vec::new));
        match run_cobc(cc, &path, stdin.as_deref()) {
            Run::Skipped(why) => skipped.push(format!("{}: {}", row.id, why)),
            Run::Failed(msg) => failures.push(format!("{}: cobc failed: {}", row.id, msg)),
            Run::Outcome(got, status, stdout, stderr) => {
                let ok = agrees(&row.exp, got.as_ref().map(|(d, p)| (d.as_str(), *p)), status);
                // A race: whether the conflict is seen depends on how the
                // threads interleave, so neither tool's output is fixed.
                let racy = row.id == "conf.cross-thread-write-conflict";
                let ok = ok || (racy && got.is_none());
                if !ok {
                    failures.push(format!("{}: expected {:?}, observed {:?}", row.id, row.exp, got));
                } else if racy {
                    passed += 1;
                } else if let Some(d) = match oracle::is_cobc_only(&src) {
                    true => output_differs_from_header(&src, &stdout, &stderr),
                    false => output_differs(&path, &stdout, &stderr),
                } {
                    failures.push(format!("{}: {}", row.id, d));
                } else {
                    passed += 1;
                }
            }
        }
    }
    let _ = fs::remove_dir_all(&scratch);
    (passed, skipped, failures)
}
