// `extern "…";` (spec/20 `rule.trust.extern-code`, D-0030) as `cobc`
// reads it (cobc/src/link.rs): every kind of file, a library name, a path
// relative to the module file that declares it, duplicates, a shared
// library found from another working directory, and each error reported
// at its declaration. The conformance case
// 20-trust-boundaries/extern_code_c_file_ok.cb covers a `.c` file.

mod oracle;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch(cc: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cobc-extern-code-{}-{}-{}", std::process::id(), cc, name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("vendor")).unwrap();
    fs::create_dir_all(dir.join("lib")).unwrap();
    dir
}

fn sh(cc: &str, dir: &Path, args: &[&str]) {
    let st = Command::new(cc).args(args).current_dir(dir).status().expect("run the C compiler");
    assert!(st.success(), "[{}] {:?} failed", cc, args);
}

const MAIN: &str = "\
import std;

module geo \"./lib/geo.cb\";
module geo2 \"./lib/geo2.cb\";

extern \"./vendor/twice.o\";
extern \"./vendor/libplus1.a\";
extern \"./vendor/libneg.so\";
extern \"m\";
extern \"m\";

extern fn twice(i64 x) : i64;
extern fn plus1(i64 x) : i64;
extern fn neg(i64 x) : i64;
extern fn sqrt(f64 x) : f64;

fn main()
{
    printf(\"%v\", geo::rect(6, 7));
    printf(\" \");
    printf(\"%v\", unsafe { neg(plus1(twice(20))) });
    printf(\" \");
    printf(\"%v\", unsafe { sqrt(2.25) });
    printf(\"\\n\");
}
";

#[test]
fn every_kind_of_extern_code_links_and_runs_from_any_directory() {
    let results = oracle::per_compiler(|cc| {
        let dir = scratch(cc, "forms");
        fs::write(dir.join("vendor/shapes.c"), "#include <stdint.h>\nint32_t area(int32_t w, int32_t h) { return w * h; }\n").unwrap();
        fs::write(dir.join("vendor/twice.c"), "#include <stdint.h>\nint64_t twice(int64_t x) { return 2 * x; }\n").unwrap();
        fs::write(dir.join("vendor/plus1.c"), "#include <stdint.h>\nint64_t plus1(int64_t x) { return x + 1; }\n").unwrap();
        fs::write(dir.join("vendor/neg.c"), "#include <stdint.h>\nint64_t neg(int64_t x) { return -x; }\n").unwrap();
        sh(cc, &dir, &["-c", "vendor/twice.c", "-o", "vendor/twice.o"]);
        sh(cc, &dir, &["-c", "vendor/plus1.c", "-o", "vendor/plus1.o"]);
        let st = Command::new("ar").args(["rcs", "vendor/libplus1.a", "vendor/plus1.o"]).current_dir(&dir).status().expect("run ar");
        assert!(st.success());
        sh(cc, &dir, &["-shared", "-fPIC", "vendor/neg.c", "-o", "vendor/libneg.so"]);
        // A path in a module file is relative to that file (lib/); two
        // modules naming the same file link it once.
        fs::write(
            dir.join("lib/geo.cb"),
            "import std;\n\nextern \"../vendor/shapes.c\";\n\nextern fn area(i32 w, i32 h) : i32;\n\nexport fn rect(u8 w, u8 h) : i32\n{\n    unsafe { area(widen<i32>(w), widen<i32>(h)) }\n}\n",
        )
        .unwrap();
        fs::write(dir.join("lib/geo2.cb"), "extern \"../vendor/shapes.c\";\n").unwrap();
        fs::write(dir.join("main.cb"), MAIN).unwrap();
        let exe = dir.join("main.bin");
        let out = oracle::cobc(cc).arg("-o").arg(&exe).arg(dir.join("main.cb")).output().expect("run cobc");
        let built = (out.status.code(), String::from_utf8_lossy(&out.stderr).into_owned());
        // Run from `/`: the shared library is found through the rpath.
        let run = Command::new(&exe).current_dir("/").output().expect("run the program");
        let _ = fs::remove_dir_all(&dir);
        (built, run.status.code(), String::from_utf8_lossy(&run.stdout).into_owned())
    });
    for (cc, (built, code, stdout)) in results {
        assert_eq!(built.0, Some(0), "[{}] cobc: {}", cc, built.1);
        assert_eq!((code, stdout.as_str()), (Some(0), "42 -41 1.5\n"), "[{}]", cc);
    }
}

// (declaration, call, expected exit, expected stderr fragments)
const ERRORS: &[(&str, &str, i32, &[&str])] = &[
    ("extern \"./absent.c\";", "", 2, &["cobc: extern \"./absent.c\": no such file", "at "]),
    ("extern \"./main.cb\";", "", 2, &["not a .c, .o, .a or .so file"]),
    ("extern \"-lfoo\";", "", 2, &["neither a library name"]),
    ("extern \"lib/x.a\";", "", 2, &["neither a library name"]),
    ("extern \"surely_no_such_library_xyz\";", "", 2, &["the C compiler found no library `libsurely_no_such_library_xyz`"]),
    ("extern \"./broken.c\";", "", 2, &["failed on"]),
    ("extern \"./ok.c\";\nextern fn missing(i64 x) : i64;", "    unsafe { missing(1); }", 3, &["unsupported: no C function `missing` in libc, libm or the program's C code"]),
];

#[test]
fn extern_code_errors_are_reported_at_the_declaration() {
    let results = oracle::per_compiler(|cc| {
        let mut got = Vec::new();
        for (i, (decl, call, _, _)) in ERRORS.iter().enumerate() {
            let dir = scratch(cc, &format!("err{}", i));
            fs::write(dir.join("broken.c"), "int f(void) { return 1 }\n").unwrap();
            fs::write(dir.join("ok.c"), "int ok(void) { return 1; }\n").unwrap();
            let main = dir.join("main.cb");
            fs::write(&main, format!("import std;\n\n{}\n\nfn main()\n{{\n{}\n}}\n", decl, call)).unwrap();
            let out = oracle::cobc(cc).arg("--run").arg(&main).output().expect("run cobc");
            got.push((out.status.code(), String::from_utf8_lossy(&out.stderr).into_owned()));
            let _ = fs::remove_dir_all(&dir);
        }
        got
    });
    for (cc, got) in results {
        for ((decl, _, code, frags), (got_code, stderr)) in ERRORS.iter().zip(got) {
            assert_eq!(got_code, Some(*code), "[{}] {}: {}", cc, decl, stderr);
            for f in *frags {
                assert!(stderr.contains(f), "[{}] {}: expected {:?} in:\n{}", cc, decl, f, stderr);
            }
            if *code == 2 && !decl.contains("broken") {
                assert!(stderr.contains("main.cb:3"), "[{}] {}: not located at its declaration:\n{}", cc, decl, stderr);
            }
        }
    }
}
