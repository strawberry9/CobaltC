// cobfmt from the command line (`private/cobfmt-proposal.md` §5, §6, §8).

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn cobfmt(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cobfmt")).args(args).output().expect("run cobfmt")
}

fn cobfmt_stdin(args: &[&str], input: &str) -> Output {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_cobfmt"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run cobfmt");
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

// A fresh directory under the target directory, per test.
fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("cobfmt-{}", name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const MESSY: &str = "import std;\nfn main() {\n  i32 x=1+2;\n  if (x>2) { printf(\"%v\\n\",x); }\n}\n";
const TIDY: &str = "import std;\nfn main()\n{\n    i32 x = 1 + 2;\n    if (x > 2) { printf(\"%v\\n\", x); }\n}\n";

fn status(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

#[test]
fn no_arguments_prints_the_usage_and_exits_2() {
    let o = cobfmt(&[]);
    assert_eq!(status(&o), 2);
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("usage: cobfmt") && err.contains("--backup"), "{}", err);
}

#[test]
fn help_and_version() {
    let o = cobfmt(&["--help"]);
    assert_eq!(status(&o), 0);
    let out = String::from_utf8_lossy(&o.stdout);
    for flag in ["--help", "--backup", "--check", "--diff", "--stdout", "--width", "--no-wrap", "--quiet", "--version", "exit status"] {
        assert!(out.contains(flag), "--help lacks {}", flag);
    }
    let v = cobfmt(&["--version"]);
    assert_eq!(status(&v), 0);
    assert!(String::from_utf8_lossy(&v.stdout).starts_with("cobfmt "));
}

#[test]
fn unknown_option_is_a_usage_error() {
    let o = cobfmt(&["--frobnicate", "x.cb"]);
    assert_eq!(status(&o), 2);
    assert!(String::from_utf8_lossy(&o.stderr).contains("unknown option"));
}

#[test]
fn formats_in_place() {
    let d = scratch("inplace");
    let f = d.join("a.cb");
    std::fs::write(&f, MESSY).unwrap();
    let o = cobfmt(&[f.to_str().unwrap()]);
    assert_eq!(status(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), TIDY);
    // Already formatted: nothing changes, still 0.
    let o = cobfmt(&[f.to_str().unwrap()]);
    assert_eq!(status(&o), 0);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), TIDY);
}

#[test]
fn backup_keeps_the_previous_contents_and_never_overwrites() {
    let d = scratch("backup");
    let f = d.join("a.cb");
    // The author's own backup must not be touched.
    std::fs::write(d.join("a.cb.bak"), "mine\n").unwrap();
    std::fs::write(&f, MESSY).unwrap();
    assert_eq!(status(&cobfmt(&["--backup", f.to_str().unwrap()])), 0);
    assert_eq!(std::fs::read_to_string(d.join("a.cb.format.bak")).unwrap(), MESSY);
    assert_eq!(std::fs::read_to_string(d.join("a.cb.bak")).unwrap(), "mine\n");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), TIDY);
    // A second change: the next free name.
    std::fs::write(&f, "fn main() {\n}\n").unwrap();
    assert_eq!(status(&cobfmt(&["--backup", f.to_str().unwrap()])), 0);
    assert_eq!(std::fs::read_to_string(d.join("a.cb.format.bak.1")).unwrap(), "fn main() {\n}\n");
    assert_eq!(std::fs::read_to_string(d.join("a.cb.format.bak")).unwrap(), MESSY);
    // Unchanged files get no backup.
    assert_eq!(status(&cobfmt(&["--backup", f.to_str().unwrap()])), 0);
    assert!(!d.join("a.cb.format.bak.2").exists());
    // No temporary file is left behind.
    let left: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains(".tmp")).collect();
    assert!(left.is_empty());
}

#[test]
fn check_and_diff_write_nothing() {
    let d = scratch("check");
    let f = d.join("a.cb");
    std::fs::write(&f, MESSY).unwrap();
    let o = cobfmt(&["--check", f.to_str().unwrap()]);
    assert_eq!(status(&o), 1);
    assert!(String::from_utf8_lossy(&o.stdout).contains("a.cb"));
    let o = cobfmt(&["--diff", f.to_str().unwrap()]);
    assert_eq!(status(&o), 1);
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("+    i32 x = 1 + 2;") && out.contains("-  i32 x=1+2;"), "{}", out);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), MESSY);
    std::fs::write(&f, TIDY).unwrap();
    assert_eq!(status(&cobfmt(&["--check", f.to_str().unwrap()])), 0);
    assert_eq!(status(&cobfmt(&["--diff", f.to_str().unwrap()])), 0);
}

#[test]
fn standard_input_with_a_dash() {
    let o = cobfmt_stdin(&["-"], MESSY);
    assert_eq!(status(&o), 0);
    assert_eq!(String::from_utf8_lossy(&o.stdout), TIDY);
    let o = cobfmt_stdin(&["--check", "-"], MESSY);
    assert_eq!(status(&o), 1);
}

#[test]
fn stdout_leaves_the_file() {
    let d = scratch("stdout");
    let f = d.join("a.cb");
    std::fs::write(&f, MESSY).unwrap();
    let o = cobfmt(&["--stdout", f.to_str().unwrap()]);
    assert_eq!(status(&o), 0);
    assert_eq!(String::from_utf8_lossy(&o.stdout), TIDY);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), MESSY);
}

#[test]
fn a_file_that_does_not_parse_is_reported_and_left_alone() {
    let d = scratch("noparse");
    let f = d.join("bad.cb");
    let src = "fn main() {\n    i32 x = ;\n}\n";
    std::fs::write(&f, src).unwrap();
    let o = cobfmt(&[f.to_str().unwrap()]);
    assert_eq!(status(&o), 2);
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("does not parse") && err.contains("bad.cb:2"), "{}", err);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), src);
}

#[test]
fn directories_skip_hidden_and_target() {
    let d = scratch("tree");
    for sub in ["src", "src/inner", ".git", "target"] {
        std::fs::create_dir_all(d.join(sub)).unwrap();
    }
    for f in ["src/a.cb", "src/inner/b.cb", ".git/c.cb", "target/d.cb"] {
        std::fs::write(d.join(f), MESSY).unwrap();
    }
    std::fs::write(d.join("src/notes.txt"), "x").unwrap();
    let o = cobfmt(&["--quiet", d.to_str().unwrap()]);
    assert_eq!(status(&o), 0);
    assert_eq!(std::fs::read_to_string(d.join("src/a.cb")).unwrap(), TIDY);
    assert_eq!(std::fs::read_to_string(d.join("src/inner/b.cb")).unwrap(), TIDY);
    assert_eq!(std::fs::read_to_string(d.join(".git/c.cb")).unwrap(), MESSY);
    assert_eq!(std::fs::read_to_string(d.join("target/d.cb")).unwrap(), MESSY);
}

#[test]
fn width_breaks_a_long_call_one_argument_per_line() {
    let src = "import std;\nfn main()\n{\n    printf(\"%v %v %v\\n\", 111111111, 222222222, 333333333);\n}\n";
    let o = cobfmt_stdin(&["--width", "40", "-"], src);
    assert_eq!(status(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let out = String::from_utf8_lossy(&o.stdout).to_string();
    assert_eq!(
        out,
        "import std;\nfn main()\n{\n    printf(\n        \"%v %v %v\\n\",\n        111111111,\n        222222222,\n        333333333,\n    );\n}\n"
    );
    // Formatting it again, with the same width, changes nothing.
    let again = cobfmt_stdin(&["--width", "40", "-"], &out);
    assert_eq!(String::from_utf8_lossy(&again.stdout), out);
    // A short line is not broken at the default width (120).
    assert_eq!(String::from_utf8_lossy(&cobfmt_stdin(&["-"], src).stdout), src);
}

// The owner, 2026-10-04: lines are broken at 120 characters by default;
// `--no-wrap` leaves every line as long as it is.
#[test]
fn the_default_width_is_120_and_no_wrap_breaks_nothing() {
    let args: Vec<String> = (0..12).map(|i| format!("{}", 1_000_000_000u64 + i)).collect();
    let src = format!("import std;\nfn main()\n{{\n    printf(\"%v\\n\", {});\n}}\n", args.join(", "));
    assert!(src.lines().any(|l| l.len() > 120));
    let out = String::from_utf8_lossy(&cobfmt_stdin(&["-"], &src).stdout).to_string();
    assert!(out.lines().all(|l| l.len() <= 120), "{}", out);
    assert!(out.contains("    printf(\n"), "{}", out);
    let kept = String::from_utf8_lossy(&cobfmt_stdin(&["--no-wrap", "-"], &src).stdout).to_string();
    assert_eq!(kept, src);
}

// A type's arguments stay together when a parameter list is broken
// (`slice<u8, shared>` is one parameter's type, not two items).
#[test]
fn breaking_keeps_type_arguments_together() {
    let src = "fn check(str name, slice<u8, shared> salt, slice<u8, shared> ikm, ref<HashMap<String, Vec<u8>>, shared> m, usize len, str w)\n{\n}\n";
    let out = String::from_utf8_lossy(&cobfmt_stdin(&["--width", "60", "-"], src).stdout).to_string();
    assert_eq!(
        out,
        "fn check(\n    str name,\n    slice<u8, shared> salt,\n    slice<u8, shared> ikm,\n    ref<HashMap<String, Vec<u8>>, shared> m,\n    usize len,\n    str w,\n)\n{\n}\n"
    );
}

#[test]
fn crlf_round_trips() {
    let d = scratch("crlf");
    let f = d.join("w.cb");
    std::fs::write(&f, MESSY.replace('\n', "\r\n")).unwrap();
    assert_eq!(status(&cobfmt(&[f.to_str().unwrap()])), 0);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), TIDY.replace('\n', "\r\n"));
}

// The style is the corpus's (§8): these are already formatted.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

#[test]
fn the_showcases_examples_and_benchmarks_are_already_formatted() {
    let r = repo();
    for dir in ["showcase", "impl/cobaltc_examples", "bench"] {
        let o = cobfmt(&["--check", r.join(dir).to_str().unwrap()]);
        assert_eq!(status(&o), 0, "{} has files cobfmt would change:\n{}{}", dir, String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    }
}

// The conformance cases are formatted too; syntax-error cases are not
// formatted (exit 2) by design.
#[test]
fn the_conformance_cases_are_already_formatted() {
    let dir = repo().join("impl/conformance");
    let o = cobfmt(&["--check", dir.to_str().unwrap()]);
    assert!(String::from_utf8_lossy(&o.stdout).trim().is_empty(), "files cobfmt would change:\n{}", String::from_utf8_lossy(&o.stdout));
    for line in String::from_utf8_lossy(&o.stderr).lines() {
        assert!(line.contains("does not parse"), "unexpected error: {}", line);
    }
}
