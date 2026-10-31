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

// `--warn-shadow` (CHG-0333): a note per item of the program whose name
// hides an imported one, at its file and line; nothing else changes.
#[test]
fn warn_shadow_notes_hidden_imports() {
    let dir = scratch("warn-shadow");
    let src = "import std;\n\nfn min(i64 a, i64 b) : i64\n{\n    if (a < b) { a } else { b }\n}\n\nfn main()\n{\n    printf(\"%v\\n\", min(3: i64, 2: i64));\n}\n";
    std::fs::write(dir.join("shadow.cb"), src).unwrap();
    let out = coby_in(&dir, &["--warn-shadow", "shadow.cb"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "2\n");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("note: fn `min` of this program hides `std::min`"), "{}", err);
    assert!(err.contains("--> shadow.cb:3"), "{}", err);
    let out = coby_in(&dir, &["shadow.cb"]);
    assert!(out.status.success() && out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
    let out = coby_in(&dir, &["--warn-shadow", "--check", "shadow.cb"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stderr).contains("hides `std::min`"));
}

// `--check-syntax` (CHG-0346): whether one text parses, as cobfmt asks
// it; a program the static pass would reject still parses, and a module
// file is found relative to `--origin` when the text comes on standard
// input.
#[test]
fn dump_resolved_names_items_by_their_keys() {
    let dir = scratch("dump-resolved");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ok.cb"), "import std;\n\nmodule geo\n{\n    export fn area() : i32\n    {\n        2\n    }\n}\n\nfn main()\n{\n    printf(\"%v\\n\", geo::area());\n}\n").unwrap();
    std::fs::write(dir.join("hidden.cb"), "module geo\n{\n    fn area() : i32\n    {\n        2\n    }\n}\n\nfn main()\n{\n    i32 a = geo::area();\n}\n").unwrap();
    let out = coby_in(&dir, &["--dump-resolved", "ok.cb"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    // `printf` is `std::print($fmt(…))`; `geo::area` stays as it is written.
    assert!(text.contains("(path @13:5 std::print <>)") && text.contains("(path @13:5 $fmt <>)") && text.contains("(path @13:20 geo::area <>)"), "{}", text);
    // `--dump-folded`: a constant's use is its literal, typed by its suffix.
    std::fs::write(dir.join("konst.cb"), "const u32 N = 6 * 7;\n\nfn main() : u8\n{\n    u32 n = N;\n    narrow<u8>(n)\n}\n").unwrap();
    let out = coby_in(&dir, &["--dump-folded", "konst.cb"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("(let u32 n @5:5\n      (int @5:0 42 u32)"), "{}", String::from_utf8_lossy(&out.stdout));
    let out = coby_in(&dir, &["--dump-resolved", "hidden.cb"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&out.stderr), "hidden.cb:11: diag.name-not-visible: `geo::area` is not exported from its module\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_syntax_parses_only() {
    let dir = scratch("check-syntax");
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    std::fs::write(dir.join("lib/m.cb"), "export fn f()\n{\n}\n").unwrap();
    std::fs::write(dir.join("prog.cb"), "module m \"./lib/m.cb\";\n\nfn main()\n{\n    i32 x = true;\n}\n").unwrap();
    std::fs::write(dir.join("bad.cb"), "fn main()\n{\n    x = ;\n}\n").unwrap();
    let out = coby_in(&dir, &["--check-syntax", "prog.cb"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
    let out = coby_in(&dir, &["--check-syntax", "bad.cb"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&out.stderr), "bad.cb:3: expected an expression, found `;` (column 9)\n");
    let piped_in = |cwd: &std::path::Path, args: &[&str], text: &str| {
        use std::io::Write;
        let mut child = Command::new(env!("CARGO_BIN_EXE_coby"))
            .args(args)
            .current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("run coby");
        child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
        child.wait_with_output().unwrap()
    };
    let piped = |args: &[&str], text: &str| piped_in(&dir, args, text);
    // A module's syntax error is reported where it is, found through the origin.
    std::fs::write(dir.join("lib/m.cb"), "export fn f(\n{\n}\n").unwrap();
    let text = std::fs::read_to_string(dir.join("prog.cb")).unwrap();
    let out = piped(&["--check-syntax", "--origin", "prog.cb", "-"], &text);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("m.cb:2:"), "{}", String::from_utf8_lossy(&out.stderr));
    // Without one, a module is found from the current directory; where it
    // cannot be found, the text is parsed alone.
    let out = piped(&["--check-syntax", "-"], &text);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("m.cb:2:"), "{}", String::from_utf8_lossy(&out.stderr));
    let out = piped_in(&dir.join("lib"), &["--check-syntax", "-"], &text);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(coby_in(&dir, &["--check-syntax", "gone.cb"]).status.code(), Some(2));
    assert_eq!(coby_in(&dir, &["--check-syntax"]).status.code(), Some(2));
    assert_eq!(coby_in(&dir, &["--check-syntax", "a.cb", "b.cb"]).status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}

// D-0299: `--certificate FILE` with `--check`, and with a run: what the
// static pass decided, body by body, the file written either way.
#[test]
fn certificate_is_written_by_check_and_by_a_run() {
    let dir = scratch("certificate");
    std::fs::write(dir.join("prog.cb"), "import std;\n\nfn bump(ref<i32, exclusive> p)\n{\n    *p = *p + 1;\n}\n\nfn main()\n{\n    i32 x = 1;\n    bump(&mut x);\n    printf(\"%v\\n\", x);\n}\n").unwrap();
    let out = coby_in(&dir, &["--certificate", "c.txt", "--check", "prog.cb"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(dir.join("c.txt")).unwrap();
    assert!(text.starts_with("CobaltC certificate 1\ntool: coby\nprogram: prog.cb\nverdict: static pass: every body decided ("), "{}", text);
    assert!(text.contains("\nfn main: decided\n") && text.contains("\nfn bump: decided\n"), "{}", text);
    let out = coby_in(&dir, &["--certificate", "c2.txt", "prog.cb"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "2\n");
    let text = std::fs::read_to_string(dir.join("c2.txt")).unwrap();
    assert!(text.contains("\nfn main: decided\n"), "{}", text);
    // A rejected program names its diagnostic; a directory is refused.
    std::fs::write(dir.join("bad.cb"), "import std;\n\nfn main()\n{\n    i32 x = 1;\n    auto r = &mut x;\n    printf(\"%v\\n\", x);\n    *r = 2;\n}\n").unwrap();
    let out = coby_in(&dir, &["--certificate", "c3.txt", "--check", "bad.cb"]);
    assert_eq!(out.status.code(), Some(1));
    let text = std::fs::read_to_string(dir.join("c3.txt")).unwrap();
    assert!(text.contains("\nverdict: rejected: diag.aliasing-conflict\n"), "{}", text);
    let out = coby_in(&dir, &["--certificate", "c4.txt", "--check", "."]);
    assert_eq!(out.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}
