// D-0309 (1b): the stack check (`cb_stack_check`, D-0077) is kept in a
// function on a cycle of calls, one whose address is taken, and one whose
// frame alone is over the budget (`stack_checks.rs`), and dropped
// elsewhere -- `main` included, the first frame of its thread. A recursion
// direct, mutual or through a `fn` value still faults
// `diag.stack-exhausted` under gcc and clang, as in `coby`.

#[path = "oracle/exe.rs"]
mod exe;

use std::process::Command;

const PROG: &str = r#"import std;
import std::env;

fn direct(u64 n) : u64
{
    if (n == 0) { 0 } else { 1 + direct(n - 1) }
}

fn ping(u64 n) : u64
{
    if (n == 0) { 0 } else { 1 + pong(n - 1) }
}

fn pong(u64 n) : u64
{
    if (n == 0) { 0 } else { 1 + ping(n - 1) }
}

fn via_value(fn(u64) : u64 f, u64 n) : u64
{
    if (n == 0) { 0 } else { 1 + f(n - 1) }
}

fn through(u64 n) : u64
{
    via_value(through, n)
}

fn big_helper(u64 n) : u64
{
    array<u64, 12500> a = [0; 12500];
    a[widen<usize>(n % 12500)] = n;
    a[0] + a[widen<usize>(n % 12500)]
}

fn with_big(u64 n) : u64
{
    if (n == 0) { 0 } else { big_helper(n) + with_big(n - 1) }
}

fn main() : u8
{
    String m = Result::unwrap_or(arg(0), String::new());
    u64 n = 100000000;
    if (m == "direct") { printf("%v\n", direct(n)); }
    if (m == "mutual") { printf("%v\n", ping(n)); }
    if (m == "value") { printf("%v\n", through(n)); }
    if (m == "big") { printf("%v\n", with_big(n)); }
    if (m == "ok") { printf("%v %v\n", direct(1000), with_big(100)); }
    0
}
"#;

// The functions whose body keeps `cb_stack_check();`, by C name.
fn checked(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut sig = "";
    for l in text.lines() {
        if !l.starts_with([' ', '#', '{', '}']) && l.ends_with(')') {
            sig = l;
        }
        if l == "    cb_stack_check();" {
            let open = sig.find('(').unwrap_or(sig.len());
            let head = &sig[..open];
            let name = head.rsplit(' ').next().unwrap_or("");
            out.push(name.to_string());
        }
    }
    out.sort();
    out
}

#[test]
fn stack_checks_kept_only_where_depth_can_grow() {
    let dir = std::env::temp_dir().join(format!("cobc-stack-checks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let case = dir.join("r.cb");
    std::fs::write(&case, PROG).unwrap();
    let c = dir.join("r.c");
    let out = Command::new(exe::cobc_exe()).arg("--emit-c").arg("-o").arg(&c).arg(&case).output().expect("run cobc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&c).unwrap();
    let got = checked(&text);
    for f in ["f_direct", "f_ping", "f_pong", "f_through", "f_with_big", "f_big_helper"] {
        assert!(got.iter().any(|g| g == f), "{} lost its check; kept: {:?}", f, got);
    }
    for f in ["f_main", "f_via_value"] {
        assert!(!got.iter().any(|g| g == f), "{} kept a check; kept: {:?}", f, got);
    }
    for cc in ["gcc", "clang"] {
        let exe = dir.join(format!("r_{}", cc));
        let out = Command::new(exe::cobc_exe()).args(["--cc", cc, "-o"]).arg(&exe).arg(&case).output().expect("run cobc");
        assert!(out.status.success(), "{}: {}", cc, String::from_utf8_lossy(&out.stderr));
        let run = Command::new(&exe).arg("ok").output().expect("run the program");
        assert_eq!(String::from_utf8_lossy(&run.stdout), "1000 5050\n", "{}", cc);
        for m in ["direct", "mutual", "value"] {
            let run = Command::new(&exe).arg(m).output().expect("run the program");
            let err = String::from_utf8_lossy(&run.stderr);
            assert!(err.starts_with("fault[diag.stack-exhausted]"), "{} {}: expected the fault; stderr {:?}", cc, m, err);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
