// `print`'s exact output (spec/21 `rule.stdlib.print`), which the
// conformance cases' headers cannot state: `conf.print-output`'s file,
// run by the real `coby` binary. `cobc`'s output is compared with
// `coby`'s by cobc/tests/compiled_suite.rs, which runs the same file.

use std::path::Path;
use std::process::Command;

#[test]
fn print_writes_each_printable_type_exactly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let case = root.join("conformance/21-standard-library-semantics/print_output_ok.cb");
    let out = Command::new(env!("CARGO_BIN_EXE_coby")).arg(&case).output().expect("run coby");
    assert!(out.status.success(), "coby failed: {}", String::from_utf8_lossy(&out.stderr));
    let expected = "\
text
-42
0
340282366920938463463374607431768211455
-170141183460469231731687303715884105728
true
false
0.1
1.0
-0.0
0.3333333333333333
0.000001
1.0e-7
100000000000000000000.0
1.0e21
161305489493093.12
NaN
inf
-inf
0.1
0.33333334
owned
";
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}

// `eprintf` (spec/21 `rule.stdlib.format` [Eprintf]): its text goes to
// standard error, and nothing else does.
#[test]
fn eprintf_writes_to_standard_error() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let case = root.join("conformance/21-standard-library-semantics/eprintf_stderr_ok.cb");
    let out = Command::new(env!("CARGO_BIN_EXE_coby")).arg(&case).output().expect("run coby");
    assert!(out.status.success(), "coby failed: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "out 1\nout 2.5\n");
    assert_eq!(String::from_utf8_lossy(&out.stderr), "warning: odd input at line 7\n");
}
