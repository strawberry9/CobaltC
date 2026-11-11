// D-0255: the targets `cobc --target` compiles for, and what each one
// decides: the runtime it links and where it is, the system libraries, the
// executable's name, the default C compiler and the linker's flags. Both
// are x86-64 with 64-bit addresses and little-endian byte order, so no
// layout rule depends on which.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    LinuxX64,
    WindowsX64,
}

pub const TRIPLES: &[(&str, Target)] = &[("x86_64-unknown-linux-gnu", Target::LinuxX64), ("x86_64-pc-windows-gnu", Target::WindowsX64)];

impl Target {
    pub fn parse(s: &str) -> Option<Target> {
        TRIPLES.iter().find(|(t, _)| *t == s).map(|(_, t)| *t)
    }

    // The target `cobc` itself was built for: its own platform's programs by
    // default. `None` where `cobc` cannot compile natively (an MSVC build:
    // its runtime is not one MinGW's GCC links).
    pub fn host() -> Option<Target> {
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Target::LinuxX64)
        } else if cfg!(all(windows, target_env = "gnu", target_arch = "x86_64")) {
            Some(Target::WindowsX64)
        } else {
            None
        }
    }

    pub fn triple(self) -> &'static str {
        TRIPLES.iter().find(|(_, t)| *t == self).map(|(s, _)| *s).unwrap()
    }

    pub fn windows(self) -> bool {
        self == Target::WindowsX64
    }

    pub fn lib_file(self) -> &'static str {
        crate::cbrt_link::lib_file_for(self.triple()).expect("a supported target")
    }

    pub fn link_libs(self) -> &'static [&'static str] {
        crate::cbrt_link::link_libs_for(self.triple()).expect("a supported target")
    }

    // `prog.cb` -> `prog` (`prog.exe` for Windows).
    pub fn exe_name(self, stem: &Path) -> PathBuf {
        if self.windows() {
            stem.with_extension("exe")
        } else {
            stem.to_path_buf()
        }
    }

    // The C compiler when `--cc` names none: the platform's own natively,
    // MinGW's cross compiler for Windows from Linux.
    pub fn default_cc(self, host: Option<Target>) -> &'static str {
        match (self, host) {
            (Target::WindowsX64, Some(Target::WindowsX64)) => "gcc",
            (Target::WindowsX64, _) => "x86_64-w64-mingw32-gcc",
            (Target::LinuxX64, _) => "cc",
        }
    }

    // Where the runtime for this target is: beside `cobc` for its own
    // target (`target/release`), else in Cargo's directory for the triple
    // (`target/<triple>/release`, from `cargo build -p cbrt --target …`), or
    // `<triple>/` beside `cobc` in an unpacked release.
    pub fn runtime_dir(self, cobc_dir: &Path) -> PathBuf {
        if Some(self) == Target::host() {
            return cobc_dir.to_path_buf();
        }
        let packaged = cobc_dir.join(self.triple());
        if packaged.join(self.lib_file()).exists() {
            return packaged;
        }
        cobc_dir.parent().map(|t| t.join(self.triple()).join("release")).unwrap_or(packaged)
    }

    // The command that builds the runtime this target needs, for the
    // messages when it is missing or stale.
    pub fn runtime_build(self) -> String {
        match (self, Target::host()) {
            (t, h) if Some(t) == h => "`cargo build --release -p cbrt -p cobc` (without --release for a debug cobc)".to_string(),
            (Target::WindowsX64, _) => "`cargo +1.77.2 build --release -p cbrt --target x86_64-pc-windows-gnu` (Rust 1.77.2, whose runtime still runs on Windows 7)".to_string(),
            (t, _) => format!("`cargo build --release -p cbrt --target {}`", t.triple()),
        }
    }
}
