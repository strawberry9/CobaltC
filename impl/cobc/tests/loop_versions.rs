// D-0309: a loop whose test keeps its variable below a bound fixed for the
// loop (`loop_bound`), and whose body indexes a vector of fixed length with
// that variable, is two in the C (`version_loops`): a copy without those
// bounds checks where the bound is at most the length, its labels renamed,
// and the loop as it was otherwise. Both a `for` with a `continue` and a
// `foreach` over a range are copied; the program's output, and its bounds
// fault where the bound is over the length (with the location `coby`
// gives), are the same under gcc and clang.

#[path = "oracle/mod.rs"]
mod oracle;

const PROG: &str = r#"import std;
import std::env;

fn mark(ref<Vec<bool>, exclusive> v, usize limit)
{
    for (usize m = 0; m < limit; m += 3)
    {
        if (m % 2 == 0)
        {
            continue;
        }
        v[m] = true;
    }
}

fn count(ref<Vec<bool>, shared> v, usize n) : u64
{
    u64 c = 0;
    foreach (k in 0..n)
    {
        if (v[k])
        {
            c += 1;
        }
    }
    c
}

fn main() : u8
{
    String m = Result::unwrap_or(arg(0), String::new());
    Vec<bool> v = Vec::filled(30, false);
    usize lim = if (m == "over") { 40 } else { 30 };
    mark(&mut v, 30);
    printf("%d\n", count(&v, lim));
    0
}
"#;

#[test]
fn loops_copied_without_checks_their_bound_makes() {
    let dir = std::env::temp_dir().join(format!("cobc-loop-versions-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let case = dir.join("v.cb");
    std::fs::write(&case, PROG).unwrap();
    let c = dir.join("v.c");
    let out = std::process::Command::new(oracle::cobc_exe()).arg("--emit-c").arg("-o").arg(&c).arg(&case).output().expect("run cobc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&c).unwrap();
    // One copy for each loop, the `for`'s `continue` sent to its copy's label.
    let tests = text.lines().filter(|l| l.trim().starts_with("if (") && l.contains(" <= hl") && l.ends_with(") {")).count();
    assert_eq!(tests, 2, "the loops copied");
    assert!(text.lines().any(|l| l.trim().starts_with("goto cont") && l.contains("_v")), "a renamed label");
    assert!(!text.contains("/*@ver") && !text.contains("/*@vck"), "a mark left in the C");
    let line = PROG.lines().position(|l| l.contains("if (v[k])")).expect("marker") + 1;
    let run = oracle::coby_cmd().arg(&case).arg("over").output().expect("run coby");
    let want = String::from_utf8_lossy(&run.stderr).lines().take(2).collect::<Vec<_>>().join("\n");
    assert!(want.contains(&format!("v.cb:{}:", line)), "coby: {:?}", want);
    for (cc, r) in oracle::per_compiler(|cc| {
        let exe = dir.join(format!("v_{}", cc));
        let out = oracle::cobc(cc).arg("-o").arg(&exe).arg(&case).output().expect("run cobc");
        assert!(out.status.success(), "{}: {}", cc, String::from_utf8_lossy(&out.stderr));
        let ok = std::process::Command::new(&exe).arg("ok").output().expect("run the program");
        let over = std::process::Command::new(&exe).arg("over").output().expect("run the program");
        (String::from_utf8_lossy(&ok.stdout).to_string(), String::from_utf8_lossy(&over.stderr).lines().take(2).collect::<Vec<_>>().join("\n"))
    }) {
        assert_eq!(r.0, "5\n", "{}: the output", cc);
        assert_eq!(r.1, want, "{}: the fault's report", cc);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
