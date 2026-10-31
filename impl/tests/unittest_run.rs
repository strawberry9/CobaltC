// D-0230: `coby --unittest`, the report of a test run and its status.

use std::process::Command;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("coby-unittest-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn coby(dir: &std::path::Path, args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_coby")).args(args).current_dir(dir).env("NO_COLOR", "1").env_remove("COBALTC_EXPLAIN").output().expect("run coby");
    (out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

const PROGRAM: &str = "import std;\n\nfn twice(i32 x) : i32\n{\n    x * 2\n}\n\nunittest \"twice: small\"\n{\n    assert(twice(2) == 4);\n}\n\nunittest\n{\n    assert(twice(3) == 7, \"3 twice is %v\", twice(3));\n}\n\nunittest \"twice: overflow faults\"\n{\n    i32 big = 2147483647;\n    i32 _ = twice(big);\n}\n\nunittest \"twice: zero\"\n{\n    assert(twice(0) == 0);\n}\n\nfn main()\n{\n    printf(\"main\\n\");\n}\n";

#[test]
fn a_test_run_reports_each_test_and_goes_on_after_a_failure() {
    let dir = scratch("report");
    std::fs::write(dir.join("t.cb"), PROGRAM).unwrap();
    let (code, out, err) = coby(&dir, &["--unittest", "t.cb"]);
    assert_eq!(code, Some(1), "{}{}", out, err);
    assert_eq!(err, "");
    assert_eq!(
        out,
        "test t.cb:8 twice: small ... ok\n\
         test t.cb:13 ... FAILED\n\
         fault[diag.assert-failed]: 3 twice is 6\n  --> t.cb:15:5\n   |\n15 |     assert(twice(3) == 7, \"3 twice is %v\", twice(3));\n   |     ^^^^^^\n   = repair: the program's own condition names what was assumed; find why it did not hold\n\
         test t.cb:18 twice: overflow faults ... FAILED\n\
         fault[diag.arith-overflow]: arithmetic overflow\n --> t.cb:5:5\n  |\n5 |     x * 2\n  |     ^\n  = repair: widen, or use `wrapping_`/`saturating_`/`checked_`\n\
         test t.cb:24 twice: zero ... ok\n\
         4 tests: 2 passed, 2 failed\n"
    );
    // The program itself runs `main`, and no test.
    assert_eq!(coby(&dir, &["t.cb"]), (Some(0), "main\n".to_string(), String::new()));
}

#[test]
fn a_filter_runs_only_the_tests_whose_name_contains_it() {
    let dir = scratch("filter");
    std::fs::write(dir.join("t.cb"), PROGRAM).unwrap();
    let (code, out, _) = coby(&dir, &["--unittest", "--unittest-filter", "twice: ", "t.cb"]);
    assert_eq!(code, Some(1));
    assert!(out.ends_with("3 tests: 2 passed, 1 failed, 1 filtered out\n"), "{}", out);
    let (code, out, _) = coby(&dir, &["--unittest-filter", "zero", "--unittest", "t.cb"]);
    assert_eq!((code, out.as_str()), (Some(0), "test t.cb:24 twice: zero ... ok\n1 test: 1 passed, 0 failed, 3 filtered out\n"));
    let (code, out, err) = coby(&dir, &["--unittest", "--unittest-filter", "nothing", "t.cb"]);
    assert_eq!((code, out.as_str(), err.as_str()), (Some(1), "", "unittest: no test's name contains \"nothing\" (4 tests in all)\n"));
}

#[test]
fn a_file_of_tests_needs_no_main() {
    let dir = scratch("nomain");
    std::fs::write(dir.join("lib.cb"), "import std;\n\nexport fn inc(i32 x) : i32\n{\n    x + 1\n}\n\nunittest \"inc\"\n{\n    assert(inc(1) == 2);\n}\n").unwrap();
    assert_eq!(coby(&dir, &["--unittest", "lib.cb"]), (Some(0), "test lib.cb:8 inc ... ok\n1 test: 1 passed, 0 failed\n".to_string(), String::new()));
    let (code, _, err) = coby(&dir, &["lib.cb"]);
    assert_eq!(code, Some(1));
    assert!(err.starts_with("error[diag.no-main]"), "{}", err);
}

#[test]
fn a_module_files_tests_run_where_it_is_declared() {
    let dir = scratch("modules");
    std::fs::write(dir.join("main.cb"), "import std;\n\nmodule m \"./m.cb\";\n\nunittest \"root\"\n{\n    assert(m::one() == 1);\n}\n\nfn main()\n{\n}\n").unwrap();
    std::fs::write(dir.join("m.cb"), "import std;\n\nfn hidden() : i32\n{\n    1\n}\n\nexport fn one() : i32\n{\n    hidden()\n}\n\nunittest \"hidden\"\n{\n    assert(hidden() == 1);\n}\n").unwrap();
    assert_eq!(coby(&dir, &["--unittest", "main.cb"]), (Some(0), "test m.cb:13 hidden ... ok\ntest main.cb:5 root ... ok\n2 tests: 2 passed, 0 failed\n".to_string(), String::new()));
}

#[test]
fn std_has_its_own_test_run() {
    let dir = scratch("std");
    std::fs::write(dir.join("e.cb"), "fn main()\n{\n}\n").unwrap();
    let (code, out, _) = coby(&dir, &["--unittest-std", "e.cb"]);
    assert_eq!(code, Some(0));
    assert!(out.ends_with(" passed, 0 failed\n"), "{}", out);
}
