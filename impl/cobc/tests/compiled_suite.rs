// The file-based conformance suite and the examples, compiled and run
// by `cobc --run` instead of interpreted by `coby` (impl/COBC-PLAN.md
// §4), once under each C compiler (`oracle::c_compilers`: GCC and
// Clang). Every case's outcome must match its header exactly as under
// `coby`, and its stdout and stderr (the diagnostic, location included)
// must be `coby`'s; a case `cobc` cannot compile yet (exit 3, `unsupported: …`)
// is counted as skipped and listed, so the gap is visible, not hidden;
// an internal error in `cobc` fails the case.
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

// A run made here and not yet kept in the store: its key and exit code
// (D-0307).
type Fresh = Option<(String, i32)>;

fn run_case(cc: &str, path: &Path, stdin: Option<&[u8]>) -> (Run, Fresh) {
    let mut cmd = oracle::cobc(cc);
    let src = fs::read_to_string(path).unwrap_or_default();
    cmd.current_dir(oracle::case_dir(path)).args(oracle::case_flags(&src)).arg("--run").arg(path).args(oracle::case_args_os(&src).expect("cobc runs on Linux"));
    let (out, k) = oracle::cobc_output(cc, path, cmd, stdin);
    let fresh = k.map(|k| (k, oracle::store::code_of(&out.status)));
    (classify(out), fresh)
}

fn classify(out: std::process::Output) -> Run {
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
        // An internal error is a fault of `cobc`, not a gap it declares.
        if first.contains("internal error in cobc") {
            return Run::Outcome(format!("cobc failed: {}", first), out.stdout, out.stderr);
        }
        return Run::Skipped(first.trim_start_matches("unsupported: ").to_string());
    }
    if out.status.code() == Some(2) {
        return Run::Outcome(format!("cobc failed: {}", first), out.stdout, out.stderr);
    }
    let (diag, phase) = coby::diagnostics::first_line_id(&first).unwrap_or(("?", "?"));
    let diag = diag.to_string();
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
        assert!(passed * oracle::shard_count() >= 20, "[{}] only {} cases compiled and passed", cc, passed);
        failures.extend(fails.into_iter().map(|f| format!("[{}] {}", cc, f)));
    }
    assert!(failures.is_empty(), "{} compiled case(s) disagree with their headers:\n{}", failures.len(), failures.join("\n"));
}

// One compiler's run of the conformance suite: passed, skipped, failures.
fn conformance_suite(cc: &str) -> (usize, Vec<String>, Vec<String>) {
    let root = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../conformance")).expect("impl/conformance");
    let mut files = Vec::new();
    find_cb_files(&root, &mut files);
    files.sort();
    let mut passed = 0;
    let mut skipped: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    let mut slowest = oracle::Slowest::new();
    for (index, path) in files.iter().enumerate() {
        let src = fs::read_to_string(path).unwrap();
        if src.lines().next().map_or(false, |l| l.trim() == "// CONFORMANCE-FIXTURE") {
            continue;
        }
        if !oracle::in_shard(index) {
            continue;
        }
        let Some((_, expect)) = parse_expect(&src) else { continue };
        if oracle::platform_skipped(&src) {
            skipped.push(format!("{}: its `platform:` header names another platform", path.strip_prefix(&root).unwrap().display()));
            continue;
        }
        if let Some(why) = oracle::slow_skipped(&src) {
            skipped.push(format!("{}: slow ({}); COBALTC_TEST_SLOW=1 runs it", path.strip_prefix(&root).unwrap().display(), why));
            continue;
        }
        let stdin = oracle::case_stdin(&src).or_else(|| oracle::is_cobc_only(&src).then(Vec::new));
        let started = std::time::Instant::now();
        // Canonical, as compiled_spec_rows passes it: a diagnostic prints the
        // path as given, so both runners' runs of a file are the same run
        // (and the store keeps one, D-0307).
        let path = &fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        let (run, fresh) = run_case(cc, path, stdin.as_deref());
        match run {
            Run::Skipped(why) => skipped.push(format!("{}: {}", path.strip_prefix(&root).unwrap_or(path).display(), why)),
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
                    if let Some((k, code)) = fresh {
                        oracle::keep_cobc_output(&Some(k), code, &stdout, &stderr);
                    }
                }
            }
        }
        // The case's whole cost: compiling, running, and the oracle run.
        slowest.note(&path.strip_prefix(&root).unwrap_or(path).display().to_string(), started.elapsed());
    }
    slowest.report(&format!("compiled_suite [{}]", cc));
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
        match run_case(cc, path, None).0 {
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
