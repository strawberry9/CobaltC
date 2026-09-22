// `read_line`'s `Err(Io)` (spec/21 `rule.stdlib.read`), which a
// conformance case's `stdin-hex:` header cannot produce: standard input
// here is a directory, so reading it fails. Its other outcomes are the
// `read_line_*` cases of impl/conformance/21-standard-library-semantics/.

mod oracle;

use std::fs;
use std::process::Stdio;

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

#[test]
fn read_line_is_io_error_when_input_cannot_be_read() {
    let dir = std::env::temp_dir().join(format!("cobc-read-input-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let src = dir.join("read_io.cb");
    fs::write(&src, PROGRAM).unwrap();
    let results = oracle::per_compiler(|cc| {
        let input = fs::File::open(&dir).expect("open the directory as stdin");
        let out = oracle::cobc(cc).arg("--run").arg(&src).stdin(Stdio::from(input)).output().expect("run cobc");
        (out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
    });
    let _ = fs::remove_dir_all(&dir);
    for (cc, (code, stdout, stderr)) in results {
        assert_eq!((code, stdout.as_str()), (Some(0), "io\n"), "[{}] stderr: {}", cc, stderr);
    }
}
