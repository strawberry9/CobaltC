// The `cobc` driver itself: what it does with its output path.

use std::fs;
use std::process::Command;

#[path = "oracle/exe.rs"]
mod exe;
use exe::cobc_exe;

// `cobc foo.cb` writes `foo`; when `foo` is a directory (one holding the
// program's module files, say), it must say so and write nothing, not
// leave the linker to fail with "cannot open output file".
#[test]
fn default_output_that_is_a_directory_is_reported() {
    let dir = std::env::temp_dir().join(format!("cobc-driver-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("prog")).unwrap();
    fs::write(dir.join("prog.cb"), "import std;\n\nfn main()\n{\n    printf(\"hi\\n\");\n}\n").unwrap();
    let out = Command::new(cobc_exe()).arg("prog.cb").current_dir(&dir).output().expect("run cobc");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", err);
    assert!(err.contains("that is a directory") && err.contains("-o"), "stderr: {}", err);
    assert!(!dir.join("prog.c").exists(), "the C file was written anyway");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn about_shows_the_banner_and_explains_cobc() {
    let out = Command::new(cobc_exe()).arg("--about").output().expect("run cobc");
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
    let cobc = |args: &[&str]| Command::new(cobc_exe()).args(args).current_dir(&dir).output().expect("run cobc");
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

    // `--check-syntax` (CHG-0346): the parser alone, as `coby`'s.
    assert_eq!(cobc(&["--check-syntax", "bad.cb"]).status.code(), Some(0));
    fs::write(dir.join("broken.cb"), "fn main()\n{\n    x = ;\n}\n").unwrap();
    let out = cobc(&["--check-syntax", "broken.cb"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&out.stderr), "broken.cb:3: expected an expression, found `;` (column 9)\n");
    assert_eq!(cobc(&["--check-syntax"]).status.code(), Some(2));
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
    // Decided (D-0284): the program keeps no record, and the report says so.
    let out = Command::new(cobc_exe()).args(["--report-tracking", "prog.cb"]).current_dir(&dir).output().expect("run cobc");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(text.starts_with("no run-time tracking: every conflict is decided"), "{}", text);
    // With the record kept, the listing: what the record would cost.
    let out = Command::new(cobc_exe()).args(["--report-tracking", "prog.cb"]).env("COBALTC_PATHLESS", "0").current_dir(&dir).output().expect("run cobc");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(text.starts_with("The program keeps a record of references: COBALTC_PATHLESS=0"), "{}", text);
    let first = text.lines().nth(3).unwrap_or("");
    assert!(first.contains("prog.cb:12") && first.contains("loop") && first.contains("t += v[i];"), "{}", text);
    assert!(!text.contains("prog.cb:18 "), "the confined loop is listed:\n{}", text);
    assert!(!dir.join("prog").exists() && !dir.join("prog.c").exists(), "something was compiled");
    let _ = fs::remove_dir_all(&dir);
}

// D-0285: a program the first lowering finds decided is lowered again
// knowing it: the C says so (`CB_DECIDED`), a view copied and walked in a
// function needs no object and its statements no scopes, and the program
// ends as its first lowering does (COBALTC_DECIDED=0), under both C
// compilers.
#[test]
fn decided_program_is_lowered_knowing_it() {
    let dir = std::env::temp_dir().join(format!("cobc-decided-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = "import std;\n\nfn count(StringView v) : u64\n{\n    u64 n = 0;\n    foreach (c in StringView::chars(v))\n    {\n        n += 1;\n    }\n    n\n}\n\nfn main()\n{\n    String s = String::from_str(\"h\u{e9}llo\");\n    StringView v = String::as_view(&s);\n    printf(\"%v %v\\n\", count(v), count(StringView::sub(v, 0, 1)));\n}\n";
    fs::write(dir.join("prog.cb"), src).unwrap();
    for cc in ["gcc", "clang"] {
        let build = |decided: &str, out: &str| {
            let o = Command::new(cobc_exe()).args(["-v", "--keep-c", "--cc", cc, "-o", out, "prog.cb"]).env("COBALTC_DECIDED", decided).current_dir(&dir).output().expect("run cobc");
            assert!(o.status.success(), "{}: {}", cc, String::from_utf8_lossy(&o.stderr));
            let run = Command::new(dir.join(out)).output().expect("run prog");
            assert_eq!((run.status.code(), String::from_utf8_lossy(&run.stdout).into_owned()), (Some(0), "5 1\n".to_string()), "{}", cc);
            (String::from_utf8_lossy(&o.stderr).into_owned(), fs::read_to_string(dir.join(format!("{}.c", out))).unwrap())
        };
        let (note, c) = build("1", "d");
        assert!(note.contains("lowered pathless and decided (D-0284, D-0285)"), "{}: {}", cc, note);
        assert!(c.starts_with("#define CB_PATHLESS 1\n#define CB_DECIDED 1\n"), "{}: {}", cc, &c[..c.len().min(200)]);
        let at = c.find("static uint64_t f_count(struct s_std__StringView p_v)\n{").expect("f_count");
        let body = &c[at..at + c[at..].find("\n}\n").unwrap()];
        assert!(!body.contains("cb_new(") && !body.contains("cb_stmt_push") && !body.contains("cb_copy_datum("), "{}: {}", cc, body);
        let (note, c) = build("0", "p");
        assert!(note.contains("lowered pathless (D-0284)"), "{}: {}", cc, note);
        assert!(c.starts_with("#define CB_PATHLESS 1\n") && !c.contains("#define CB_DECIDED"), "{}", cc);
    }
    let _ = fs::remove_dir_all(&dir);
}

// CHG-0382: a call whose later argument is a borrow formed in place
// (`std::Vec::swap`'s `swap_places(&mut (*v)[i], &mut (*v)[j])`), or
// whose earlier argument is a value (`f(x, f(x, 1))`), leaves nothing to
// the run: the program is decided, and ends as the interpreter has it.
#[test]
fn vec_swap_and_value_arguments_leave_a_program_decided() {
    let dir = std::env::temp_dir().join(format!("cobc-evalorder-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = "import std;\n\nfn f(i64 a, i64 b) : i64\n{\n    a + b\n}\n\nfn main()\n{\n    Vec<i32> v = Vec::new();\n    Vec::push(&mut v, 1);\n    Vec::push(&mut v, 2);\n    Vec::swap(&mut v, 0, 1);\n    i64 x = 2;\n    printf(\"%v %v %v\\n\", v[0], v[1], f(x, f(x, 1)));\n}\n";
    fs::write(dir.join("prog.cb"), src).unwrap();
    for cc in ["gcc", "clang"] {
        let o = Command::new(cobc_exe()).args(["-v", "--cc", cc, "-o", "prog", "prog.cb"]).current_dir(&dir).output().expect("run cobc");
        assert!(o.status.success(), "{}: {}", cc, String::from_utf8_lossy(&o.stderr));
        let note = String::from_utf8_lossy(&o.stderr).into_owned();
        assert!(note.contains("lowered pathless and decided (D-0284, D-0285)"), "{}: {}", cc, note);
        let run = Command::new(dir.join("prog")).output().expect("run prog");
        assert_eq!((run.status.code(), String::from_utf8_lossy(&run.stdout).into_owned()), (Some(0), "2 1 5\n".to_string()), "{}", cc);
    }
    let _ = fs::remove_dir_all(&dir);
}

// D-0289: a program whose threads are lent what they reach by handles
// its frames hold is decided: it locks a mutex with no lock path, a thread
// lent an exclusive reference is not contained (its fault ends the
// program, spec/18), a resource passes to and from a thread with no
// object, and it ends as the interpreter has it, under both C compilers.
#[test]
fn threads_lent_by_held_handles_leave_a_program_decided() {
    let dir = std::env::temp_dir().join(format!("cobc-scoped-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let ok = "import std;\n\nfn add(ref<mutex<u64>, shared> m, u64 n) : u64\n{\n    foreach (i in 0..n)\n    {\n        auto g = lock(m);\n        *g += 1;\n    }\n    n\n}\n\nfn bump(ref<i32, exclusive> p)\n{\n    *p = *p + 1;\n}\n\nfn total(Vec<u64> v) : Vec<u64>\n{\n    Vec<u64> r = Vec::new();\n    Vec::push(&mut r, widen<u64>(Vec::len(&v)));\n    r\n}\n\nfn main()\n{\n    auto m = Mutex::new(0: u64);\n    i32 x = 1;\n    {\n        auto a = spawn(add, &m, 30);\n        auto b = spawn(add, &m, 12);\n        auto c = spawn(bump, &mut x);\n        join(c);\n        printf(\"%v \", join(a) + join(b));\n    }\n    Vec<u64> v = Vec::new();\n    Vec::push(&mut v, 7);\n    auto t = spawn(total, v);\n    Vec<u64> r = join(t);\n    printf(\"%v %v %v\\n\", *lock(&m), x, r[0]);\n}\n";
    let lent = "import std;\n\nfn boom(ref<i32, exclusive> p) : i32\n{\n    *p = 2;\n    1 / (*p - 2)\n}\n\nfn main()\n{\n    i32 x = 1;\n    auto h = spawn(boom, &mut x);\n    match (try_join(h))\n    {\n        Ok(v) : printf(\"ok %v\\n\", v),\n        Err(f) : printf(\"contained\\n\"),\n    }\n    printf(\"after\\n\");\n}\n";
    fs::write(dir.join("ok.cb"), ok).unwrap();
    fs::write(dir.join("lent.cb"), lent).unwrap();
    // As `coby` ends them: the lent thread's fault is not contained, so
    // the program ends with it and prints nothing.
    let ok_want = (Some(0), "42 42 2 1\n".to_string());
    let lent_want = (Some(1), String::new());
    for cc in ["gcc", "clang"] {
        for (f, want) in [("ok", &ok_want), ("lent", &lent_want)] {
            let o = Command::new(cobc_exe()).args(["-v", "--cc", cc, "-o", f, &format!("{}.cb", f)]).current_dir(&dir).output().expect("run cobc");
            assert!(o.status.success(), "{} {}: {}", cc, f, String::from_utf8_lossy(&o.stderr));
            let note = String::from_utf8_lossy(&o.stderr).into_owned();
            assert!(note.contains("lowered pathless and decided (D-0284, D-0285)"), "{} {}: {}", cc, f, note);
            let run = Command::new(dir.join(f)).current_dir(&dir).output().expect("run prog");
            assert_eq!(&(run.status.code(), String::from_utf8_lossy(&run.stdout).into_owned()), want, "{} {}", cc, f);
        }
    }
    let _ = fs::remove_dir_all(&dir);
}

// D-0289, CHG-0384: a worker pool's handles kept in a local `Vec` hold
// what their threads were lent, as a handle binding does: pushed by
// `Vec::push(&mut hs, spawn(…))`, taken out by a consuming `foreach`, the
// program is decided. A handle left lending on some paths when the lent
// place is written (`popped`: one taken out, the rest still running;
// `branch`: dropped on one branch only), or the `Vec` given away
// (`given`), leave the function to the run.
#[test]
fn worker_pools_in_a_vec_leave_a_program_decided() {
    let dir = std::env::temp_dir().join(format!("cobc-pools-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let pool = "import std;\n\nfn look(ref<i64, shared> x) : i64\n{\n    *x\n}\n\nfn main()\n{\n    i64 x = 3;\n    Vec<handle<i64>> hs = Vec::new();\n    for (i64 i = 0; i < 4; i += 1)\n    {\n        Vec::push(&mut hs, spawn(look, &x));\n    }\n    i64 t = 0;\n    foreach (h in hs)\n    {\n        t += join(h);\n    }\n    x = 10;\n    printf(\"%v %v\\n\", t, x);\n}\n";
    let popped = "import std;\n\nfn look(ref<i64, shared> x) : i64\n{\n    *x\n}\n\nfn main()\n{\n    i64 x = 3;\n    Vec<handle<i64>> hs = Vec::new();\n    for (i64 i = 0; i < 4; i += 1)\n    {\n        Vec::push(&mut hs, spawn(look, &x));\n    }\n    i64 t = 0;\n    if (Some(w) = Vec::pop(&mut hs))\n    {\n        t += join(w);\n    }\n    x = 10;\n    printf(\"%v %v\\n\", t, x);\n}\n";
    let branch = "import std;\n\nfn look(ref<i64, shared> x) : i64\n{\n    *x\n}\n\nfn main()\n{\n    i64 x = 3;\n    handle<i64> h = spawn(look, &x);\n    if (x == 3)\n    {\n        drop(h);\n    }\n    x = 10;\n    printf(\"%v\\n\", x);\n}\n";
    let given = "import std;\n\nfn look(ref<i64, shared> x) : i64\n{\n    *x\n}\n\nfn give(ref<Vec<handle<i64>>, exclusive> hs, ref<Vec<handle<i64>>, exclusive> out)\n{\n    Vec::push(out, Option::unwrap(Vec::pop(hs)));\n}\n\nfn main()\n{\n    Vec<handle<i64>> keep = Vec::new();\n    {\n        i64 x = 3;\n        Vec<handle<i64>> hs = Vec::new();\n        Vec::push(&mut hs, spawn(look, &x));\n        give(&mut hs, &mut keep);\n    }\n    printf(\"%v\\n\", join(Option::unwrap(Vec::pop(&mut keep))));\n}\n";
    for (f, src) in [("pool", pool), ("popped", popped), ("branch", branch), ("given", given)] {
        fs::write(dir.join(format!("{}.cb", f)), src).unwrap();
    }
    for cc in ["gcc", "clang"] {
        let o = Command::new(cobc_exe()).args(["-v", "--cc", cc, "-o", "pool", "pool.cb"]).current_dir(&dir).output().expect("run cobc");
        let note = String::from_utf8_lossy(&o.stderr).into_owned();
        assert!(o.status.success() && note.contains("lowered pathless and decided (D-0284, D-0285)"), "{}: {}", cc, note);
        let run = Command::new(dir.join("pool")).current_dir(&dir).output().expect("run prog");
        assert_eq!((run.status.code(), String::from_utf8_lossy(&run.stdout).into_owned()), (Some(0), "12 10\n".to_string()), "{}", cc);
    }
    for (f, why) in [("popped", "may reach on some paths"), ("branch", "may reach on some paths"), ("given", "is borrowed exclusively while it holds what its threads were lent")] {
        let o = Command::new(cobc_exe()).args(["-v", "--cc", "gcc", "-o", f, &format!("{}.cb", f)]).current_dir(&dir).output().expect("run cobc");
        let note = String::from_utf8_lossy(&o.stderr).into_owned();
        assert!(o.status.success() && note.contains("kept paths") && note.contains(why), "{}: {}", f, note);
    }
    let _ = fs::remove_dir_all(&dir);
}

// D-0286: in a decided program values pass between functions with no
// object: past the runtime's header the C neither sends nor receives one,
// a `String` taken by value is bound objectless and none is bound in the
// function, and the program ends as its first lowering does
// (COBALTC_DECIDED=0), under both C compilers.
#[test]
fn decided_program_passes_values_without_objects() {
    let dir = std::env::temp_dir().join(format!("cobc-flights-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = "import std;\n\nfn twice(String s) : String\n{\n    String r = String::clone(&s);\n    String::append(&mut r, String::as_view(&s));\n    r\n}\n\nfn first(Vec<String> v) : Option<String>\n{\n    if (Vec::len(&v) == 0)\n    {\n        return None;\n    }\n    Some(twice(String::clone(&v[0])))\n}\n\nfn main()\n{\n    Vec<String> v = Vec::new();\n    Vec::push(&mut v, String::from_str(\"ab\"));\n    match (first(v))\n    {\n        Some(x) : printf(\"%s %v\\n\", &x, String::len(&x)),\n        None : printf(\"none\\n\"),\n    }\n    match (first(Vec::new()))\n    {\n        Some(x) : printf(\"%s\\n\", &x),\n        None : printf(\"none\\n\"),\n    }\n}\n";
    fs::write(dir.join("prog.cb"), src).unwrap();
    for cc in ["gcc", "clang"] {
        let build = |decided: &str, out: &str| {
            let o = Command::new(cobc_exe()).args(["-v", "--keep-c", "--cc", cc, "-o", out, "prog.cb"]).env("COBALTC_DECIDED", decided).current_dir(&dir).output().expect("run cobc");
            assert!(o.status.success(), "{}: {}", cc, String::from_utf8_lossy(&o.stderr));
            let run = Command::new(dir.join(out)).output().expect("run prog");
            assert_eq!((run.status.code(), String::from_utf8_lossy(&run.stdout).into_owned()), (Some(0), "abab 4\nnone\n".to_string()), "{}", cc);
            (String::from_utf8_lossy(&o.stderr).into_owned(), fs::read_to_string(dir.join(format!("{}.c", out))).unwrap())
        };
        let (note, c) = build("1", "d");
        assert!(note.contains("lowered pathless and decided"), "{}: {}", cc, note);
        let after = &c[c.find("/* ---- types ---- */").expect("types")..];
        assert!(!after.contains("cb_send(") && !after.contains("cb_recv("), "{}: a value in flight", cc);
        let at = c.find("static struct s_std__String f_twice(struct s_std__String p_s)\n{").expect("f_twice");
        let body = &c[at..at + c[at..].find("\n}\n").unwrap()];
        assert!(!body.contains("cb_bind(") && body.contains("cb_drop_plain_buf(&p_s,"), "{}: {}", cc, body);
        let (_, c) = build("0", "p");
        assert!(c.contains("cb_recv("), "{}", cc);
    }
    let _ = fs::remove_dir_all(&dir);
}

// Linkage: the program's own functions are `static` (the C compiler may
// inline them), `main` is not, and a program whose loop calls pure-`std`
// functions (`Vec::push`, `Vec::len`, the element accessor) builds and
// runs.
#[test]
fn program_functions_are_static_and_std_helpers_link() {
    let dir = std::env::temp_dir().join(format!("cobc-linkage-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = "import std;\n\nfn step(ref<i64, exclusive> x)\n{\n    *x += 1;\n}\n\nfn main()\n{\n    Vec<i64> v = Vec::new();\n    foreach (i in 0..10)\n    {\n        Vec::push(&mut v, narrow<i64>(i));\n    }\n    i64 t = 0;\n    foreach (i in 0..Vec::len(&v))\n    {\n        step(&mut t);\n        t += v[i];\n    }\n    printf(\"%v\\n\", t);\n}\n";
    fs::write(dir.join("prog.cb"), src).unwrap();
    let out = Command::new(cobc_exe()).args(["--keep-c", "-o", "prog", "prog.cb"]).current_dir(&dir).output().expect("run cobc");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let c = fs::read_to_string(dir.join("prog.c")).unwrap();
    assert!(c.lines().any(|l| l.starts_with("static void f_step(")), "`step` is not static");
    assert!(c.lines().any(|l| l.starts_with("void f_main(")), "`main` is not external");
    let run = Command::new(dir.join("prog")).output().expect("run prog");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "55\n");
    let _ = fs::remove_dir_all(&dir);
}

// WI-11: `#line` directives name the program's lines in the generated C,
// and `--no-line-directives` leaves them out; the C is otherwise the same.
// Both C compilers build what they number.
#[test]
fn line_directives_name_the_program_lines() {
    let dir = std::env::temp_dir().join(format!("cobc-lines-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = "import std;\n\nfn add(i32 a, i32 b) : i32\n{\n    i32 c = a + b;\n    return c;\n}\n\nfn main() : u8\n{\n    i32 x = add(1, 2);\n    Vec<i32> v = Vec::new();\n    Vec::push(&mut v, x);\n    printf(\"%v\\n\", v[0]);\n    return 0;\n}\n";
    fs::write(dir.join("prog.cb"), src).unwrap();
    for cc in ["gcc", "clang"] {
        let build = |extra: &[&str], out: &str| {
            let mut args = vec!["--keep-c", "--cc", cc, "-o", out];
            args.extend_from_slice(extra);
            args.push("prog.cb");
            let o = Command::new(cobc_exe()).args(&args).current_dir(&dir).output().expect("run cobc");
            assert!(o.status.success(), "{}: {}", cc, String::from_utf8_lossy(&o.stderr));
            let run = Command::new(dir.join(out)).output().expect("run prog");
            assert_eq!(String::from_utf8_lossy(&run.stdout), "3\n", "{}", cc);
            fs::read_to_string(dir.join(format!("{}.c", out))).unwrap()
        };
        let with = build(&[], "a");
        let without = build(&["--no-line-directives"], "b");
        // Every statement of the program's own functions is named.
        for line in [5, 6, 11, 12, 13, 14, 15] {
            assert!(with.contains(&format!("#line {} \"prog.cb\"\n", line)), "{}: no directive for line {}", cc, line);
        }
        // After a function, the C's own numbering resumes.
        let resume = with.lines().enumerate().find(|(_, l)| l.starts_with("#line ") && l.ends_with("a.c\"")).expect("a directive back to the C file");
        let n: usize = resume.1.split(' ').nth(1).unwrap().parse().unwrap();
        assert_eq!(n, resume.0 + 2, "{}: resumes at the wrong line", cc);
        assert!(!without.contains("#line"), "{}: --no-line-directives left a directive", cc);
        let stripped: String = with.lines().filter(|l| !l.starts_with("#line ")).map(|l| format!("{}\n", l)).collect();
        assert_eq!(stripped, without, "{}: the directives changed the C", cc);
    }
    let _ = fs::remove_dir_all(&dir);
}

// WI-11: with `-O0 -g` and a sanitizer, a memory error in the program's
// own (unsafe) code is reported at its `.cb` line. Skipped where clang or
// its AddressSanitizer runtime is missing.
#[test]
fn sanitizer_reports_the_program_line() {
    let ok = Command::new("clang").args(["-fsanitize=address", "-x", "c", "-", "-o", "/dev/null"]).stdin(std::process::Stdio::piped()).spawn().and_then(|mut c| {
        use std::io::Write;
        c.stdin.take().unwrap().write_all(b"int main(void) { return 0; }\n")?;
        c.wait()
    });
    if !matches!(ok, Ok(s) if s.success()) || !cfg!(target_os = "linux") {
        eprintln!("skipped: no clang with AddressSanitizer");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cobc-asan-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let wrapper = concat!(env!("CARGO_MANIFEST_DIR"), "/../fuzz/clang-asan");
    let src = "import std;\n\nstruct P\n{\n    i64 x;\n    i64 y;\n}\n\nfn main()\n{\n    match (allocate(sizeof<i64>(), alignof<i64>()))\n    {\n        Ok(b)  :\n        {\n            rawptr<P> p = reinterpret_ptr<P>(b);\n            unsafe\n            {\n                (*p).y = 40;\n            }\n        },\n        Err(_) :\n        {\n        },\n    }\n}\n";
    fs::write(dir.join("over.cb"), src).unwrap();
    let o = Command::new(cobc_exe()).args(["-O0", "-g", "--cc", wrapper, "-o", "over", "over.cb"]).current_dir(&dir).output().expect("run cobc");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    // `setarch -R`: clang 14's sanitized binaries can fail at start under ASLR.
    let run = Command::new("setarch").args(["-R", "./over"]).current_dir(&dir).output().expect("run over");
    let err = String::from_utf8_lossy(&run.stderr);
    assert!(err.contains("heap-buffer-overflow"), "{}", err);
    assert!(err.contains("over.cb:18"), "the report does not name the program's line:\n{}", err);
    let _ = fs::remove_dir_all(&dir);
}

// D-0230: `cobc --unittest` builds the program's tests; the test program
// reports them exactly as `coby --unittest` does, under both C compilers,
// with -o kept and with --run.
#[test]
fn unittest_build_reports_as_coby_does() {
    let dir = std::env::temp_dir().join(format!("cobc-unittest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = "import std;\n\nfn twice(i32 x) : i32\n{\n    x * 2\n}\n\nunittest \"twice: small\"\n{\n    assert(twice(2) == 4);\n}\n\nunittest\n{\n    assert(twice(3) == 7, \"3 twice is %v\", twice(3));\n}\n\nunittest \"twice: zero\"\n{\n    assert(twice(0) == 0);\n}\n\nfn main()\n{\n    printf(\"main\\n\");\n}\n";
    fs::write(dir.join("t.cb"), src).unwrap();
    let coby = std::path::Path::new(env!("CARGO_BIN_EXE_cobc")).with_file_name(if cfg!(windows) { "coby.exe" } else { "coby" });
    let want = Command::new(&coby).args(["--unittest", "t.cb"]).current_dir(&dir).env("NO_COLOR", "1").output().expect("run coby");
    assert_eq!(want.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&want.stdout).ends_with("3 tests: 2 passed, 1 failed\n"), "{}", String::from_utf8_lossy(&want.stdout));
    for cc in ["gcc", "clang"] {
        let built = Command::new(cobc_exe()).args(["--unittest", "--cc", cc, "-o", "tests", "t.cb"]).current_dir(&dir).output().expect("run cobc");
        assert!(built.status.success(), "{}: {}", cc, String::from_utf8_lossy(&built.stderr));
        let got = Command::new(dir.join("tests")).current_dir(&dir).env("NO_COLOR", "1").output().expect("run the tests");
        assert_eq!((got.status.code(), String::from_utf8_lossy(&got.stdout)), (want.status.code(), String::from_utf8_lossy(&want.stdout)), "{}", cc);
        let run = Command::new(cobc_exe()).args(["--unittest", "--unittest-filter", "zero", "--cc", cc, "--run", "t.cb"]).current_dir(&dir).output().expect("run cobc");
        assert_eq!((run.status.code(), String::from_utf8_lossy(&run.stdout).into_owned()), (Some(0), "test t.cb:18 twice: zero ... ok\n1 test: 1 passed, 0 failed, 2 filtered out\n".to_string()), "{}", cc);
        // Without --unittest, the program: `main`, and no test.
        let plain = Command::new(cobc_exe()).args(["--cc", cc, "--run", "t.cb"]).current_dir(&dir).output().expect("run cobc");
        assert_eq!(String::from_utf8_lossy(&plain.stdout), "main\n", "{}", cc);
    }
    let _ = fs::remove_dir_all(&dir);
}

// WI-15: a panic in either tool is an internal error: one line naming the
// tool and the place in its source, exit status 4, and no Rust panic text
// (its backtrace only with RUST_BACKTRACE). COBALTC_TEST_INTERNAL_ERROR
// makes each tool panic on purpose in the thread that does its work.
#[test]
fn internal_error_is_one_line() {
    let dir = std::env::temp_dir().join(format!("cobc-internal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("p.cb"), "fn main()\n{\n}\n").unwrap();
    let coby = std::path::Path::new(env!("CARGO_BIN_EXE_cobc")).with_file_name(if cfg!(windows) { "coby.exe" } else { "coby" });
    let under_test = cobc_exe();
    for (tool, exe) in [("coby", coby.as_path()), ("cobc", under_test.as_path())] {
        let o = Command::new(exe).arg("p.cb").env("COBALTC_TEST_INTERNAL_ERROR", "1").env_remove("RUST_BACKTRACE").current_dir(&dir).output().expect("run the tool");
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(4), "{}: {}", tool, err);
        assert_eq!(err.lines().count(), 1, "{}: {}", tool, err);
        assert!(err.starts_with(&format!("internal error in {}: COBALTC_TEST_INTERNAL_ERROR is set (", tool)), "{}: {}", tool, err);
        assert!(err.trim_end().ends_with("); please report it with the program"), "{}: {}", tool, err);
        assert!(!err.contains("panicked"), "{}: {}", tool, err);
        let b = Command::new(exe).arg("p.cb").env("COBALTC_TEST_INTERNAL_ERROR", "1").env("RUST_BACKTRACE", "1").current_dir(&dir).output().expect("run the tool");
        assert_eq!(b.status.code(), Some(4), "{}", tool);
        assert!(String::from_utf8_lossy(&b.stderr).lines().count() > 1, "{}: a backtrace follows with RUST_BACKTRACE", tool);
    }
    let _ = fs::remove_dir_all(&dir);
}

// A reader that stops early (`cobc --explore 3 p.cb | head -1`) is not an
// internal error: the tool ends quietly, status 1, as a Unix tool does at
// a closed pipe. Here the pipe is closed before `cobc` has compiled.
#[test]
fn closed_output_pipe_is_not_an_internal_error() {
    let dir = std::env::temp_dir().join(format!("cobc-pipe-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("p.cb"), "import std;\n\nfn main()\n{\n    printf(\"x\\n\");\n}\n").unwrap();
    let mut child = Command::new(cobc_exe())
        .args(["--explore", "3", "p.cb"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(&dir)
        .spawn()
        .expect("run cobc");
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("wait for cobc");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", err);
    assert!(!err.contains("internal error"), "stderr: {}", err);
    let _ = fs::remove_dir_all(&dir);
}

// `--explore` gives every run the same standard input, but no longer reads
// it to its end first: with an input held open (a supervisor's pipe) and a
// program that never reads it, the runs go ahead (they waited for ever).
#[test]
fn explore_does_not_wait_for_an_open_input() {
    let dir = std::env::temp_dir().join(format!("cobc-explore-input-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("p.cb"), "import std;\n\nfn main()\n{\n    printf(\"x\\n\");\n}\n").unwrap();
    let mut child = Command::new(cobc_exe())
        .args(["--explore", "3", "p.cb"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(&dir)
        .spawn()
        .expect("run cobc");
    let held = child.stdin.take();
    let start = std::time::Instant::now();
    let mut status = None;
    while start.elapsed() < std::time::Duration::from_secs(300) {
        if let Some(s) = child.try_wait().expect("poll cobc") {
            status = Some(s);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if status.is_none() {
        let _ = child.kill();
    }
    drop(held);
    let out = child.wait_with_output().expect("wait for cobc");
    assert!(status.is_some(), "cobc --explore waited for its input to end");
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("explored 3 schedules of p.cb: 1 distinct outcome"));
    let _ = fs::remove_dir_all(&dir);
}

// D-0255: `--target` names one of the two targets; another is a usage error
// naming both.
#[test]
fn unknown_target_is_a_usage_error() {
    let dir = std::env::temp_dir().join(format!("cobc-target-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("p.cb"), "import std;\n\nfn main()\n{\n}\n").unwrap();
    let out = Command::new(cobc_exe()).args(["--target", "aarch64-unknown-linux-gnu", "p.cb"]).current_dir(&dir).output().expect("run cobc");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", err);
    assert!(err.contains("x86_64-unknown-linux-gnu") && err.contains("x86_64-pc-windows-gnu"), "stderr: {}", err);
    let _ = fs::remove_dir_all(&dir);
}

// D-0255: from Linux, `--target x86_64-pc-windows-gnu` writes `p.exe` with
// MinGW-w64's GCC, and `--run` runs it under COBALTC_RUNNER. Skipped, saying
// so, where MinGW-w64, Wine or the Windows runtime is missing.
#[cfg(target_os = "linux")]
#[test]
fn windows_target_builds_an_exe_that_wine_runs() {
    let have = |p: &str| Command::new(p).arg("--version").output().map_or(false, |o| o.status.success());
    let cobc = cobc_exe();
    let runtime = std::path::Path::new(env!("CARGO_BIN_EXE_cobc")).parent().and_then(|d| d.parent()).map(|t| t.join("x86_64-pc-windows-gnu/release/libcbrt.a"));
    if !have("x86_64-w64-mingw32-gcc") || !have("wine64") || !runtime.as_ref().map_or(false, |r| r.exists()) {
        eprintln!("skipped: needs x86_64-w64-mingw32-gcc, wine64 and target/x86_64-pc-windows-gnu/release/libcbrt.a (cargo +1.77.2 build --release -p cbrt --target x86_64-pc-windows-gnu)");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cobc-win-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("p.cb"), "import std;\n\nfn main() : u8\n{\n    printf(\"%v %v\\n\", arg_count(), 6 * 7);\n    3\n}\n").unwrap();
    let out = Command::new(&cobc).args(["--target", "x86_64-pc-windows-gnu", "p.cb"]).current_dir(&dir).output().expect("run cobc");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.join("p.exe").exists(), "no p.exe");
    // --run without a runner says what it needs.
    let out = Command::new(&cobc).args(["--target", "x86_64-pc-windows-gnu", "--run", "p.cb"]).env_remove("COBALTC_RUNNER").current_dir(&dir).output().expect("run cobc");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("COBALTC_RUNNER"));
    let out = Command::new(&cobc)
        .args(["--target", "x86_64-pc-windows-gnu", "--run", "p.cb", "a", "b"])
        .env("COBALTC_RUNNER", "wine64")
        .env("WINEDEBUG", "-all")
        .current_dir(&dir)
        .output()
        .expect("run cobc");
    assert_eq!(out.status.code(), Some(3), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "2 42\n");
    let _ = fs::remove_dir_all(&dir);
}

// `--warn-shadow` (CHG-0333): the note, with `--run` and with `--check`,
// the flag before or after `--check`; the exit status unchanged.
#[test]
fn warn_shadow_notes_hidden_imports() {
    let dir = std::env::temp_dir().join(format!("cobc-warn-shadow-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let cobc = |args: &[&str]| Command::new(cobc_exe()).args(args).current_dir(&dir).output().expect("run cobc");
    let src = "import std;\n\nstruct Option\n{\n    bool some;\n}\n\nfn main()\n{\n    Option o = Option { .some = true };\n    printf(\"%v\\n\", o.some);\n}\n";
    fs::write(dir.join("shadow.cb"), src).unwrap();
    let out = cobc(&["--warn-shadow", "--run", "shadow.cb"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "true\n");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("note: struct `Option` of this program hides `std::Option`") && err.contains("--> shadow.cb:3"), "{}", err);
    for args in [["--check", "--warn-shadow", "shadow.cb"], ["--warn-shadow", "--check", "shadow.cb"]] {
        let out = cobc(&args);
        assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
        assert!(String::from_utf8_lossy(&out.stderr).contains("hides `std::Option`"));
    }
    let out = cobc(&["--check", "shadow.cb"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
    let _ = fs::remove_dir_all(&dir);
}

// D-0269: a contained fault's unwind ends the objectless locals (D-0200) of
// the frames it folds, whose ends are C statements the unwind's jump leaves
// unrun. Observable only as memory: LeakSanitizer reports nothing for a
// pool whose jobs fault, and the program's output is the one `coby` gives.
// Skipped where clang or its AddressSanitizer runtime is missing.
#[test]
fn contained_fault_frees_objectless_locals() {
    let ok = Command::new("clang").args(["-fsanitize=address", "-x", "c", "-", "-o", "/dev/null"]).stdin(std::process::Stdio::piped()).spawn().and_then(|mut c| {
        use std::io::Write;
        c.stdin.take().unwrap().write_all(b"int main(void) { return 0; }\n")?;
        c.wait()
    });
    if !matches!(ok, Ok(s) if s.success()) || !cfg!(target_os = "linux") {
        eprintln!("skipped: no clang with AddressSanitizer");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cobc-noted-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let wrapper = concat!(env!("CARGO_MANIFEST_DIR"), "/../fuzz/clang-asan");
    let case = concat!(env!("CARGO_MANIFEST_DIR"), "/../conformance/19-concurrency/contained_fault_frees_objectless_locals_ok.cb");
    fs::copy(case, dir.join("pool.cb")).unwrap();
    let coby = std::path::Path::new(env!("CARGO_BIN_EXE_cobc")).with_file_name(if cfg!(windows) { "coby.exe" } else { "coby" });
    let want = Command::new(&coby).arg("pool.cb").current_dir(&dir).output().expect("run coby");
    assert!(want.status.success(), "{}", String::from_utf8_lossy(&want.stderr));
    let o = Command::new(cobc_exe()).args(["-O0", "-g", "--cc", wrapper, "-o", "pool", "pool.cb"]).current_dir(&dir).output().expect("run cobc");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    // `setarch -R`: clang 14's sanitized binaries can fail at start under ASLR.
    let run = Command::new("setarch").args(["-R", "./pool"]).current_dir(&dir).output().expect("run pool");
    let err = String::from_utf8_lossy(&run.stderr);
    assert!(!err.contains("LeakSanitizer") && !err.contains("AddressSanitizer"), "the sanitizer reported:\n{}", err);
    assert_eq!(run.status.code(), Some(0), "{}", err);
    assert_eq!(String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&want.stdout), "the output differs from coby's");
    let _ = fs::remove_dir_all(&dir);
}

// `--emit-c -o OUT` writes to OUT the C that standard output gets, its
// `#line` directives naming OUT unless `--c-name` names another; `-o -` is
// standard output; the options come in either order; a program the static
// pass rejects gets its diagnostic and status 1, and OUT is not written.
#[test]
fn emit_c_writes_to_the_file_o_names() {
    let dir = std::env::temp_dir().join(format!("cobc-emit-c-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("prog.cb"), "import std;\n\nfn main()\n{\n    printf(\"hi\\n\");\n}\n").unwrap();
    let emit = |args: &[&str]| Command::new(cobc_exe()).arg("--emit-c").args(args).current_dir(&dir).output().expect("run cobc");
    let named = emit(&["--c-name", "out.c", "prog.cb"]);
    assert!(named.status.success(), "{}", String::from_utf8_lossy(&named.stderr));
    assert!(String::from_utf8_lossy(&named.stdout).contains(" \"out.c\"\n"), "no #line naming out.c");
    let o = emit(&["-o", "out.c", "prog.cb"]);
    assert!(o.status.success() && o.stdout.is_empty(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(fs::read(dir.join("out.c")).unwrap(), named.stdout);
    let both = emit(&["-o", "other.c", "--c-name", "out.c", "prog.cb"]);
    assert!(both.status.success());
    assert_eq!(fs::read(dir.join("other.c")).unwrap(), named.stdout);
    let plain = emit(&["prog.cb"]);
    let dash = emit(&["-o", "-", "prog.cb"]);
    assert!(dash.status.success() && !dash.stdout.is_empty());
    assert_eq!(dash.stdout, plain.stdout);
    fs::write(dir.join("bad.cb"), "fn main()\n{\n    u8 x = true;\n}\n").unwrap();
    let bad = emit(&["-o", "bad.c", "bad.cb"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("diag.type-mismatch"), "{}", String::from_utf8_lossy(&bad.stderr));
    assert!(!dir.join("bad.c").exists());
    let usage = emit(&["-o", "x.c"]);
    assert_eq!(usage.status.code(), Some(2));
    let _ = fs::remove_dir_all(&dir);
}
