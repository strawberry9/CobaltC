// `read_line`'s `Err(Io)` under `coby` (spec/21 `rule.stdlib.read`,
// D-0050), which a conformance case's `stdin-hex:` header cannot
// produce: standard input here is a directory, so reading it fails. As
// cobc/tests/read_input.rs does for `cobc`. Only where a directory can be
// opened as a file (Unix); the case compiles everywhere.

#[cfg(unix)]
#[test]
fn read_line_is_io_error_when_input_cannot_be_read() {
    use std::fs;
    use std::process::{Command, Stdio};

    const PROGRAM: &str = "\
import std;

fn main()
{
    match (read_line())
    {
        Ok(_) :
        {
            printf(\"ok\\n\");
        },
        Err(e) :
        {
            match (e)
            {
                Io :
                {
                    printf(\"io\\n\");
                },
                Utf8(_) :
                {
                    printf(\"utf8\\n\");
                },
            }
        },
    }
}
";
    let dir = std::env::temp_dir().join(format!("coby-read-input-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let src = dir.join("read_io.cb");
    fs::write(&src, PROGRAM).unwrap();
    let input = fs::File::open(&dir).expect("open the directory as stdin");
    let out = Command::new(env!("CARGO_BIN_EXE_coby")).arg(&src).stdin(Stdio::from(input)).output().expect("run coby");
    let _ = fs::remove_dir_all(&dir);
    assert_eq!((out.status.code(), String::from_utf8_lossy(&out.stdout).as_ref()), (Some(0), "io\n"), "stderr: {}", String::from_utf8_lossy(&out.stderr));
}

// Input read by a program that also runs a thread: the read releases the
// interpreter's lock while it waits, and both finish.
#[test]
fn a_thread_runs_while_input_is_read() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    const PROGRAM: &str = "\
import std;

fn count(u64 n) : u64
{
    u64 s = 0;
    u64 i = 0;
    while (i < n)
    {
        s = s + i;
        i = i + 1;
    }
    s
}

fn main()
{
    auto h = spawn(count, 1000);
    match (read_line())
    {
        Ok(o) : match (o)
        {
            Some(line) : printf(\"%v \", &line),
            None : printf(\"none \"),
        },
        Err(_) : printf(\"err \"),
    }
    printf(\"%v\\n\", join(h));
}
";
    let dir = std::env::temp_dir().join(format!("coby-read-thread-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("read_thread.cb");
    std::fs::write(&src, PROGRAM).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_coby"))
        .arg(&src)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run coby");
    child.stdin.take().unwrap().write_all(b"hello\n").unwrap();
    let out = child.wait_with_output().expect("run coby");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hello 499500\n", "stderr: {}", String::from_utf8_lossy(&out.stderr));
}
