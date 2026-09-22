// The `coby` command line itself.

use std::process::Command;

#[test]
fn about_explains_coby_and_cobc() {
    let out = Command::new(env!("CARGO_BIN_EXE_coby")).arg("--about").output().expect("run coby");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("reference interpreter") && text.contains("cobc"), "{}", text);
    assert!(!text.contains('\r'), "the art kept a carriage return");
}

#[test]
fn usage_names_about() {
    let out = Command::new(env!("CARGO_BIN_EXE_coby")).output().expect("run coby");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("coby --about"));
}
