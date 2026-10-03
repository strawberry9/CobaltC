// The `cobc` driver itself: what it does with its output path.

use std::fs;
use std::process::Command;

// `cobc foo.cb` writes `foo`; when `foo` is a directory (one holding the
// program's module files, say), it must say so and write nothing, not
// leave the linker to fail with "cannot open output file".
#[test]
fn default_output_that_is_a_directory_is_reported() {
    let dir = std::env::temp_dir().join(format!("cobc-driver-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("prog")).unwrap();
    fs::write(dir.join("prog.cb"), "import std;\n\nfn main()\n{\n    printf(\"hi\\n\");\n}\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).arg("prog.cb").current_dir(&dir).output().expect("run cobc");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", err);
    assert!(err.contains("that is a directory") && err.contains("-o"), "stderr: {}", err);
    assert!(!dir.join("prog.c").exists(), "the C file was written anyway");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn about_shows_the_banner_and_explains_cobc() {
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).arg("--about").output().expect("run cobc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("                            C O B A L T C\n"), "{}", text);
    assert!(text.contains("the CobaltC compiler") && text.contains("coby"), "{}", text);
}
