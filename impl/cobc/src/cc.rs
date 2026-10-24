// The C compiler: the options every compile takes, one C file compiled to
// an object (`--lib`), and the reading of the generated C that `localize`
// does (`lower/program.rs`). D-0277: the program's C is one file, compiled
// every time (in parts when large); there is no object cache.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

pub const CC_FLAGS: &[&str] = &["-std=gnu11", "-ffp-contract=off", "-fno-optimize-sibling-calls", "-w"];

// `CC_FLAGS`, and `-fno-optimize-crc` where `cc` accepts it. GCC 15 added a
// pass (on from -O2) that replaces a bit-at-a-time CRC loop with a table or
// carry-less multiply; GCC 16.1 gets the loop wrong in the form `cobc`
// writes it (`std::crypto::crc32` gave 0, or never returned). Asked once:
// GCC before 15 and Clang refuse the flag.
pub fn cc_flags(cc: &str) -> Vec<&'static str> {
    static CRC: OnceLock<bool> = OnceLock::new();
    let off = *CRC.get_or_init(|| {
        if cc_identity(cc).contains("clang") {
            return false;
        }
        Command::new(cc)
            .args(["-fno-optimize-crc", "-fsyntax-only", "-x", "c", "-"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_or(false, |s| s.success())
    });
    let mut v = CC_FLAGS.to_vec();
    if off {
        v.push("-fno-optimize-crc");
    }
    v
}

// The C compiler's `--version` text, asked once.
fn cc_version(cc: &str) -> String {
    static TEXT: OnceLock<String> = OnceLock::new();
    TEXT.get_or_init(|| {
        let out = Command::new(cc).arg("--version").output().ok();
        out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
    })
    .clone()
}

// The C compiler's identity: the first line of `cc --version`.
fn cc_identity(cc: &str) -> String {
    format!("{} {}", cc, cc_version(cc).lines().next().unwrap_or(""))
}

// D-0277: how many translation units a program's C is compiled in, all at
// once: one a megabyte of its functions' C, at most eight. The same program
// is always split alike, whatever the machine. (`cbc`'s 92 MB of C at `-O2`
// on four processors: 357 s as one file, 145 s in four with GCC's LTO, 128 s
// with Clang's ThinLTO; a 4.7 MB program's 32.8 s and 12.4 s.)
pub fn part_count(units_len: usize) -> usize {
    (units_len / 1_000_000).clamp(1, 8)
}

// D-0277: link-time optimisation, which lets the link inline across the
// parts as the C compiler inlines within one file (in four parts with none,
// `cbc` ran 2-4% slower). GCC's (`--version` names the Free Software
// Foundation): `-flto` for each part, `-flto=auto` for the link, and for a
// program in one part `-flto=auto` alone, whose code generation GCC runs in
// parallel too (`cbc` as one file: 170 s). Clang's ThinLTO, `-flto=thin`,
// for a program in parts, through the first linker of the Clang's own `lld`
// and `gold` that links a probe against the runtime (`LTO_PROBE`: clang 14's
// gold plugin cannot read the LLVM bitcode a newer rustc's standard library
// leaves in `libcbrt.a`); ThinLTO of one file is one module, so nothing is
// gained. Without either, the parts are compiled with no LTO. None at `-O0`, or for
// `--lib`, whose archive a C build without it must be able to link.
pub enum Lto {
    Off,
    Gcc,
    Thin(&'static str),
}

impl Lto {
    pub fn part_flags(&self) -> Vec<&'static str> {
        match self {
            Lto::Off => vec![],
            Lto::Gcc => vec!["-flto"],
            Lto::Thin(_) => vec!["-flto=thin"],
        }
    }

    pub fn link_flags(&self) -> Vec<&'static str> {
        match self {
            Lto::Off => vec![],
            Lto::Gcc => vec!["-flto=auto"],
            Lto::Thin(ld) => vec!["-flto=thin", ld],
        }
    }
}

pub fn lto(cc: &str, opt: &str, parts: usize, probe: &dyn Fn(&[&str]) -> bool) -> Lto {
    if opt == "-O0" {
        return Lto::Off;
    }
    let clang = cc_identity(cc).contains("clang");
    if !clang && cc_version(cc).contains("Free Software Foundation") {
        return Lto::Gcc;
    }
    if clang && parts > 1 {
        // `lld` only when it is the Clang installation's own (one of
        // another LLVM, found on the PATH, may not read Clang's bitcode).
        let ld: &[&'static str] = if own_lld(cc) { &["-fuse-ld=lld", "-fuse-ld=gold"] } else { &["-fuse-ld=gold"] };
        for ld in ld {
            if probe(&["-flto=thin", ld]) {
                return Lto::Thin(ld);
            }
        }
    }
    Lto::Off
}

// Whether `cc` (a Clang) has an `ld.lld` of its own LLVM: the one
// `-print-prog-name` names, of the same major version.
fn own_lld(cc: &str) -> bool {
    let path = Command::new(cc).arg("-print-prog-name=ld.lld").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if !Path::new(&path).is_absolute() {
        return false;
    }
    let lld = Command::new(&path).arg("--version").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let major = |text: &str, after: &str| -> Option<String> {
        let i = text.find(after)? + after.len();
        Some(text[i..].trim_start().split(|c: char| !c.is_ascii_digit()).next()?.to_string())
    };
    match (major(&cc_version(cc), "clang version"), major(&lld, "LLD")) {
        (Some(a), Some(b)) => !a.is_empty() && a == b,
        _ => false,
    }
}

// The probe `lto` links (`main.rs`): a program calling into the runtime,
// with the definitions a generated program gives it (`Generated::tail`).
pub const LTO_PROBE: &str = "#include <stdint.h>\n__thread uint32_t cb_at_file, cb_at_line;\nvoid cb_where(uint32_t *file, uint32_t *line) { *file = cb_at_file; *line = cb_at_line; }\nvoid cb_set_where(uint32_t file, uint32_t line) { cb_at_file = file; cb_at_line = line; }\n__thread uintptr_t cb_stack_floor;\nvoid cb_set_stack_floor(uintptr_t floor) { cb_stack_floor = floor; }\nvoid cb_frame_push(void);\nint main(void) { cb_frame_push(); return 0; }\n";

// Every C identifier in `text`.
pub fn identifiers(text: &str) -> HashSet<&str> {
    let mut out = HashSet::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'_' || c.is_ascii_alphabetic() {
            let start = i;
            while i < b.len() && (b[i] == b'_' || b[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            out.insert(&text[start..i]);
        } else {
            i += 1;
        }
    }
    out
}

// A function definition's first line: at the start of a line, not a
// declaration, and followed by the line `{`.
pub fn is_definition(line: &str, next: Option<&str>) -> bool {
    next == Some("{") && !line.is_empty() && !line.starts_with([' ', '{', '}', '#']) && line.ends_with(')') && !line.starts_with("static ")
}

pub fn name_before_paren(line: &str) -> Option<&str> {
    let open = line.find('(')?;
    let head = &line[..open];
    let end = head.trim_end().len();
    let start = head[..end].rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
    let name = &head[start..end];
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

// One C file to an object (`--lib`'s archive is made of objects).
pub fn compile_one(cc: &str, opt: &str, arch: Option<&str>, debug: bool, c_path: &Path, obj: &Path) -> Result<(), String> {
    let mut cmd = Command::new(cc);
    cmd.args(cc_flags(cc)).arg(opt).args(arch).args(debug.then_some("-g")).arg("-c").arg("-o").arg(obj).arg(c_path);
    crate::show(&cmd);
    let out = cmd.output().map_err(|e| format!("cannot run the C compiler `{}`: {}", cc, e))?;
    if !out.status.success() {
        return Err(format!("{}{} failed on {} (kept for inspection)", String::from_utf8_lossy(&out.stderr), cc, c_path.display()));
    }
    Ok(())
}
