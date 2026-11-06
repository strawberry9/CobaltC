// D-0309 (`[Call-Self-Tail]`, spec/15 §2a): a self tail call is not a level
// of nesting. `down(100000000)` and a 20 000-node list appended to and walked
// through reference parameters (each step a call whose argument comes from a
// `match` on the parameter's field) complete under gcc and clang, the calls
// lowered as jumps (`goto self_tail;`); mutual recursion, a frame holding a
// `String`, and one whose local is borrowed still fault
// `diag.stack-exhausted`.

#[path = "oracle/exe.rs"]
mod exe;

use std::process::Command;

const PROG: &str = r#"import std;
import std::env;

struct Node
{
    i64 key;
    Option<Box<Node>> next;
}

fn down(u64 n) : bool
{
    if (n == 0) { true } else { down(n - 1) }
}

fn append(ref<Node, exclusive> n, i64 k)
{
    match (&mut n.next)
    {
        Some(b) : append(Box::get_mut(b), k),
        None    : n.next = Some(Box::new(Node { .key = k, .next = None })),
    }
}

fn length(ref<Node, shared> n, u64 acc) : u64
{
    match (&n.next)
    {
        Some(b) : length(Box::get(b), acc + 1),
        None    : acc + 1,
    }
}

fn even(u64 n) : bool
{
    if (n == 0) { true } else { odd(n - 1) }
}

fn odd(u64 n) : bool
{
    if (n == 0) { false } else { even(n - 1) }
}

fn with_string(u64 n) : u64
{
    String s = String::new();
    if (n == 0) { 0 } else { with_string(n - 1) }
}

fn with_borrow(u64 n) : u64
{
    u64 x = n;
    ref<u64, shared> r = &x;
    if (*r == 0) { 0 } else { with_borrow(n - 1) }
}

fn main() : u8
{
    String m = Result::unwrap_or(arg(0), String::new());
    if (m == "down") { printf("%v\n", down(100000000)); }
    if (m == "list")
    {
        Node head = Node { .key = 0, .next = None };
        for (i64 i = 1; i < 20000; i += 1)
        {
            append(&mut head, i);
        }
        printf("%v\n", length(&head, 0));
    }
    if (m == "mutual") { printf("%v\n", even(100000000)); }
    if (m == "string") { printf("%v\n", with_string(100000000)); }
    if (m == "borrow") { printf("%v\n", with_borrow(100000000)); }
    0
}
"#;

#[test]
fn self_tail_calls_are_no_level_of_nesting() {
    let dir = std::env::temp_dir().join(format!("cobc-self-tail-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let case = dir.join("t.cb");
    std::fs::write(&case, PROG).unwrap();
    let c = dir.join("t.c");
    let out = Command::new(exe::cobc_exe()).arg("--emit-c").arg("-o").arg(&c).arg(&case).output().expect("run cobc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&c).unwrap();
    assert!(text.matches("goto self_tail;").count() >= 3, "down, append and length jump");
    for cc in ["gcc", "clang"] {
        let exe = dir.join(format!("t_{}", cc));
        let out = Command::new(exe::cobc_exe()).args(["--cc", cc, "-o"]).arg(&exe).arg(&case).output().expect("run cobc");
        assert!(out.status.success(), "{}: {}", cc, String::from_utf8_lossy(&out.stderr));
        for (m, want) in [("down", "true\n"), ("list", "20000\n")] {
            let run = Command::new(&exe).arg(m).output().expect("run the program");
            assert_eq!(String::from_utf8_lossy(&run.stdout), want, "{} {}: stderr {}", cc, m, String::from_utf8_lossy(&run.stderr));
        }
        for m in ["mutual", "string", "borrow"] {
            let run = Command::new(&exe).arg(m).output().expect("run the program");
            let err = String::from_utf8_lossy(&run.stderr);
            assert!(err.starts_with("fault[diag.stack-exhausted]"), "{} {}: expected the fault; stderr {:?}", cc, m, err);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
