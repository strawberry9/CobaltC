// A loop that names a confined `Vec` local only to index it or ask its
// length reads the vector's `ptr` and `len` from copies made before it
// (`hoist_vec_headers`), a nested loop from its own, and a loop that reads
// none of its copies keeps none; one that pushes onto it, assigns it or
// hands it on reads the vector's header as before. The
// emitted C copies exactly the vectors it may, and the program's output
// and its bounds fault (with the location `coby` gives) are the same under
// gcc and clang.

#[path = "oracle/exe.rs"]
mod exe;

use std::process::Command;

const PROG: &str = r#"import std;

fn sum(ref<Vec<u64>, shared> v) : u64
{
    u64 s = 0;
    foreach (x in v)
    {
        s += *x;
    }
    s
}

fn main() : u8
{
    Vec<u64> a = Vec::filled(10, 1);
    // Fixed header: written and read by index.
    for (usize i = 1; i < Vec::len(&a); i += 1)
    {
        a[i] = a[i - 1] * 2;
    }
    printf("%d\n", a[9]);
    // A push inside: the header changes, no copy may be used.
    Vec<u64> b = Vec::new();
    Vec::push(&mut b, 1);
    usize k = 0;
    while (k < 40)
    {
        Vec::push(&mut b, b[k] + 1);
        k += 1;
    }
    printf("%d %d\n", Vec::len(&b), b[40]);
    // Nested loops, an inner shadowing name.
    Vec<u8> c = Vec::filled(16, 0);
    for (usize i = 0; i < 16; i += 1)
    {
        for (usize j = i; j < 16; j += 1)
        {
            c[j] = c[j] + 1;
        }
        Vec<u8> c = Vec::filled(2, 7);
        c[1] = 9;
        printf("%d", c[1]);
    }
    printf("\n%d %d\n", c[0], c[15]);
    // Reassigned inside: no copy.
    Vec<u64> d = Vec::filled(3, 5);
    for (usize r = 0; r < 3; r += 1)
    {
        d[0] = d[0] + 1;
        if (r == 1)
        {
            d = Vec::filled(5, 2);
        }
    }
    printf("%d %d\n", Vec::len(&d), d[0]);
    printf("%d\n", sum(&a));
    // Out of bounds in a copied loop.
    Vec<bool> e = Vec::filled(5, true);
    usize n = 0;
    for (usize m = 0; m < 8; m += 1)
    {
        if (e[m])
        {
            n += 1;
        }
    }
    printf("%d\n", n);
    0
}
"#;

// The locals whose header the emitted C copies, by source name, once per
// copy (`(l_a_1).ptr` is `a`).
fn copied(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if !t.ends_with(").ptr;") || !t.contains(" *hp") {
            continue;
        }
        let Some(at) = t.find("= (l_") else { continue };
        let name = &t[at + 5..t.len() - 6];
        let Some(cut) = name.rfind('_') else { continue };
        out.push(name[..cut].to_string());
    }
    out
}

#[test]
fn loops_read_a_fixed_vector_header_from_copies() {
    let dir = std::env::temp_dir().join(format!("cobc-loop-header-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let case = dir.join("h.cb");
    std::fs::write(&case, PROG).unwrap();
    let c = dir.join("h.c");
    let out = Command::new(exe::cobc_exe()).arg("--emit-c").arg("-o").arg(&c).arg(&case).output().expect("run cobc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&c).unwrap();
    // `a`'s loop, `c`'s inner loop, `e`'s loop; not `b` (pushed onto), `d`
    // (assigned) or the inner `c` (declared in the loop), and not `c`'s
    // outer loop, which indexes it only in the inner one (a copy no access
    // reads is taken out).
    assert_eq!(copied(&text), ["a", "c", "e"], "the copies made");
    let want = "512\n41 41\n9999999999999999\n1 16\n5 3\n1023\n";
    for cc in ["gcc", "clang"] {
        let exe = dir.join(format!("h_{}", cc));
        let out = Command::new(exe::cobc_exe()).args(["--cc", cc, "-o"]).arg(&exe).arg(&case).output().expect("run cobc");
        assert!(out.status.success(), "{}: {}", cc, String::from_utf8_lossy(&out.stderr));
        let run = Command::new(&exe).output().expect("run the program");
        assert_eq!(String::from_utf8_lossy(&run.stdout), want, "{}", cc);
        let err = String::from_utf8_lossy(&run.stderr);
        assert!(err.starts_with("fault[diag.index-out-of-bounds]") && err.contains("h.cb:62:9\n"), "{}: expected the fault at 62:9; stderr {:?}", cc, err);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
