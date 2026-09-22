// The program's own C code: every `extern "…";` (spec/20
// `rule.trust.extern-code`, D-0030), resolved and handed to the C
// compiler after the generated program.
//
// A string beginning `./`, `../` or `/` is a file, relative to the file
// that declares it (as `module m "./m.cb";` is): a `.c` file, compiled
// here on its own with warnings shown, or a `.o`, `.a` or `.so` file,
// linked as it is. A shared library is linked by its absolute path with
// its directory as an rpath, so the executable finds it from any working
// directory. Any other string is a library name, linked as `cc -l` links
// it. Each file (by its canonical path) and each name is linked once.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

pub enum Code {
    File(PathBuf, Kind),
    Name(String),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    C,
    Object,
    Static,
    Shared,
}

// Every declaration, resolved and checked, or the first that is not, as a
// message naming it and where it is.
pub fn resolve(decls: &[(String, usize)], map: &coby::loader::SourceMap) -> Result<Vec<Code>, String> {
    let mut out: Vec<Code> = Vec::new();
    for (code, line) in decls {
        let at = coby::locate(*line, map);
        let fail = |why: &str| {
            let place = at.as_ref().map(|(f, l)| format!("\n  at {}:{}", f, l)).unwrap_or_default();
            format!("extern \"{}\": {}{}", code, why, place)
        };
        let resolved = if code.starts_with("./") || code.starts_with("../") || code.starts_with('/') {
            let dir = at.as_ref().and_then(|(f, _)| Path::new(f).parent().map(Path::to_path_buf)).unwrap_or_default();
            let joined = if Path::new(code).is_absolute() { PathBuf::from(code) } else { dir.join(code) };
            let path = std::fs::canonicalize(&joined).ok().filter(|p| p.is_file()).ok_or_else(|| fail("no such file"))?;
            let kind = kind_of(&path).ok_or_else(|| fail("not a .c, .o, .a or .so file"))?;
            Code::File(path, kind)
        } else if is_library_name(code) {
            Code::Name(code.clone())
        } else {
            return Err(fail("neither a library name (as `cc -l` takes it) nor a path beginning ./, ../ or /"));
        };
        let dup = out.iter().any(|c| match (c, &resolved) {
            (Code::File(a, _), Code::File(b, _)) => a == b,
            (Code::Name(a), Code::Name(b)) => a == b,
            _ => false,
        });
        if !dup {
            out.push(resolved);
        }
    }
    Ok(out)
}

fn kind_of(path: &Path) -> Option<Kind> {
    let name = path.file_name()?.to_str()?;
    match path.extension()?.to_str()? {
        "c" => Some(Kind::C),
        "o" => Some(Kind::Object),
        "a" => Some(Kind::Static),
        "so" => Some(Kind::Shared),
        // A versioned shared library: `libz.so.1`.
        _ if name.contains(".so.") => Some(Kind::Shared),
        _ => None,
    }
}

// What `cc -lNAME` accepts as one argument, and nothing that could read
// as another option.
fn is_library_name(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('-') && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+'))
}

// The C compiler arguments that link `code`, after compiling each `.c`
// file into `obj_dir`. A `.c` file's warnings go to standard error; one
// that does not compile is an error naming it.
pub fn link_args(code: &[Code], cc: &str, opt: &str, arch: Option<&str>, obj_dir: &Path) -> Result<Vec<OsString>, String> {
    let mut args: Vec<OsString> = Vec::new();
    for (i, c) in code.iter().enumerate() {
        match c {
            Code::File(path, Kind::C) => {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("code");
                let obj = obj_dir.join(format!("{}-{}.o", i, stem));
                let out = Command::new(cc)
                    .arg(opt)
                    .args(arch)
                    .arg("-c")
                    .arg(path)
                    .arg("-o")
                    .arg(&obj)
                    .output()
                    .map_err(|e| format!("cannot run the C compiler `{}`: {}", cc, e))?;
                eprint!("{}", String::from_utf8_lossy(&out.stderr));
                if !out.status.success() {
                    return Err(format!("{} failed on {}", cc, path.display()));
                }
                args.push(obj.into());
            }
            Code::File(path, Kind::Shared) => {
                args.push(path.clone().into());
                let dir = path.parent().unwrap_or(Path::new("/"));
                let mut rpath = OsString::from("-Wl,-rpath,");
                rpath.push(dir);
                args.push(rpath);
            }
            Code::File(path, _) => args.push(path.clone().into()),
            Code::Name(name) => args.push(format!("-l{}", name).into()),
        }
    }
    Ok(args)
}
