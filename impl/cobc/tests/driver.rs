// The `cobc` driver itself: what it does with its output path.

use std::fs;
use std::process::Command;

// `cobc foo.cb` writes `foo`; when `foo` is a directory (one holding the
// program's module files, say), it must say so and write nothing, not
// leave the linker to fail with "cannot open output file".
#[test]
fn default_output_that_is_a_directory_is_reported() {
    let dir = std::env::temp_dir().join(format!("cobc-driver-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("prog")).unwrap();
    fs::write(dir.join("prog.cb"), "import std;\n\nfn main()\n{\n    printf(\"hi\\n\");\n}\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).arg("prog.cb").current_dir(&dir).output().expect("run cobc");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", err);
    assert!(err.contains("that is a directory") && err.contains("-o"), "stderr: {}", err);
    assert!(!dir.join("prog.c").exists(), "the C file was written anyway");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn about_shows_the_banner_and_explains_cobc() {
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).arg("--about").output().expect("run cobc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("                            C O B A L T C\n"), "{}", text);
    assert!(text.contains("the CobaltC compiler") && text.contains("coby"), "{}", text);
}

// `--check`: the front end and the `extern "…";` files; no C is written,
// compiled or linked.
#[test]
fn check_builds_nothing_and_checks_extern_files() {
    let dir = std::env::temp_dir().join(format!("cobc-check-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let cobc = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_cobc")).args(args).current_dir(&dir).output().expect("run cobc");
    fs::write(dir.join("ok.cb"), "import std;\n\nfn main()\n{\n    printf(\"hi\\n\");\n}\n").unwrap();
    let out = cobc(&["--check", "ok.cb"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
    let left: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(left, vec![std::ffi::OsString::from("ok.cb")], "cobc --check wrote files");

    fs::write(dir.join("ext.cb"), "extern \"./gone.c\";\n\nfn main()\n{\n}\n").unwrap();
    let out = cobc(&["--check", "ext.cb"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", err);
    assert!(err.contains("extern \"./gone.c\": no such file") && err.contains("ext.cb:1"), "stderr: {}", err);

    // A rejected program: the static diagnostic a build prints, exit 1.
    fs::write(dir.join("bad.cb"), "fn main()\n{\n    i32 x = true;\n}\n").unwrap();
    let out = cobc(&["--check", "bad.cb"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stderr, cobc(&["bad.cb"]).stderr);

    // No other option goes with it.
    assert_eq!(cobc(&["--check", "-O2", "ok.cb"]).status.code(), Some(2));
    assert_eq!(cobc(&["--check"]).status.code(), Some(2));
    let _ = fs::remove_dir_all(&dir);
}

// `--report-tracking`: nothing is compiled; the lines that still carry
// run-time tracking are listed, a loop's first, and a confined loop is
// not among them. (`v` is moved into another binding after its loop, so
// no analysis can prove its loop's reads; `w` is confined.)
#[test]
fn report_tracking_lists_tracked_lines_and_compiles_nothing() {
    let dir = std::env::temp_dir().join(format!("cobc-report-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = "import std;\n\nfn main()\n{\n    Vec<i64> v = Vec::new();\n    Vec::push(&mut v, 1);\n    Vec::push(&mut v, 2);\n    i64 first = 7;\n    i64 t = 0;\n    foreach (i in 0..Vec::len(&v))\n    {\n        t += v[i];\n    }\n    Vec<i64> w = Vec::new();\n    Vec::push(&mut w, 3);\n    foreach (i in 0..Vec::len(&w))\n    {\n        t += w[i];\n    }\n    Vec<i64> keep = v;\n    printf(\"%v %v %v\\n\", t, first, Vec::len(&keep));\n}\n";
    fs::write(dir.join("prog.cb"), src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).args(["--report-tracking", "prog.cb"]).current_dir(&dir).output().expect("run cobc");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let first = text.lines().nth(2).unwrap_or("");
    assert!(first.contains("prog.cb:12") && first.contains("loop") && first.contains("t += v[i];"), "{}", text);
    assert!(!text.contains("prog.cb:18 "), "the confined loop is listed:\n{}", text);
    assert!(!dir.join("prog").exists() && !dir.join("prog.c").exists(), "something was compiled");
    let _ = fs::remove_dir_all(&dir);
}

// Linkage under the object cache: the program's own functions are
// `static` (the C compiler may inline them), `main` is not, and a program
// whose loop calls cached `std` functions (`Vec::push`, `Vec::len`, the
// element accessor, given inline-only copies) builds and runs through the
// cache as without it.
#[test]
fn program_functions_are_static_and_cached_helpers_link() {
    let dir = std::env::temp_dir().join(format!("cobc-linkage-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = "import std;\n\nfn step(ref<i64, exclusive> x)\n{\n    *x += 1;\n}\n\nfn main()\n{\n    Vec<i64> v = Vec::new();\n    foreach (i in 0..10)\n    {\n        Vec::push(&mut v, narrow<i64>(i));\n    }\n    i64 t = 0;\n    foreach (i in 0..Vec::len(&v))\n    {\n        step(&mut t);\n        t += v[i];\n    }\n    printf(\"%v\\n\", t);\n}\n";
    fs::write(dir.join("prog.cb"), src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).args(["--keep-c", "-o", "prog", "prog.cb"]).current_dir(&dir).output().expect("run cobc");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let c = fs::read_to_string(dir.join("prog.c")).unwrap();
    assert!(c.lines().any(|l| l.starts_with("static void f_step(")), "`step` is not static");
    assert!(c.lines().any(|l| l.starts_with("void f_main(")), "`main` is not external");
    let run = Command::new(dir.join("prog")).output().expect("run prog");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "55\n");
    let _ = fs::remove_dir_all(&dir);
}
