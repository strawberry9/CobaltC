// CHG-0372: a sub-view a function returns keeps its holder in the caller
// however the C compiler lays out the callee's storage. GCC at -O1 (and
// with LTO above -O0 since D-0277) inlines `head` and gives its dead
// local's storage to the caller's `h`; the result's temporary, which still
// named that storage until the statement ended, forgot `h`'s slot at its
// end, and the write beside the view went unreported. The conformance case
// is compiled at -O1 and -O2 under both C compilers, with the fault
// expected each time.

#[path = "oracle/exe.rs"]
mod exe;

use std::process::Command;

#[test]
fn returned_subview_keeps_its_holder_when_the_callee_is_inlined() {
    let case = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../conformance/21-standard-library-semantics/returned_subview_beside_write_rejected.cb");
    let dir = std::env::temp_dir().join(format!("cobc-returned-view-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for cc in ["gcc", "clang"] {
        for opt in ["-O1", "-O2"] {
            let exe = dir.join(format!("rv_{}_{}", cc, &opt[1..]));
            let out = Command::new(exe::cobc_exe()).args([opt, "--cc", cc, "-o"]).arg(&exe).arg(&case).output().expect("run cobc");
            assert!(out.status.success(), "{} {}: {}", cc, opt, String::from_utf8_lossy(&out.stderr));
            let run = Command::new(&exe).output().expect("run the program");
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
