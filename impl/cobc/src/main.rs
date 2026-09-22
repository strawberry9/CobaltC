// cobc -- the CobaltC compiler driver (impl/COBC-PLAN.md §3.7).
//
//   cobc [--run] [--keep-c] [-O LEVEL] [-march=CPU] [--cc COMPILER] [-o OUT] FILE.cb
//   cobc --help
//
// Front end: `coby`'s (loader, parser, resolver, static pass) — a
// statically rejected program prints exactly what `coby` prints and
// exits 1. Back end: `lower` emits C, `cc` compiles it against the
// `cbrt` staticlib found beside this executable. `--run` compiles to a
// temporary directory and runs the result, forwarding its exit status
// and streams, so it is a drop-in for `coby` under every test runner.
// A construct this stage does not compile yet exits 3 with
// `unsupported: …`, which the runners count as skipped, not failed.

mod link;
mod lower;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

// The header of `--help`: the project's ASCII title.
const BANNER: &str = concat!(
    "                            C O B A L T C\n",
    "                        Programming Language\n",
);

// What `cobc` is, next to `coby`, for `--about`, under the banner.
const ABOUT: &str = concat!(
    "\n",
    "cobc -- the CobaltC compiler\n",
    "\n",
    "cobc turns a CobaltC program into a native executable. Its front end is\n",
    "coby's -- the parser, name resolution and static checks -- so it\n",
    "accepts and rejects exactly the programs coby does, with the same\n",
    "diagnostics. It then translates the program to C and compiles that with\n",
    "the system's C compiler, cc, or another named by --cc (such as clang),\n",
    "linked against cbrt, its run-time library.\n",
    "\n",
    "A compiled program keeps every run-time check the specification\n",
    "requires, except those the compiler proves can never fail. The\n",
    "tracking of objects, references and access paths that coby does for\n",
    "every operation is kept only where the compiler cannot prove it\n",
    "unnecessary, and the rest is plain C, so compiled programs run many\n",
    "times faster.\n",
    "\n",
    "coby, the reference interpreter, runs a program straight from its\n",
    "source on an abstract machine that follows the specification as\n",
    "written. The test suites require cobc's executables to match coby on\n",
    "every program's output and diagnostics, under both GCC and Clang.\n",
    "\n",
    "Use cobc to build an executable, or to see how fast a program can be;\n",
    "use coby to run a program at once, with no build step.\n",
    "\n",
    "  cobc FILE.cb          build FILE (see -o)\n",
    "  cobc --run FILE.cb    build in a temporary directory and run\n",
    "  cobc --help           every option\n",
    "  coby --about          the interpreter\n",
);

const USAGE: &str = "usage: cobc [--run] [--keep-c] [-O LEVEL] [-march=CPU] [--cc COMPILER] [-o OUT] FILE.cb [ARG...]";

const HELP: &str = "\
Compile a CobaltC program to a native executable.

Checks FILE.cb with the same front end as `coby`, lowers it to C, and
compiles that C with `cc` (or the compiler --cc names) against the
`libcbrt.a` runtime found beside this executable.

Options:
  -o OUT       Write the executable to OUT. Default: FILE.cb without its
               extension (`foo.cb` -> `foo`). A program whose modules sit
               in a directory of that name (`foo/`) needs -o.
  --run        Compile to a temporary directory, run the program, forward
               its exit status and output, then delete the directory. With
               `-o`, the executable is written to OUT instead and kept.
               Every ARG after FILE.cb is passed to the program (its
               argument 0 is the first), so options go before FILE.cb.
  --keep-c     Keep the generated C file instead of deleting it after
               `cc`. It sits beside the executable with the extension
               replaced by `.c` (`-o prog` -> `prog.c`, `a.out` -> `a.c`).
               Under `--run` without `-o` it lives in the temporary
               directory and is deleted with it. When `cc` fails the C is
               always kept and its path printed.
  -O LEVEL     Optimization level passed to `cc`: 0, 1, 2, 3, s, or g.
               Also accepted joined: -O0, -O2, -Os, ... Default: 2.
               Given more than once, the last one wins.
  -march=CPU   Target CPU passed to `cc`, e.g. `native`, `x86-64-v3`,
               `haswell`. Default: `cc`'s own (baseline x86-64), which runs
               on any x86-64 machine; `native` may not run on an older CPU
               than the one that compiled it. Given more than once, the
               last one wins.
  --cc COMPILER
               The C compiler to use instead of `cc`: a name found on
               PATH, such as `gcc` or `clang`, or a path. Also accepted
               as --cc=COMPILER. It is given the same options as `cc`,
               which GCC and Clang both accept. Which is faster depends
               on the program (bench/RESULTS.md). Given more than once,
               the last one wins.
  --about      What cobc is, next to the interpreter coby, and exit.
  -h, --help   Print this help and exit.

The program's own C code: `extern \"./shapes.c\";` (or a `.o`, `.a` or
`.so` file) links a file, its path relative to the file that declares
it; `extern \"z\";` links an installed library, as `cc -lz`. A `.c` file
is compiled on its own with the C compiler's warnings shown; a `.so` is
found from any working directory. Each is linked once, and a missing
one is reported at its declaration.

Floating point: the C compiler is always given `-ffp-contract=off`, so every float
operation is rounded on its own as the specification requires (spec/06),
even where -march enables fused multiply-add instructions.

Exit status:
  0    compiled (with --run: the program's own exit status, which is
       main's value for `fn main() : u8`)
  1    the program was rejected statically; the diagnostic is on stderr
  2    usage error, unreadable file, C compiler failure, or C code
       named by `extern \"…\";` that is missing or does not compile
  3    unsupported: a construct the compiler cannot compile yet, or an
       `extern fn` naming a function not in libc, libm or the
       program's C code
";

fn usage() -> ExitCode {
    eprintln!("{}", USAGE);
    eprintln!("Try `cobc --help` for more information.");
    ExitCode::from(2)
}

fn opt_level(level: &str) -> Option<String> {
    matches!(level, "0" | "1" | "2" | "3" | "s" | "g").then(|| format!("-O{}", level))
}

fn march(cpu: &str) -> Option<String> {
    let ok = !cpu.is_empty() && cpu.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    ok.then(|| format!("-march={}", cpu))
}

fn main() -> ExitCode {
    let mut run = false;
    let mut keep_c = false;
    let mut opt = String::from("-O2");
    let mut arch: Option<String> = None;
    let mut cc = String::from("cc");
    let mut out: Option<PathBuf> = None;
    let mut file: Option<PathBuf> = None;
    let mut prog_args: Vec<std::ffi::OsString> = Vec::new();
    // Bytes, not text: a program's argument need not be UTF-8.
    let mut args = std::env::args_os().skip(1);
    while let Some(os) = args.next() {
        // With --run, everything after FILE.cb is the program's.
        if run && file.is_some() {
            prog_args.push(os);
            continue;
        }
        let Some(a) = os.to_str().map(str::to_string) else {
            if file.is_some() {
                return usage();
            }
            file = Some(PathBuf::from(os));
            continue;
        };
        let next = |args: &mut std::iter::Skip<std::env::ArgsOs>| args.next().and_then(|o| o.into_string().ok());
        match a.as_str() {
            "--about" => {
                print!("{}{}", BANNER, ABOUT);
                return ExitCode::SUCCESS;
            }
            "-h" | "--help" => {
                print!("{}\n{}\n\n{}", BANNER, USAGE, HELP);
                return ExitCode::SUCCESS;
            }
            "--run" => run = true,
            "--keep-c" => keep_c = true,
            "-o" => match next(&mut args) {
                Some(o) => out = Some(PathBuf::from(o)),
                None => return usage(),
            },
            "-O" => match next(&mut args).as_deref().and_then(opt_level) {
                Some(o) => opt = o,
                None => return usage(),
            },
            _ if a.starts_with("-O") => match opt_level(&a[2..]) {
                Some(o) => opt = o,
                None => return usage(),
            },
            _ if a.starts_with("-march=") => match march(&a["-march=".len()..]) {
                Some(m) => arch = Some(m),
                None => return usage(),
            },
            "--cc" => match next(&mut args) {
                Some(c) if !c.is_empty() => cc = c,
                _ => return usage(),
            },
            _ if a.starts_with("--cc=") && a.len() > "--cc=".len() => cc = a["--cc=".len()..].to_string(),
            _ if a.starts_with('-') => return usage(),
            _ if file.is_some() => return usage(),
            _ => file = Some(PathBuf::from(a)),
        }
    }
    let Some(file) = file else { return usage() };
    let src = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {}", file.display(), e);
            return ExitCode::from(2);
        }
    };

    let analysis = coby::analyze(&src, Some(&file));
    if let Some(d) = analysis.error {
        eprint!("{}", coby::render_located(&d, "static", &analysis.map));
        return ExitCode::FAILURE;
    }
    let items = analysis.items.expect("checked");
    // The program's own C code (`extern \"…\";`), checked before any C is
    // written: a missing file is reported at its declaration.
    let code = match link::resolve(&items.extern_code, &analysis.map) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cobc: {}", e);
            return ExitCode::from(2);
        }
    };

    let c_text = match lower::Gen::new(&items, &analysis.map).program() {
        Ok(t) => t,
        Err(lower::Unsupported(what)) => {
            eprintln!("unsupported: {}", what);
            return ExitCode::from(3);
        }
    };

    let scratch = if run {
        let d = std::env::temp_dir().join(format!("cobc-{}-{}", std::process::id(), unique()));
        std::fs::create_dir_all(&d).expect("temp dir");
        Some(d)
    } else {
        None
    };
    let exe = match (&out, &scratch) {
        (Some(o), _) => o.clone(),
        (None, Some(d)) => d.join("a.out"),
        (None, None) => file.with_extension(""),
    };
    // The default name (`tinyos.cb` -> `tinyos`) can be a directory beside
    // the source, such as one holding its module files; say so before
    // writing anything, rather than leave it to the linker.
    if exe.is_dir() {
        eprintln!(
            "cobc: cannot write the executable to {}: that is a directory{}",
            exe.display(),
            if out.is_none() { format!("; choose another name with -o, e.g. `cobc -o {}.out {}`", exe.display(), file.display()) } else { String::new() }
        );
        return ExitCode::from(2);
    }
    let c_path = exe.with_extension("c");
    std::fs::write(&c_path, c_text).expect("write C");

    let obj_dir = std::env::temp_dir().join(format!("cobc-obj-{}-{}", std::process::id(), unique()));
    let linked = std::fs::create_dir_all(&obj_dir)
        .map_err(|e| format!("cannot create {}: {}", obj_dir.display(), e))
        .and_then(|_| link::link_args(&code, &cc, &opt, arch.as_deref(), &obj_dir))
        .and_then(|extra| compile(&cc, &c_path, &exe, &opt, arch.as_deref(), &extra));
    let _ = std::fs::remove_dir_all(&obj_dir);
    let output = match linked {
        Ok(o) => o,
        Err(e) => {
            eprintln!("cobc: {}", e);
            return ExitCode::from(2);
        }
    };
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        // An `extern fn` naming a function the C libraries do not have is
        // reported as the interpreter reports it: unsupported, exit 3.
        if let Some(rest) = err.split("undefined reference to `").nth(1) {
            let name = rest.split('\'').next().unwrap_or(rest);
            if !name.starts_with("cb_") {
                let also = if code.is_empty() { "" } else { " or the program's C code" };
                eprintln!("unsupported: no C function `{}` in libc, libm{}", name, also);
                let _ = std::fs::remove_file(&c_path);
                return ExitCode::from(3);
            }
        }
        // A library named by `extern \"…\";` that the linker cannot find
        // (GNU ld: `cannot find -lNAME`; lld: `unable to find library
        // -lNAME`) is reported at its declaration, like a missing file.
        let missing = err.split("find -l").nth(1).or_else(|| err.split("find library -l").nth(1));
        if let Some(rest) = missing {
            let name: String = rest.chars().take_while(|c| !c.is_whitespace() && *c != ':').collect();
            if let Some((_, line)) = items.extern_code.iter().find(|(c, _)| *c == name) {
                let at = coby::locate(*line, &analysis.map).map(|(f, l)| format!("\n  at {}:{}", f, l)).unwrap_or_default();
                eprintln!("cobc: extern \"{}\": the C compiler found no library `lib{}`{}", name, name, at);
                let _ = std::fs::remove_file(&c_path);
                return ExitCode::from(2);
            }
        }
        eprint!("{}", err);
        eprintln!("cobc: {} failed on {} (kept for inspection)", cc, c_path.display());
        return ExitCode::from(2);
    }
    if !keep_c {
        let _ = std::fs::remove_file(&c_path);
    }

    let code = if run {
        let st = Command::new(&exe).args(&prog_args).status().expect("run compiled program");
        if let Some(d) = &scratch {
            let _ = std::fs::remove_dir_all(d);
        }
        st.code().unwrap_or(1)
    } else {
        0
    };
    ExitCode::from(code as u8)
}

fn unique() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
}

// `extra` links the program's own C code (`link::link_args`): after the
// generated program, before the runtime and the system libraries.
fn compile(cc: &str, c_path: &Path, exe: &Path, opt: &str, arch: Option<&str>, extra: &[std::ffi::OsString]) -> Result<std::process::Output, String> {
    let here = std::env::current_exe().map_err(|e| e.to_string())?;
    let lib_dir = here.parent().ok_or("no parent dir")?.to_path_buf();
    if !lib_dir.join("libcbrt.a").exists() {
        return Err(format!("libcbrt.a not found in {} (build the workspace: `cargo build --workspace`, or `cargo build -p cbrt -p cobc`)", lib_dir.display()));
    }
    // `-ffp-contract=off`: gnu11 lets the compiler fuse `a*b + c` into one FMA
    // with a single rounding wherever the target has FMA (any -march
    // since Haswell); spec/06 rounds each operation on its own.
    Command::new(cc)
        .args(["-std=gnu11", opt, "-ffp-contract=off", "-w"])
        .args(arch)
        .arg("-o")
        .arg(exe)
        .arg(c_path)
        .args(extra)
        .arg("-L")
        .arg(&lib_dir)
        .args(["-lcbrt", "-lpthread", "-ldl", "-lm"])
        .output()
        .map_err(|e| format!("cannot run the C compiler `{}`: {}", cc, e))
}
