// The file-based conformance suite and the examples, compiled and run
// by `cobc --run` instead of interpreted by `coby` (impl/COBC-PLAN.md
// §4), once under each C compiler (`oracle::c_compilers`: GCC and
// Clang). Every case's outcome must match its header exactly as under
// `coby`, and its stdout and stderr (the diagnostic, location included)
// must be `coby`'s; a case `cobc` cannot compile yet (exit 3, `unsupported: …`)
// is counted as skipped and listed, so the gap is visible, not hidden.
// A case's `stdin-hex:` bytes are its standard input, for `cobc`'s
// program and the `coby` oracle alike (CHG-0038, D-0050). A case with a
// `cobc-only:` header (it links its own C code) has no oracle, so its
// output is checked against its header instead (CHG-0039).

mod oracle;

use oracle::{output_differs, output_differs_from_header};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn parse_expect(src: &str) -> Option<(String, String)> {
    let mut fields: HashMap<String, String> = HashMap::new();
    for line in src.lines() {
        let line = line.trim_start();
        if !line.starts_with("//") {
            break;
        }
        let body = line.trim_start_matches('/').trim();
        if let Some((key, val)) = body.split_once(':') {
            fields.insert(key.trim().to_string(), val.trim().to_string());
        }
    }
    Some((fields.remove("kind")?, fields.remove("expect")?))
}

fn find_cb_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e)) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            find_cb_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("cb") {
            out.push(path);
        }
    }
}

enum Run {
    Outcome(String, Vec<u8>, Vec<u8>),
    Skipped(String),
}

fn run_case(cc: &str, path: &Path, stdin: Option<&[u8]>) -> Run {
    let mut cmd = oracle::cobc(cc);
    let src = fs::read_to_string(path).unwrap_or_default();
    cmd.current_dir(oracle::case_dir(path)).arg("--run").arg(path).args(oracle::case_args_os(&src).expect("cobc runs on Linux"));
    let out = match stdin {
        Some(input) => oracle::output_with_stdin(&mut cmd, input),
        None => cmd.output().unwrap_or_else(|e| panic!("failed to run cobc on {}: {}", path.display(), e)),
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    let first = stderr.lines().next().unwrap_or("").to_string();
    if out.status.success() {
        return Run::Outcome("ok".to_string(), out.stdout, out.stderr);
    }
    // `ok(s)` with s != 0 (CHG-0040): main's status, nothing on stderr.
    if out.stderr.is_empty() {
        return Run::Outcome(format!("exit {}", out.status.code().unwrap_or(-1)), out.stdout, out.stderr);
    }
    if out.status.code() == Some(3) {
        return Run::Skipped(first.trim_start_matches("unsupported: ").to_string());
    }
    if first.contains("parse error at") || first.contains("lex error at") {
        return Run::Outcome("parse-error".to_string(), out.stdout, out.stderr);
    }
    if out.status.code() == Some(2) {
        return Run::Outcome(format!("cobc failed: {}", first), out.stdout, out.stderr);
    }
    let diag = first.split_whitespace().find(|t| t.starts_with("diag.")).unwrap_or("?").to_string();
    let phase = if first.contains("(static)") {
        "static"
    } else if first.contains("(dynamic)") {
        "dynamic"
    } else {
        "?"
    };
    Run::Outcome(format!("{} ({})", diag, phase), out.stdout, out.stderr)
}

#[test]
fn compiled_conformance_suite_matches_headers() {
    let mut failures = Vec::new();
    for (cc, (passed, skipped, fails)) in oracle::per_compiler(conformance_suite) {
        eprintln!("compiled_suite [{}]: {} passed, {} skipped, {} failed", cc, passed, skipped.len(), fails.len());
        for s in &skipped {
            eprintln!("  skipped {}", s);
        }
        assert!(passed >= 20, "[{}] only {} cases compiled and passed", cc, passed);
        failures.extend(fails.into_iter().map(|f| format!("[{}] {}", cc, f)));
    }
    assert!(failures.is_empty(), "{} compiled case(s) disagree with their headers:\n{}", failures.len(), failures.join("\n"));
}

// One compiler's run of the conformance suite: passed, skipped, failures.
fn conformance_suite(cc: &str) -> (usize, Vec<String>, Vec<String>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../conformance");
    let mut files = Vec::new();
    find_cb_files(&root, &mut files);
    files.sort();
    let mut passed = 0;
    let mut skipped: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for path in &files {
        let src = fs::read_to_string(path).unwrap();
        if src.lines().next().map_or(false, |l| l.trim() == "// CONFORMANCE-FIXTURE") {
            continue;
        }
        let Some((_, expect)) = parse_expect(&src) else { continue };
        let stdin = oracle::case_stdin(&src).or_else(|| oracle::is_cobc_only(&src).then(Vec::new));
        match run_case(cc, path, stdin.as_deref()) {
            Run::Skipped(why) => skipped.push(format!("{}: {}", path.strip_prefix(&root).unwrap().display(), why)),
            Run::Outcome(actual, stdout, stderr) => {
                if actual != expect {
                    failures.push(format!("{}: expected `{}` but observed `{}`", path.display(), expect, actual));
                } else if let Some(d) = match oracle::is_cobc_only(&src) {
                    true => output_differs_from_header(&src, &stdout, &stderr),
                    false => output_differs(path, &stdout, &stderr),
                } {
                    failures.push(format!("{}: {}", path.display(), d));
                } else {
                    passed += 1;
                }
            }
        }
    }
    (passed, skipped, failures)
}

#[test]
fn compiled_examples_run_ok_or_are_skipped() {
    let mut failures = Vec::new();
    for (cc, (ran, skipped, fails)) in oracle::per_compiler(examples) {
        eprintln!("compiled examples [{}]: {} ran ok, {} skipped", cc, ran, skipped);
        failures.extend(fails.into_iter().map(|f| format!("[{}] {}", cc, f)));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// One compiler's run of the examples: ran, skipped, failures.
fn examples(cc: &str) -> (usize, usize, Vec<String>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cobaltc_examples");
    let mut paths = Vec::new();
    find_cb_files(&dir, &mut paths);
    paths.sort();
    let mut failures = Vec::new();
    let (mut ran, mut skipped) = (0, 0);
    for path in &paths {
        let src = fs::read_to_string(path).unwrap();
        if !src.lines().any(|l| l.starts_with("fn main")) {
            continue;
        }
        match run_case(cc, path, None) {
            Run::Skipped(_) => skipped += 1,
            Run::Outcome(o, stdout, stderr) if o == "ok" => match output_differs(path, &stdout, &stderr) {
                None => ran += 1,
                Some(d) => failures.push(format!("{}: {}", path.display(), d)),
            },
            Run::Outcome(o, _, _) => failures.push(format!("{}: {}", path.display(), o)),
        }
    }
    (ran, skipped, failures)
}
