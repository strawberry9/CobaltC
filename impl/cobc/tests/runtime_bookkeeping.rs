// cbrt's own records stay bounded (CHG-0093): a program that re-points
// one reference down a tree, and reads back the elements it pushed into
// a Vec, thousands of times, ends with almost nothing still recorded.
// Before the fix each walk left a path behind on the tree's object and
// each element read left an orphaned object. The counts are what cbrt
// reports at `[Terminate-Ok]` when `COBALTC_RT_STATS` is set.

use std::fs;
use std::process::Command;

const PROGRAM: &str = r#"import std;

struct N
{
    Vec<i64> keys;
    Vec<N> kids;
}

fn make(i64 depth) : N
{
    N n = N { .keys = Vec::new(), .kids = Vec::new() };
    Vec::push(&mut n.keys, depth);
    if (depth > 0)
    {
        Vec::push(&mut n.kids, make(depth - 1));
        Vec::push(&mut n.kids, make(depth - 1));
    }
    n
}

fn deepest(ref<N, shared> n) : i64
{
    ref<N, shared> cur = n;
    while (Vec::len(&cur.kids) > 0)
    {
        cur = Vec::index_shared(&cur.kids, 1);
    }
    cur.keys[0]
}

fn main()
{
    N root = make(5);
    i64 sum = 0;
    for (u32 i = 0; i < 2000; i += 1)
    {
        sum += deepest(&root);
        sum += Vec::index_shared(&root.kids, 0).keys[0];
    }
    printf("%v\n", sum);
}
"#;

fn live_at_exit(cc: &str, dir: &std::path::Path) -> (u64, u64) {
    let exe = dir.join(format!("walk_{}", cc));
    let out = Command::new(env!("CARGO_BIN_EXE_cobc"))
        .args(["--cc", cc, "-o"])
        .arg(&exe)
        .arg(dir.join("walk.cb"))
        .output()
        .expect("run cobc");
    assert!(out.status.success(), "{}: {}", cc, String::from_utf8_lossy(&out.stderr));
    let run = Command::new(&exe).env("COBALTC_RT_STATS", "1").output().expect("run the program");
    assert!(run.status.success(), "{}: {}", cc, String::from_utf8_lossy(&run.stderr));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "8000\n", "{}", cc);
    let err = String::from_utf8_lossy(&run.stderr).to_string();
    let line = err.lines().find(|l| l.starts_with("cbrt: live at exit:")).unwrap_or_else(|| panic!("{}: no stats line in {:?}", cc, err));
    let nums: Vec<u64> = line.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
    (nums[0], nums[1])
}

#[test]
fn runtime_records_stay_bounded_under_gcc_and_clang() {
    let dir = std::env::temp_dir().join(format!("cobc-bookkeeping-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("walk.cb"), PROGRAM).unwrap();
    for cc in ["gcc", "clang"] {
        let (objects, paths) = live_at_exit(cc, &dir);
        assert!(objects < 16 && paths < 16, "{}: {} objects and {} paths still recorded at exit", cc, objects, paths);
    }
    let _ = fs::remove_dir_all(&dir);
}
