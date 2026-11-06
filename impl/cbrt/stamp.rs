// The runtime's source stamp: a hash of every source `libcbrt.a` is built
// from. `include!`d by `cbrt/build.rs`, which bakes it into the library,
// and by `cobc/build.rs`, which bakes it into `cobc`, so `cobc` can tell a
// stale runtime without linking the runtime crate (D-0255: on Windows that
// pulls the runtime's C-facing code into `cobc.exe`).

const STAMP_SOURCES: &[&str] = &[
    "src/lib.rs",
    "src/link_info.rs",
    "include/cbrt.h",
    "../src/diagnostics.rs",
    "../src/numtext.rs",
    "../src/fmt.rs",
    "../src/fileio.rs",
    "../src/procio.rs",
    "../src/netio.rs",
    "../src/osrand.rs",
    "../src/outbuf.rs",
    "../../spec/registry/diagnostics.md",
];

// The stamp of the sources under `cbrt_dir` (the runtime crate's
// directory), printing the `rerun-if-changed` lines for them.
fn cbrt_stamp(cbrt_dir: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for s in STAMP_SOURCES {
        let p = format!("{}/{}", cbrt_dir, s);
        println!("cargo:rerun-if-changed={}", p);
        std::fs::read(&p).unwrap_or_default().hash(&mut h);
    }
    format!("{:016x}", h.finish())
}
