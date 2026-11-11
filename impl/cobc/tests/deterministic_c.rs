// The C cobc writes for a program depends on the program alone. The static
// pass and the lowering key their facts about an expression by its address;
// before the session findings of 2026-11-06 an expression they made and
// freed (a `foreach` expansion) left its facts behind, and a later node at
// the same address took them, so the same file was decided in one build and
// not the next, or failed with an internal error ("a StringView operation of
// an unexpected shape"). The case below (a conformance case too,
// `conf.foreach-view-loops-checked-alike`) changed between builds before the
// fix; it is emitted several times and every C must be the same.

#[path = "oracle/exe.rs"]
mod exe;

use std::process::Command;

#[test]
fn the_c_of_a_program_is_the_same_on_every_build() {
    let case = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../conformance/14-control-flow/foreach_view_loops_checked_alike_ok.cb");
    let dir = std::env::temp_dir().join(format!("cobc-deterministic-c-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut first: Option<Vec<u8>> = None;
    for k in 0..4 {
        let out_c = dir.join("case.c");
        let out = Command::new(exe::cobc_exe()).arg("--emit-c").arg("-o").arg(&out_c).arg(&case).output().expect("run cobc");
        assert!(out.status.success(), "build {}: {}", k, String::from_utf8_lossy(&out.stderr));
        let c = std::fs::read(&out_c).unwrap();
        assert!(String::from_utf8_lossy(&c).contains("#define CB_DECIDED 1"), "build {}: not decided", k);
        match &first {
            None => first = Some(c),
            Some(f) => assert!(*f == c, "build {} wrote other C than build 0", k),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
