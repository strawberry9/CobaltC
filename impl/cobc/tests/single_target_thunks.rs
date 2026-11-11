// CHG-0436: a `fn` value's call thunk whose type only one code can reach
// calls that code directly (`single_target_thunks`). The program has one
// type with two targets (a fn item and a closure: the dispatch stays), and
// types with one each: a closure capturing nothing, one capturing a value,
// and a fn item. Each is called through its thunk; the output is the same
// under both C compilers, and the thunks take the forms expected.

#[path = "oracle/exe.rs"]
mod exe;

use std::process::Command;

const SRC: &str = "import std;

fn twice(u64 x) : u64
{
    x * 2
}

fn apply(fn(u64) : u64 f, u64 x) : u64
{
    f(x)
}

fn pick(fn(u32) : u32 f, u32 x) : u32
{
    f(x)
}

fn lift(fn(u16) : u64 f) : u64
{
    f(2)
}

fn u32_double(u32 x) : u32
{
    x * 2
}

fn less(fn(ref<u32, shared>, ref<u32, shared>) : bool f, u32 x, u32 y) : bool
{
    f(&x, &y)
}

fn main()
{
    u64 k = 40;
    u64 a = apply(twice, 5);
    u64 b = apply([](u64 x) : u64 { x + 1 }, 5);
    u32 c = pick(u32_double, 21);
    u64 d = lift([k](u16 x) : u64 { k + widen<u64>(x) });
    bool e = less([](ref<u32, shared> p, ref<u32, shared> q) : bool { *p < *q }, 3, 4);
    printf(\"%v %v %v %v %v\\n\", a, b, c, d, e);
}
";

// The body of the thunk `__i` function whose name starts `cb_call_<m>__i(`.
fn thunk_body<'a>(c: &'a str, m: &str) -> &'a str {
    let head = format!("cb_call_{}__i(void *h", m);
    let at = c.find(&head).unwrap_or_else(|| panic!("no thunk {}", m));
    let end = c[at..].find("\n}\n").map_or(c.len(), |e| at + e);
    &c[at..end]
}

#[test]
fn a_thunk_with_one_target_calls_it() {
    let dir = std::env::temp_dir().join(format!("cobc-single-target-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let case = dir.join("st.cb");
    std::fs::write(&case, SRC).unwrap();
    for cc in ["gcc", "clang"] {
        let exe = dir.join(format!("st_{}", cc));
        let out = Command::new(exe::cobc_exe()).args(["--cc", cc, "-o"]).arg(&exe).arg(&case).output().expect("run cobc");
        assert!(out.status.success(), "{}: {}", cc, String::from_utf8_lossy(&out.stderr));
        let run = Command::new(&exe).output().expect("run the program");
        assert!(run.status.success(), "{}: {}", cc, String::from_utf8_lossy(&run.stderr));
        assert_eq!(String::from_utf8_lossy(&run.stdout), "10 6 42 42 true\n", "{}", cc);
    }
    let c_path = dir.join("st.c");
    let out = Command::new(exe::cobc_exe()).args(["--emit-c", "-o"]).arg(&c_path).arg(&case).output().expect("run cobc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let c = std::fs::read_to_string(&c_path).unwrap();
    // Two targets: the box's kind and code decide.
    let two = thunk_body(&c, "fnu64_u64");
    assert!(two.contains("b->closure == 2u") && two.contains("b->code"), "{}", two);
    // A fn item alone: called by name, with no box read.
    let item = thunk_body(&c, "fnu32_u32");
    assert!(item.contains("f_u32_double)(a0)") && !item.contains("b->code"), "{}", item);
    // A closure capturing a value: its code, with the box's `self`.
    let cap = thunk_body(&c, "fnu16_u64");
    assert!(cap.contains("cb_call_self(b->root)") && cap.contains("))cl_") && !cap.contains("b->code"), "{}", cap);
    // A closure capturing nothing.
    let pure = thunk_body(&c, "fnrefs_u32_refs_u32_bool");
    assert!(pure.contains("))cl_") && !pure.contains("b->code"), "{}", pure);
    let _ = std::fs::remove_dir_all(&dir);
}
