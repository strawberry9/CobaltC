// Runner for the file-based .cb conformance suite under impl/conformance/.
//
// This is a separate, complementary artifact to tests/conformance.rs
// (which embeds its cases as inline Rust string literals): every case
// here is a real, standalone, Allman-formatted .cb file on disk,
// individually readable and runnable without this test harness at
// all -- `coby impl/conformance/06-arithmetic/some_case.cb` works
// exactly the same way outside `cargo test` as inside it.
//
// Every case's `expect:` header was set by actually running the file
// through the real built binary and recording the real observed
// outcome, never guessed -- see each directory's git log for the
// specific verification.
//
// A .cb file whose first line is `// CONFORMANCE-FIXTURE` is not a
// case: it is the body of a file-backed module (spec/17 §5) that some
// case declares with `module m "…";`, and is skipped here rather than
// run on its own (CHG-0026, "Conformance changes").
//
// A case with a `cobc-only:` header uses what only `cobc` provides --
// the program's own C code (`rule.trust.extern-code`): here `coby` must
// refuse it (exit 3, `unsupported: …`), and its `expect:` is checked by
// cobc/tests/compiled_suite.rs (CHG-0039).
//
// A case's `stdin-hex:` header is its standard input (CHG-0038, D-0050);
// a case without one gets none, so a read sees the end of input.
//
// A case's `args:` header gives the program's arguments, and `expect:
// exit N` is a run that completes with exit status N (CHG-0040).
//
// `coby` calls C functions (`extern fn`, `src/ffi.rs`) only on x86-64
// Linux. Elsewhere a case that calls one is refused, which is how that
// implementation documents it (`spec/20` §3 leaves calling to it), so
// the case is skipped there, saying so; on x86-64 Linux it must pass.

mod common;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct CaseHeader {
    kind: String,
    expect: String,
    expect_stdout_hex: Option<String>,
    cobc_only: bool,
    stdin: Vec<u8>,
}

fn parse_header(src: &str, path: &Path) -> CaseHeader {
    let mut fields: HashMap<String, String> = HashMap::new();
    for line in src.lines() {
        let line = line.trim_start();
        if !line.starts_with("//") {
            break;
        }
        let body = line.trim_start_matches('/').trim();
        if let Some((key, val)) = body.split_once(':') {
            let key = key.trim().to_string();
            let val = val.trim().to_string();
            if matches!(key.as_str(), "spec" | "facility" | "kind" | "expect" | "expect-stdout-hex" | "stdin-hex" | "cobc-only" | "args") {
                fields.insert(key, val);
            }
        }
    }
    let kind = fields
        .remove("kind")
        .unwrap_or_else(|| panic!("{}: missing required header field `kind:`", path.display()));
    let expect = fields
        .remove("expect")
        .unwrap_or_else(|| panic!("{}: missing required header field `expect:`", path.display()));
    let expect_stdout_hex = fields.remove("expect-stdout-hex");
    let cobc_only = fields.contains_key("cobc-only");
    let stdin = fields.remove("stdin-hex").map(|h| (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).expect("stdin-hex: not hex")).collect()).unwrap_or_default();
    CaseHeader { kind, expect, expect_stdout_hex, cobc_only, stdin }
}

fn find_cb_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e));
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            find_cb_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("cb") {
            out.push(path);
        }
    }
}

// Runs one .cb file through the real binary and returns
// (actual_outcome_string, stdout_bytes). actual_outcome_string is one
// of: "ok", "exit N", "parse-error", or "diag.<name> (<static|dynamic>)"
// -- the same vocabulary the `expect:` header uses, so callers can
// compare directly.
fn run_case(path: &Path, args: &[std::ffi::OsString], stdin: &[u8]) -> (String, Vec<u8>) {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_coby"))
        .current_dir(common::case_dir(path))
        .arg(path)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to run coby on {}: {}", path.display(), e));
    // A program that stops before reading all of it closes the pipe.
    let _ = child.stdin.take().unwrap().write_all(stdin);
    let out = child.wait_with_output().unwrap_or_else(|e| panic!("failed to run coby on {}: {}", path.display(), e));
    if out.status.success() {
        return ("ok".to_string(), out.stdout);
    }
    if !COBY_CALLS_C && out.status.code() == Some(3) && String::from_utf8_lossy(&out.stderr).contains(NO_C_CALLS) {
        return (SKIPPED_NO_C.to_string(), out.stdout);
    }
    // `ok(s)` with s != 0: main's status, and nothing on stderr.
    if out.stderr.is_empty() {
        return (format!("exit {}", out.status.code().unwrap_or(-1)), out.stdout);
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let first_line = stderr.lines().next().unwrap_or("");
    // `parse-error` covers both lexical and grammatical failures
    // (spec/22 §1-2); the interpreter labels them separately.
    if first_line.contains("parse error at") || first_line.contains("lex error at") {
        return ("parse-error".to_string(), out.stdout);
    }
    // First line looks like: "error: diag.some-name (static)" or "(dynamic)".
    let diag = first_line
        .split_whitespace()
        .find(|tok| tok.starts_with("diag."))
        .unwrap_or_else(|| panic!("{}: stderr's first line has no diag.* id and is not a parse error: {:?}", path.display(), first_line));
    let phase = if first_line.contains("(static)") {
        "static"
    } else if first_line.contains("(dynamic)") {
        "dynamic"
    } else {
        panic!("{}: stderr's first line names {} but no (static)/(dynamic) phase: {:?}", path.display(), diag, first_line);
    };
    (format!("{} ({})", diag, phase), out.stdout)
}

use common::{COBY_CALLS_C, NO_C_CALLS};
const SKIPPED_NO_C: &str = "skipped: no C calls here";

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[test]
fn cb_conformance_suite_all_cases_match_their_declared_expectation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance");
    let mut files = Vec::new();
    find_cb_files(&root, &mut files);
    assert!(!files.is_empty(), "no .cb files found under {}", root.display());
    files.sort();

    let mut failures: Vec<String> = Vec::new();
    let mut positive_count = 0usize;
    let mut negative_count = 0usize;
    let mut fixture_count = 0usize;

    for path in &files {
        let src = fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
        if src.lines().next().map_or(false, |l| l.trim() == "// CONFORMANCE-FIXTURE") {
            fixture_count += 1;
            continue;
        }
        let header = parse_header(&src, path);
        match header.kind.as_str() {
            "positive" => positive_count += 1,
            "negative" => negative_count += 1,
            other => failures.push(format!("{}: unknown kind `{}` (must be `positive` or `negative`)", path.display(), other)),
        }

        if header.cobc_only {
            let out = Command::new(env!("CARGO_BIN_EXE_coby"))
                .arg(path)
                .output()
                .unwrap_or_else(|e| panic!("failed to run coby on {}: {}", path.display(), e));
            let stderr = String::from_utf8_lossy(&out.stderr);
            if out.status.code() != Some(3) || !stderr.starts_with("unsupported: ") {
                failures.push(format!("{}: cobc-only, so coby must refuse it (exit 3); observed exit {:?}: {}", path.display(), out.status.code(), stderr.trim()));
            }
            continue;
        }

        // An `args:` header this platform cannot pass (bytes that are not
        // UTF-8, on Windows) skips the case here; tests/spec_rows.rs still
        // runs it, handing the bytes to the interpreter directly.
        let Some(args) = common::case_args_os(&src) else {
            eprintln!("skipped {}: its arguments cannot be passed on this platform", path.display());
            continue;
        };
        let (actual, stdout) = run_case(path, &args, &header.stdin);
        if actual == SKIPPED_NO_C {
            eprintln!("skipped {}: it calls C, which coby does only on x86-64 Linux", path.display());
            continue;
        }
        if actual != header.expect {
            failures.push(format!(
                "{}: expected `{}` but observed `{}`",
                path.display(),
                header.expect,
                actual
            ));
            continue;
        }
        if let Some(expected_hex) = &header.expect_stdout_hex {
            let actual_hex = hex_encode(&stdout);
            if &actual_hex != expected_hex {
                failures.push(format!(
                    "{}: expect-stdout-hex mismatch: expected {} but observed {}",
                    path.display(),
                    expected_hex,
                    actual_hex
                ));
            }
        }
    }

    let case_count = files.len() - fixture_count;
    eprintln!(
        "cb_conformance_suite: {} cases ({} positive, {} negative), {} fixtures",
        case_count,
        positive_count,
        negative_count,
        fixture_count
    );

    assert!(
        failures.is_empty(),
        "{} of {} .cb conformance case(s) failed:\n{}",
        failures.len(),
        case_count,
        failures.join("\n")
    );
}
