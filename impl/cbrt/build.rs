// Stamps the runtime with a hash of every source it is built from, so
// `cobc` can tell a `libcbrt.a` built from these sources from a stale
// one left in the target directory (`cobc`'s `check_runtime`).
use std::hash::{Hash, Hasher};

const SOURCES: &[&str] = &[
    "src/lib.rs",
    "include/cbrt.h",
    "../src/diagnostics.rs",
    "../src/numtext.rs",
    "../src/fmt.rs",
    "../src/fileio.rs",
    "../../spec/registry/diagnostics.md",
];

fn main() {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for s in SOURCES {
        println!("cargo:rerun-if-changed={}", s);
        std::fs::read(s).unwrap_or_default().hash(&mut h);
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-env=CBRT_STAMP={:016x}", h.finish());
}
