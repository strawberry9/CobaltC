// The `coby` command line itself.

use std::process::Command;

#[test]
fn about_explains_coby_and_cobc() {
    let out = Command::new(env!("CARGO_BIN_EXE_coby")).arg("--about").output().expect("run coby");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("reference interpreter") && text.contains("cobc"), "{}", text);
    assert!(!text.contains('\r'), "the art kept a carriage return");
}

#[test]
fn usage_names_about() {
    let out = Command::new(env!("CARGO_BIN_EXE_coby")).output().expect("run coby");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("coby --about"));
}

// `--check`: the static pass alone, on files and directories.

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("coby-check-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn coby_in(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_coby")).args(args).current_dir(dir).output().expect("run coby")
}

// The static pass checks `std`'s items as a program reaches them; this
// is where `std` itself is verified whole: with `COBALTC_CHECK_ALL=1`
// every item is checked, and an empty program must still be accepted.
#[test]
fn std_is_well_typed_as_a_whole() {
    let dir = scratch("check-all");
    std::fs::write(dir.join("empty.cb"), "fn main()\n{\n}\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_coby")).arg("empty.cb").current_dir(&dir).env("COBALTC_CHECK_ALL", "1").output().expect("run coby");
    assert!(out.status.success(), "std does not check as a whole:\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn check_runs_nothing() {
    let dir = scratch("runs-nothing");
    std::fs::write(
        dir.join("p.cb"),
        "import std;\nimport std::fs;\n\nfn main()\n{\n    printf(\"ran\\n\");\n    _ = write_file(\"made.txt\", \"x\");\n}\n",
    )
    .unwrap();
    let out = coby_in(&dir, &["--check", "p.cb"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
    assert!(!dir.join("made.txt").exists(), "the program ran");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_prints_what_a_run_prints() {
    let dir = scratch("same-diagnostic");
    std::fs::write(dir.join("bad.cb"), "fn main()\n{\n    i32 x = true;\n}\n").unwrap();
    let ran = coby_in(&dir, &["bad.cb"]);
    let checked = coby_in(&dir, &["--check", "bad.cb"]);
    assert_eq!(checked.status.code(), Some(1));
    assert_eq!(ran.stderr, checked.stderr);
    assert!(checked.stdout.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_directory_checks_module_files_with_their_program() {
    let dir = scratch("modules");
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    std::fs::create_dir_all(dir.join(".hidden")).unwrap();
    std::fs::write(dir.join("prog.cb"), "module m \"./lib/m.cb\";\n\nfn main()\n{\n    m::f();\n}\n").unwrap();
    std::fs::write(dir.join("lib/m.cb"), "export fn f()\n{\n}\n").unwrap();
    std::fs::write(dir.join(".hidden/bad.cb"), "fn main(\n").unwrap();
    let out = coby_in(&dir, &["--check", "."]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    // Named on its own, a module file is checked as a program: it has no `main`.
    let out = coby_in(&dir, &["--check", "prog.cb", "lib/m.cb"]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("m.cb: rejected") && err.contains("diag.no-main"), "{}", err);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_reports_every_file_and_a_missing_one() {
    let dir = scratch("every-file");
    std::fs::write(dir.join("a.cb"), "fn main()\n{\n    i32 x = true;\n}\n").unwrap();
    std::fs::write(dir.join("b.cb"), "fn main()\n{\n    i32 y = \"s\";\n}\n").unwrap();
    let out = coby_in(&dir, &["--check", "a.cb", "b.cb", "gone.cb"]);
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("a.cb: rejected") && err.contains("b.cb: rejected"), "{}", err);
    assert!(err.contains("coby: gone.cb: no such file or directory"), "{}", err);
    let out = coby_in(&dir, &["--check"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("coby --check"));
    let _ = std::fs::remove_dir_all(&dir);
}
