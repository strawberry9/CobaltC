// D-0178 (spec/21 `rule.stdlib.print` [Print-Buffer]): a compiled
// program's standard output is buffered, and a write to standard error,
// a fault's report and the program's end write it out first, so with
// both streams sent to one file the program's order is kept. The
// conformance cases' runner reads the two streams apart and cannot see
// this; impl/tests/print_output.rs checks the same for `coby`.

mod oracle;

use std::fs;

fn run_both_to_one_file(name: &str, src: &str) -> Vec<(String, (Option<i32>, String))> {
    let dir = std::env::temp_dir().join(format!("cobc-output-order-{}-{}", name, std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let prog = dir.join(format!("{}.cb", name));
    fs::write(&prog, src).unwrap();
    let results = oracle::per_compiler(|cc| {
        let path = dir.join(format!("both-{}.txt", cc.replace('/', "_")));
        let file = fs::File::create(&path).unwrap();
        let status = oracle::cobc(cc).arg("--run").arg(&prog).stdout(file.try_clone().unwrap()).stderr(file).status().expect("run cobc");
        (status.code(), fs::read_to_string(&path).unwrap())
    });
    let _ = fs::remove_dir_all(&dir);
    results.into_iter().map(|(cc, r)| (cc.to_string(), r)).collect()
}

#[test]
fn eprintf_follows_buffered_printf() {
    let src = "import std;\n\nfn main()\n{\n    printf(\"a\\n\");\n    eprintf(\"b\\n\");\n    printf(\"c\\n\");\n}\n";
    for (cc, (code, both)) in run_both_to_one_file("order", src) {
        assert_eq!((code, both.as_str()), (Some(0), "a\nb\nc\n"), "[{}]", cc);
    }
}

#[test]
fn fault_report_follows_buffered_printf() {
    let src = "import std;\n\nfn main()\n{\n    printf(\"before\\n\");\n    Vec<i32> v = Vec::new();\n    printf(\"%v\\n\", v[0]);\n}\n";
    for (cc, (code, both)) in run_both_to_one_file("fault", src) {
        assert_eq!(code, Some(1), "[{}] {}", cc, both);
        assert!(both.starts_with("before\nerror: diag.index-out-of-bounds"), "[{}] {}", cc, both);
    }
}
