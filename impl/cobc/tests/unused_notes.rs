// D-0309: a frame's notes (D-0269, `cb_nt`) are kept only where the
// program can spawn a thread (`drop_unused_notes`): a program that never
// calls `cb_spawn` has none, and one that does keeps them. Both run as
// `coby` runs them, under gcc and clang.

#[path = "oracle/exe.rs"]
mod exe;

use std::process::Command;

const ALONE: &str = r#"import std;

fn count(usize limit) : u64
{
    Vec<bool> marks = Vec::filled(limit, true);
    String name = String::copy("marks");
    for (usize m = 0; m < limit; m += 3)
    {
        marks[m] = false;
    }
    u64 n = 0;
    foreach (k in 0..limit)
    {
        if (marks[k])
        {
            n += 1;
        }
    }
    n + narrow<u64>(StringView::len(&name))
}

fn main()
{
    printf("%v\n", count(1000));
}
"#;

const THREADED: &str = r#"import std;

fn count(usize limit) : u64
{
    Vec<bool> marks = Vec::filled(limit, true);
    for (usize m = 0; m < limit; m += 3)
    {
        marks[m] = false;
    }
    u64 n = 0;
    foreach (k in 0..limit)
    {
        if (marks[k])
        {
            n += 1;
        }
    }
    n
}

fn main()
{
    u64 a = join(spawn(count, 1000));
    printf("%v %v\n", a, count(30));
}
"#;

fn case(dir: &std::path::Path, name: &str, src: &str, notes: bool, stdout: &str) {
    let path = dir.join(format!("{}.cb", name));
    std::fs::write(&path, src).unwrap();
    let c = dir.join(format!("{}.c", name));
    let out = Command::new(exe::cobc_exe()).arg("--emit-c").arg("-o").arg(&c).arg(&path).output().expect("run cobc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&c).unwrap();
    assert_eq!(text.contains("cb_nt["), notes, "{}: notes {} expected", name, if notes { "were" } else { "were not" });
    for cc in ["gcc", "clang"] {
        let exe = dir.join(format!("{}_{}", name, cc));
        let out = Command::new(exe::cobc_exe()).args(["--cc", cc, "-o"]).arg(&exe).arg(&path).output().expect("run cobc");
        assert!(out.status.success(), "{} {}: {}", name, cc, String::from_utf8_lossy(&out.stderr));
        let run = Command::new(&exe).current_dir(dir).output().expect("run the program");
        assert_eq!(String::from_utf8_lossy(&run.stdout), stdout, "{} {}", name, cc);
        assert!(run.stderr.is_empty(), "{} {}: {}", name, cc, String::from_utf8_lossy(&run.stderr));
    }
}

#[test]
fn notes_only_where_a_thread_can_be_spawned() {
    let dir = std::env::temp_dir().join(format!("cobc-unused-notes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    case(&dir, "alone", ALONE, false, "671\n");
    case(&dir, "threaded", THREADED, true, "666 20\n");
    let _ = std::fs::remove_dir_all(&dir);
}
