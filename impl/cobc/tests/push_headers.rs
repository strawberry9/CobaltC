// D-0309: a loop whose one use of a vector is a confined push, and which
// calls nothing else and can fault nowhere, keeps the vector's header in
// locals (`keep_push_headers`): the length is stored back before `grow`
// and after the loop, its end or a `break`. A loop that calls `printf`, or
// whose pushed value can overflow, keeps the push as a call. The program's
// output, and the overflow's report (with the location `coby` gives), are
// the same under gcc and clang.

#[path = "oracle/mod.rs"]
mod oracle;

const PROG: &str = r#"import std;
import std::env;

fn fill(ref<Vec<u8>, exclusive> v, usize n)
{
    foreach (i in 0..n)
    {
        Vec::push(v, narrow_wrapping<u8>(i));
    }
}

fn fill_until(ref<Vec<u8>, exclusive> v, usize n, usize stop)
{
    foreach (i in 0..n)
    {
        if (i == stop)
        {
            break;
        }
        Vec::push(v, 7);
    }
}

fn fill_logged(ref<Vec<i64>, exclusive> v, usize n)
{
    foreach (i in 0..n)
    {
        if (i == 3)
        {
            printf("at %v\n", i);
        }
        Vec::push(v, 1);
    }
}

fn fill_scaled(ref<Vec<i64>, exclusive> v, usize n, i64 k)
{
    foreach (i in 0..n)
    {
        Vec::push(v, k * narrow<i64>(i));
    }
}

fn main() : u8
{
    String m = Result::unwrap_or(arg(0), String::new());
    Vec<u8> a = Vec::new();
    fill(&mut a, 100000);
    u64 s = 0;
    foreach (x in &a)
    {
        s += widen<u64>(*x);
    }
    Vec<u8> b = Vec::new();
    fill_until(&mut b, 5000, 1234);
    Vec<i64> c = Vec::new();
    fill_logged(&mut c, 10);
    Vec<i64> d = Vec::new();
    i64 k = if (m == "over") { 4611686018427387904 } else { 3 };
    fill_scaled(&mut d, 10, k);
    Vec<i64> w = Vec::new();
    foreach (i in 0..70000: i64)
    {
        Vec::push(&mut w, i);
    }
    printf("%v %v %v %v %v %v %v\n", Vec::len(&a), s, Vec::len(&b), Vec::len(&c), d[9], Vec::len(&w), w[69999]);
    0
}
"#;

// The body of the C function `name` (from its signature to its closing `}`).
fn body<'t>(text: &'t str, name: &str) -> &'t str {
    let start = text.lines().position(|l| !l.starts_with(' ') && l.contains(&format!(" {}(", name)) && !l.ends_with(';')).map(|i| text.lines().take(i).map(|l| l.len() + 1).sum::<usize>()).unwrap_or_else(|| panic!("no {}", name));
    let rest = &text[start..];
    &rest[..rest.find("\n}\n").map_or(rest.len(), |e| e + 3)]
}

#[test]
fn push_loops_keep_the_header_in_locals() {
    let dir = std::env::temp_dir().join(format!("cobc-push-headers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let case = dir.join("p.cb");
    std::fs::write(&case, PROG).unwrap();
    let c = dir.join("p.c");
    let out = std::process::Command::new(oracle::cobc_exe()).arg("--emit-c").arg("-o").arg(&c).arg(&case).output().expect("run cobc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&c).unwrap();
    let kept = |name: &str| body(&text, name).contains("uint64_t kh");
    assert!(kept("f_fill_c1"), "fill's loop keeps no header:\n{}", body(&text, "f_fill_c1"));
    assert!(kept("f_fill_until_c1"), "fill_until's loop keeps no header");
    assert!(kept("f_main"), "main's loop keeps no header");
    assert!(!kept("f_fill_logged_c1"), "a loop that calls printf keeps a header");
    assert!(!kept("f_fill_scaled_c1"), "a loop that can fault keeps a header");
    let line = PROG.lines().position(|l| l.contains("k * narrow<i64>(i)")).expect("marker") + 1;
    let run = oracle::coby_cmd().arg(&case).arg("over").output().expect("run coby");
    let want = String::from_utf8_lossy(&run.stderr).lines().take(2).collect::<Vec<_>>().join("\n");
    assert!(want.contains(&format!("p.cb:{}:", line)), "coby: {:?}", want);
    let ok = oracle::coby_cmd().arg(&case).arg("ok").output().expect("run coby");
    let expect = String::from_utf8_lossy(&ok.stdout).to_string();
    assert_eq!(expect, "at 3\n100000 12742320 1234 10 27 70000 69999\n", "coby's output");
    for (cc, r) in oracle::per_compiler(|cc| {
        let exe = dir.join(format!("p_{}", cc));
        let out = oracle::cobc(cc).arg("-o").arg(&exe).arg(&case).output().expect("run cobc");
        assert!(out.status.success(), "{}: {}", cc, String::from_utf8_lossy(&out.stderr));
        let ok = std::process::Command::new(&exe).arg("ok").output().expect("run the program");
        let over = std::process::Command::new(&exe).arg("over").output().expect("run the program");
        (String::from_utf8_lossy(&ok.stdout).to_string(), String::from_utf8_lossy(&over.stderr).lines().take(2).collect::<Vec<_>>().join("\n"))
    }) {
        assert_eq!(r.0, expect, "{}: the output", cc);
        assert_eq!(r.1, want, "{}: the fault's report", cc);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
