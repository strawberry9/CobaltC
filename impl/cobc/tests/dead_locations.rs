// The location a fault without one of its own reports (`cb_at`) is set
// only where some path from it reaches a line that could read it before
// another sets it (`drop_dead_locations`, over the body's flow of
// control): a loop whose bounds checks and overflow checks carry their own
// locations, and whose push sets its own where it can grow, keeps none,
// and a fault after a call, inside `std` (`Option::unwrap`) or after an
// `if`-pattern's jump is reported where `coby` reports it, under gcc and
// clang.

#[path = "oracle/mod.rs"]
mod oracle;

const PROG: &str = r#"import std;
import std::env;

fn pick(usize i) : usize
{
    i * 3
}

fn count(ref<Vec<bool>, shared> v) : u64
{
    u64 n = 0;
    foreach (k in 0..Vec::len(v))
    {
        if (v[k])
        {
            n += 1;
        }
    }
    n
}

fn fill(ref<Vec<bool>, exclusive> v, usize n)
{
    foreach (i in 0..n)
    {
        Vec::push(v, i % 3 == 0);
    }
}

fn first_even(ref<Vec<u64>, shared> w) : Option<u64>
{
    foreach (x in w)
    {
        if (*x % 2 == 0)
        {
            return Some(*x);
        }
    }
    None
}

fn main() : u8
{
    String m = Result::unwrap_or(arg(0), String::new());
    Vec<bool> v = Vec::new();
    fill(&mut v, 10);
    Vec<u64> w = Vec::filled(5, 1);
    u64 s = 0;
    foreach (i in 0..4)
    {
        if (m == "call" && i == 2)
        {
            s += w[pick(i)];
        }
        else
        {
            s += w[i];
        }
    }
    foreach (i in 0..3)
    {
        u64 t = s + 1;
        if (m == "unwrap" && i == 1)
        {
            s += Option::unwrap(first_even(&w));
        }
        s = t;
    }
    foreach (i in 0..3)
    {
        if (Some(e) = first_even(&w))
        {
            s += e;
        }
        if (m == "after-if" && i == 2)
        {
            s += w[9];
        }
    }
    printf("%d %d\n", count(&v), s);
    0
}
"#;

// The body of the C function `name`, or of its form for a confined
// argument (`name_c1`): its lines up to the closing brace.
fn body<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    let named = |l: &str| l.contains(&format!(" {}(", name)) || l.contains(&format!(" {}_c1(", name));
    let mut lines = text.lines().skip_while(|l| !(l.starts_with("static ") && named(l) && !l.ends_with(';')));
    let mut out = Vec::new();
    lines.next();
    for l in lines {
        if l == "}" {
            break;
        }
        out.push(l);
    }
    out
}

// A fault report's first two lines: what and where.
fn head(err: &str) -> String {
    err.lines().take(2).collect::<Vec<_>>().join("\n")
}

#[test]
fn locations_set_only_where_a_path_reads_them() {
    let dir = std::env::temp_dir().join(format!("cobc-dead-locations-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let case = dir.join("d.cb");
    std::fs::write(&case, PROG).unwrap();
    let c = dir.join("d.c");
    let out = std::process::Command::new(oracle::cobc_exe()).arg("--emit-c").arg("-o").arg(&c).arg(&case).output().expect("run cobc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&c).unwrap();
    for f in ["f_count", "f_fill"] {
        let b = body(&text, f);
        assert!(!b.is_empty(), "{} not found", f);
        let kept: Vec<&&str> = b.iter().filter(|l| l.trim_start().starts_with("cb_at(")).collect();
        assert!(kept.is_empty(), "{} keeps locations {:?}", f, kept);
    }
    // Each fault where `coby` reports it.
    let line_of = |needle: &str| PROG.lines().position(|l| l.contains(needle)).expect("marker") + 1;
    let cases = [("call", line_of("s += w[pick(i)];")), ("unwrap", line_of("s += Option::unwrap(")), ("after-if", line_of("s += w[9];"))];
    let mut want = Vec::new();
    for (m, line) in cases {
        let run = oracle::coby_cmd().arg(&case).arg(m).output().expect("run coby");
        let err = String::from_utf8_lossy(&run.stderr).to_string();
        assert!(err.contains(&format!("d.cb:{}:", line)), "coby {}: expected line {}; stderr {:?}", m, line, err);
        want.push((m, head(&err)));
    }
    for (cc, r) in oracle::per_compiler(|cc| {
        let exe = dir.join(format!("d_{}", cc));
        let out = oracle::cobc(cc).arg("-o").arg(&exe).arg(&case).output().expect("run cobc");
        assert!(out.status.success(), "{}: {}", cc, String::from_utf8_lossy(&out.stderr));
        let mut got = Vec::new();
        let run = std::process::Command::new(&exe).arg("ok").output().expect("run the program");
        got.push(("", String::from_utf8_lossy(&run.stdout).to_string()));
        for (m, _) in &want {
            let run = std::process::Command::new(&exe).arg(m).output().expect("run the program");
            got.push((*m, head(&String::from_utf8_lossy(&run.stderr))));
        }
        got
    }) {
        assert_eq!(r[0].1, "4 7\n", "{}: the output", cc);
        for (k, (m, w)) in want.iter().enumerate() {
            assert_eq!(&r[k + 1].1, w, "{} {}: the fault's report", cc, m);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
