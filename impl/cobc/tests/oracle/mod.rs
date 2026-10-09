// The interpreter as the oracle for compiled programs' output: stdout,
// and stderr (a diagnostic with its location).
//
// `cobc` depends on `coby` as a library only, so `cargo test -p cobc`
// never builds the `coby` executable: a sibling `target/<profile>/coby`
// would be whatever an earlier build left there, however stale. It is
// built here instead, once per test process, in the profile these tests
// were built in, before its first use.

#![allow(dead_code)]                    // each test binary uses only some of it

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[path = "../../../tests/common/mod.rs"]
mod common;
pub use common::{case_args_os, case_dir};
// The corpus runners' battery knobs (tests/common/mod.rs); the other
// test binaries that include this module do not use them.
#[allow(unused_imports)]
pub use common::{in_shard, shard_count, slow_skipped, Slowest};

// D-0143: whether a case's `platform:` header names another platform.
pub fn platform_skipped(src: &str) -> bool {
    common::platform_skipped(src)
}

// The C compilers every compiled test runs under: GCC and Clang, so that
// C one of them happens to accept but the other reads differently shows
// up as a failure (a bare `-9223372036854775808` once did). Set
// COBC_TEST_CCS to a comma-separated list to choose others. A compiler
// that is not installed fails the tests, rather than testing less.
pub fn c_compilers() -> &'static [String] {
    static CCS: OnceLock<Vec<String>> = OnceLock::new();
    CCS.get_or_init(|| {
        let list = std::env::var("COBC_TEST_CCS").unwrap_or_else(|_| "gcc,clang".to_string());
        let ccs: Vec<String> = list.split(',').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
        for cc in &ccs {
            let found = Command::new(cc).arg("--version").output().map_or(false, |o| o.status.success());
            assert!(
                found,
                "C compiler `{}` not found: install it (Ubuntu: `sudo apt install {}`), or set COBC_TEST_CCS to the compilers to test with",
                cc,
                if cc == "clang" { "clang" } else { "gcc" }
            );
        }
        assert!(!ccs.is_empty(), "COBC_TEST_CCS names no compiler");
        ccs
    })
}

// CPU seconds any one program under test may use: `cobc --run` passes
// it on to the compiled program (`COBALTC_CPU_LIMIT`), and `coby` gets
// it as its own limit (`limited`). A runaway program is stopped, never
// left spinning.
pub const CPU_LIMIT_SECS: u64 = 300;

// `cobc --cc CC`, ready for its remaining arguments.
pub fn cobc(cc: &str) -> Command {
    coby();                                   // and, with it, a runtime built from these sources
    let mut c = Command::new(env!("CARGO_BIN_EXE_cobc"));
    c.arg("--cc").arg(cc).env("COBALTC_CPU_LIMIT", CPU_LIMIT_SECS.to_string());
    limited(&mut c);
    c
}

// `coby` on a case, CPU-limited.
pub fn coby_cmd() -> Command {
    let mut c = Command::new(coby());
    limited(&mut c);
    c
}

// `RLIMIT_CPU` for the process (and what it starts), and death with the
// test process: nothing a test starts outlives it. Linux-only, like
// `cobc`'s own child_guard; elsewhere the tests run without the guard
// (Windows would need a Job Object for the same guarantee).
#[cfg(target_os = "linux")]
pub fn limited(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    #[repr(C)]
    struct RLimit {
        cur: u64,
        max: u64,
    }
    extern "C" {
        fn setrlimit(resource: i32, rlim: *const RLimit) -> i32;
        fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
    }
    unsafe {
        cmd.pre_exec(|| {
            let r = RLimit { cur: CPU_LIMIT_SECS, max: CPU_LIMIT_SECS + 1 };
            if setrlimit(0, &r) != 0 || prctl(1, 9, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(target_os = "linux"))]
pub fn limited(_cmd: &mut Command) {}

// Runs `f` once per C compiler, in parallel, and returns each result
// labelled with its compiler.
pub fn per_compiler<T: Send, F: Fn(&str) -> T + Sync>(f: F) -> Vec<(String, T)> {
    let ccs = c_compilers();
    coby();                                   // built once, before the threads race to
    std::thread::scope(|s| {
        let hs: Vec<_> = ccs.iter().map(|cc| s.spawn(|| (cc.clone(), f(cc.as_str())))).collect();
        hs.into_iter().map(|h| h.join().expect("compiler thread")).collect()
    })
}

pub fn coby() -> &'static Path {
    static COBY: OnceLock<PathBuf> = OnceLock::new();
    COBY.get_or_init(|| {
        let cobc = Path::new(env!("CARGO_BIN_EXE_cobc"));
        let profile = match cobc.parent().and_then(|d| d.file_name()).and_then(|n| n.to_str()) {
            Some("debug") | None => "dev".to_string(),
            Some(p) => p.to_string(),
        };
        let status = Command::new(env!("CARGO"))
            .args(["build", "--quiet", "--bin", "coby", "--profile", &profile, "--manifest-path"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml"))
            .status()
            .expect("run cargo to build coby");
        assert!(status.success(), "building the coby oracle failed");
        // Likewise the runtime `cobc` links: built as a dependency of
        // `cobc`, cbrt's `target/<profile>/libcbrt.a` is not refreshed.
        let status = Command::new(env!("CARGO"))
            .args(["build", "--quiet", "-p", "cbrt", "--lib", "--profile", &profile, "--manifest-path"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml"))
            .status()
            .expect("run cargo to build cbrt");
        assert!(status.success(), "building the runtime failed");
        cobc.with_file_name("coby")
    })
}

pub struct Oracle {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

// Run with the case's `args:`, as `cobc --run` runs it. Cached per file
// for the process: both compiler threads, and every spec row that cites
// the file, compare against one interpreter run (a slow case under the
// interpreter used to run once per compiler and once per row).
pub fn oracle(path: &Path) -> std::sync::Arc<Oracle> {
    use std::sync::{Arc, Mutex};
    static CACHE: OnceLock<Mutex<std::collections::HashMap<PathBuf, Arc<OnceLock<Arc<Oracle>>>>>> = OnceLock::new();
    let cell = CACHE.get_or_init(Default::default).lock().unwrap().entry(path.to_path_buf()).or_default().clone();
    // A second thread asking for the same file waits here for the first.
    cell.get_or_init(|| Arc::new(run_oracle(path))).clone()
}

fn run_oracle(path: &Path) -> Oracle {
    let src = std::fs::read_to_string(path).unwrap_or_default();
    let mut cmd = coby_cmd();
    cmd.current_dir(case_dir(path)).arg(path).args(case_args_os(&src).expect("cobc runs on Linux"));
    let out = output_with_stdin(&mut cmd, &case_stdin(&src).unwrap_or_default());
    Oracle { stdout: out.stdout, stderr: out.stderr }
}

// `None` when the compiled program's output is `coby`'s; otherwise what
// differs, for the failure message.
pub fn output_differs(path: &Path, stdout: &[u8], stderr: &[u8]) -> Option<String> {
    let o = oracle(path);
    if stdout != o.stdout.as_slice() {
        return Some(format!("stdout differs from coby's: {:?}", String::from_utf8_lossy(stdout)));
    }
    if stderr != o.stderr.as_slice() {
        return Some(format!(
            "stderr differs from coby's:\n--- coby\n{}--- cobc\n{}",
            String::from_utf8_lossy(&o.stderr),
            String::from_utf8_lossy(stderr)
        ));
    }
    None
}

// A conformance case only `cobc` can run (a `cobc-only:` header,
// CHG-0039: it links its own C code) has no `coby` oracle; its header
// states its output instead.
pub fn is_cobc_only(src: &str) -> bool {
    header_field(src, "cobc-only").is_some()
}

// A case's standard input: its `stdin-hex:` bytes (CHG-0038, D-0050),
// fed to `cobc`'s program and to the `coby` oracle alike; `None` when it
// has none (the program then gets no input).
pub fn case_stdin(src: &str) -> Option<Vec<u8>> {
    header_field(src, "stdin-hex").map(|h| unhex(&h))
}

pub fn header_field(src: &str, key: &str) -> Option<String> {
    for line in src.lines() {
        let line = line.trim_start();
        if !line.starts_with("//") {
            break;
        }
        let body = line.trim_start_matches('/').trim();
        if let Some((k, v)) = body.split_once(':') {
            if k.trim() == key {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

fn unhex(h: &str) -> Vec<u8> {
    (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).expect("stdin-hex: not hex")).collect()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// Runs `cmd` with `input` as its standard input.
pub fn output_with_stdin(cmd: &mut Command, input: &[u8]) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn");
    child.stdin.take().unwrap().write_all(input).expect("write stdin");
    child.wait_with_output().expect("wait")
}

// A cobc-only case's output against its own header, in place of
// `coby`'s: `expect-stdout-hex:` if it has one, and no error output.
pub fn output_differs_from_header(src: &str, stdout: &[u8], stderr: &[u8]) -> Option<String> {
    if let Some(want) = header_field(src, "expect-stdout-hex") {
        if hex(stdout) != want {
            return Some(format!("stdout {} differs from expect-stdout-hex {}", hex(stdout), want));
        }
    }
    if !stderr.is_empty() {
        return Some(format!("unexpected stderr: {}", String::from_utf8_lossy(stderr)));
    }
    None
}
