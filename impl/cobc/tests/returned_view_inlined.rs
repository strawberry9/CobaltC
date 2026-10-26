// CHG-0372: a sub-view a function returns keeps its holder in the caller
// however the C compiler lays out the callee's storage. GCC at -O1 (and
// with LTO above -O0 since D-0277) inlines `head` and gives its dead
// local's storage to the caller's `h`; the result's temporary, which still
// named that storage until the statement ended, forgot `h`'s slot at its
// end, and the write beside the view went unreported. The conformance case
// is rejected before the run since D-0282, so the run's part is tested on
// the same program with the view held in a struct (tier D: the static pass
// leaves its conflicts to the run), compiled at -O1 and -O2 under both C
// compilers, with the fault expected each time.

#[path = "oracle/exe.rs"]
mod exe;

use std::process::Command;

#[test]
fn returned_subview_keeps_its_holder_when_the_callee_is_inlined() {
    let dir = std::env::temp_dir().join(format!("cobc-returned-view-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let case = dir.join("held.cb");
    let src = "import std;\n\nstruct Held\n{\n    StringView v;\n}\n\nfn head(StringView v) : StringView\n{\n    &v[0 .. 1]\n}\n\nfn main()\n{\n    String s = \"hello\";\n    Held h = Held { .v = head(String::as_view(&s)) };\n    printf(\"%v\\n\", h.v);\n    String::append(&mut s, \"!\");\n    printf(\"%v\\n\", h.v);\n}\n";
    std::fs::write(&case, src).unwrap();
    for cc in ["gcc", "clang"] {
        for opt in ["-O1", "-O2"] {
            let exe = dir.join(format!("rv_{}_{}", cc, &opt[1..]));
            let out = Command::new(exe::cobc_exe()).args([opt, "--cc", cc, "-o"]).arg(&exe).arg(&case).output().expect("run cobc");
            assert!(out.status.success(), "{} {}: {}", cc, opt, String::from_utf8_lossy(&out.stderr));
            let run = Command::new(&exe).output().expect("run the program");
            assert_eq!(String::from_utf8_lossy(&run.stdout), "h\n", "{} {}", cc, opt);
            let err = String::from_utf8_lossy(&run.stderr);
            assert!(
                err.starts_with("fault[diag.aliasing-conflict]"),
                "{} {}: expected the fault; status {:?}, stdout {:?}, stderr {:?}",
                cc,
                opt,
                run.status.code(),
                String::from_utf8_lossy(&run.stdout),
                err
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
