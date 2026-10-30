// What links a program against this runtime, for each target `cobc`
// compiles for (D-0255), whatever `cobc` itself runs on. Part of the
// runtime's sources (`cbrt/stamp.rs`), and compiled into `cobc` as well
// (`#[path]`), which does not link the runtime crate.
//
// For `x86_64-pc-windows-gnu` the runtime is built with Rust 1.77.2, whose
// standard library still runs on Windows 7 (and under Wine 6); a later one
// needs `bcryptprimitives.dll`, which neither has.

/// The runtime library's file name for `triple`.
pub fn lib_file_for(triple: &str) -> Option<&'static str> {
    match triple {
        "x86_64-unknown-linux-gnu" | "x86_64-pc-windows-gnu" => Some("libcbrt.a"),
        _ => None,
    }
}

/// The system libraries the runtime needs on `triple`, as `-l` names for
/// the C compiler after `-lcbrt`: the target's `--print native-static-libs`,
/// less what the C compiler links on its own (libc, libgcc; msvcrt).
pub fn link_libs_for(triple: &str) -> Option<&'static [&'static str]> {
    match triple {
        "x86_64-unknown-linux-gnu" => Some(&["pthread", "dl", "m"]),
        "x86_64-pc-windows-gnu" => Some(&["kernel32", "advapi32", "bcrypt", "ntdll", "userenv", "ws2_32"]),
        _ => None,
    }
}
