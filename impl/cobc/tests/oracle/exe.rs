// The compiler the tests run: the one `COBC` names (another build of the
// driver, such as impl/selfhost's cbc: `COBC=/path/to/cbc cargo test -p cobc
// --test driver`), else the cobc these tests were built with. Everything
// else a test needs beside it (`coby`, the runtime) still comes from the
// cobc build.

#![allow(dead_code)]

pub fn cobc_exe() -> std::path::PathBuf {
    match std::env::var_os("COBC") {
        Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
        _ => std::path::PathBuf::from(env!("CARGO_BIN_EXE_cobc")),
    }
}
