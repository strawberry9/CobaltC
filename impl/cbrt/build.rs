// Stamps the runtime with a hash of every source it is built from, so
// `cobc` can tell a `libcbrt.a` built from these sources from a stale
// one left in the target directory (`cobc`'s `check_runtime`).
include!("stamp.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=stamp.rs");
    println!("cargo:rustc-env=CBRT_STAMP={}", cbrt_stamp("."));
    // `#[cfg(kani)]`: the Kani harnesses at the end of `src/lib.rs` (D-0261).
    println!("cargo:rustc-check-cfg=cfg(kani)");
}
