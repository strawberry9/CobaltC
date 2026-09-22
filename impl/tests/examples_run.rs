// Every program in impl/cobaltc_examples/ runs to completion through
// the real binary. A file without a root `fn main` (a module body such
// as 13_geometry.cb) is not a program and is skipped; the file that
// declares it exercises it.

use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn every_example_program_runs_ok() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("cobaltc_examples");
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("cb"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no examples under {}", dir.display());

    let mut failures = Vec::new();
    let mut ran = 0;
    for path in &paths {
        let src = fs::read_to_string(path).unwrap();
        if !src.lines().any(|l| l.starts_with("fn main")) {
            continue;
        }
        ran += 1;
        let out = Command::new(env!("CARGO_BIN_EXE_coby")).arg(path).output().unwrap();
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            failures.push(format!("{}: {}", path.display(), err.lines().take(2).collect::<Vec<_>>().join(" | ")));
        }
    }
    eprintln!("examples_run: {} programs run, {} skipped (no root main)", ran, paths.len() - ran);
    assert!(failures.is_empty(), "{} example(s) failed:\n{}", failures.len(), failures.join("\n"));
}
