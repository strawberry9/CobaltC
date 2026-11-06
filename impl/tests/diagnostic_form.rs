// WI-9 (D-0228): the form a diagnostic is shown in, and its location.

use std::process::Command;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("coby-diag-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// Runs `coby ARGS` in `dir` and gives its stderr.
fn stderr_of(dir: &std::path::Path, args: &[&str], explain: bool) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_coby"));
    c.args(args).current_dir(dir).env_remove("COBALTC_EXPLAIN").env("NO_COLOR", "1");
    if explain {
        c.env("COBALTC_EXPLAIN", "1");
    }
    let out = c.output().expect("run coby");
    assert_eq!(out.status.code(), Some(1), "{:?}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn static_diagnostic_names_the_column_and_shows_the_line() {
    let dir = scratch("static");
    std::fs::write(dir.join("s.cb"), "import std;\n\nfn main()\n{\n    bool b = true;\n    i32 x = 1 + b;\n}\n").unwrap();
    let err = stderr_of(&dir, &["s.cb"], false);
    assert_eq!(
        err,
        "error[diag.type-mismatch]: the operands are `i32` and `bool`; convert one (`widen`, `narrow`, `to_float`)\n --> s.cb:6:13\n  |\n6 |     i32 x = 1 + b;\n  |             ^\n  = repair: convert explicitly (`rule.arith.convert`) or fix the types\n"
    );
}

#[test]
fn dynamic_diagnostic_is_located_by_its_line() {
    let dir = scratch("dynamic");
    std::fs::write(dir.join("d.cb"), "import std;\n\nfn main()\n{\n    i32 a = 2147483647;\n    i32 b = narrow<i32>(arg_count()) + 1;\n    printf(\"%v\\n\", a + b);\n}\n").unwrap();
    let err = stderr_of(&dir, &["d.cb"], false);
    assert!(err.starts_with("fault[diag.arith-overflow]: arithmetic overflow\n --> d.cb:7:5\n  |\n7 |     printf(\"%v\\n\", a + b);\n  |     ^^^^^^\n  = repair: "), "{}", err);
}

#[test]
fn syntax_error_names_its_column() {
    let dir = scratch("syntax");
    std::fs::write(dir.join("y.cb"), "import std;\n\nfn main()\n{\n    i32 x = 1 +;\n}\n").unwrap();
    let err = stderr_of(&dir, &["y.cb"], false);
    assert!(err.starts_with("error[diag.syntax-error]: expected an expression, found `;`\n --> y.cb:5:16\n  |\n5 |     i32 x = 1 +;\n  |                ^\n"), "{}", err);
}

#[test]
fn fault_inside_std_is_reported_at_the_programs_line() {
    let dir = scratch("std");
    std::fs::write(dir.join("u.cb"), "import std;\n\nfn main()\n{\n    Option<i32> o = None;\n    i32 x = Option::unwrap(o);\n    printf(\"%v\\n\", x);\n}\n").unwrap();
    let err = stderr_of(&dir, &["u.cb"], false);
    assert!(err.starts_with("fault[diag.unwrap-failed]: "), "{}", err);
    assert!(err.contains(" --> u.cb:6:5\n"), "{}", err);
}

#[test]
fn file_backed_module_names_its_own_file_and_column() {
    let dir = scratch("module");
    std::fs::write(dir.join("main.cb"), "import std;\n\nmodule m \"./m.cb\";\n\nfn main()\n{\n    m::go();\n}\n").unwrap();
    std::fs::write(dir.join("m.cb"), "export fn go()\n{\n    bool b = true;\n    i32 x = 1 + b;\n}\n").unwrap();
    let err = stderr_of(&dir, &["main.cb"], false);
    assert!(err.contains(" --> m.cb:4:13\n  |\n4 |     i32 x = 1 + b;\n  |             ^\n"), "{}", err);
}

#[test]
fn tabs_and_wide_characters_keep_the_caret_under_the_token() {
    let dir = scratch("tabs");
    std::fs::write(dir.join("t.cb"), "import std;\n\nfn main()\n{\n\tStringView v = StringView::of(\"éé\"); bool b = true; i32 x = 1 + b;\n}\n").unwrap();
    let err = stderr_of(&dir, &["t.cb"], false);
    // `1` is the 62nd character of the line and its 64th byte (a tab and
    // two two-byte `é` before it): the column counts characters, and the
    // caret's padding keeps the tab and is a space for each other one.
    assert!(err.contains(" --> t.cb:5:62\n"), "{}", err);
    assert!(err.contains(&format!("  | \t{}^\n", " ".repeat(60))), "{}", err);
}

#[test]
fn a_character_that_is_not_ascii_is_named_whole() {
    let dir = scratch("utf8");
    std::fs::write(dir.join("n.cb"), "fn main()\n{\n    i32 é = 1;\n}\n").unwrap();
    let err = stderr_of(&dir, &["n.cb"], false);
    assert!(err.starts_with("error[diag.syntax-error]: unexpected character 'é': a name is ASCII"), "{}", err);
    assert!(err.contains(" --> n.cb:3:9\n"), "{}", err);
}

#[test]
fn explain_adds_the_registry_entry() {
    let dir = scratch("explain");
    std::fs::write(dir.join("e.cb"), "import std;\n\nfn main()\n{\n    i32 x = 2147483647 + 1;\n}\n").unwrap();
    let short = stderr_of(&dir, &["e.cb"], false);
    assert!(!short.contains("rule:"), "{}", short);
    for long in [stderr_of(&dir, &["--explain", "e.cb"], false), stderr_of(&dir, &["e.cb"], true)] {
        assert!(long.starts_with(&short), "{}", long);
        assert!(long.contains("\n  rule:      `[Arith-Checked-Overflow]`"), "{}", long);
        assert!(long.contains("  see: spec/examples.md -> "), "{}", long);
    }
}
