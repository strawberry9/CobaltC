// `cobc` knows the stamp of the runtime it was built with (`cbrt/stamp.rs`),
// to find a stale `libcbrt.a` (`check_runtime`).
include!("../cbrt/stamp.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../cbrt/stamp.rs");
    println!("cargo:rustc-env=CBRT_STAMP={}", cbrt_stamp("../cbrt"));
}
