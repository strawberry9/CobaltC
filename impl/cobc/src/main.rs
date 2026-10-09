// cobc -- the CobaltC compiler driver (impl/COBC-PLAN.md §3.7).
//
//   cobc [--run] [--keep-c] [-v] [-s] [-O LEVEL] [-march=CPU] [--cc COMPILER] [-o OUT] FILE.cb
//   cobc --check PATH...
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

mod cache;
mod link;
mod lower;
mod report;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicBool, Ordering};

// `-v`: each command shown before it runs (`show`).
static VERBOSE: AtomicBool = AtomicBool::new(false);

// A line of `-v`'s, on standard error.
fn note(msg: &str) {
    if VERBOSE.load(Ordering::Relaxed) {
        eprintln!("cobc: {}", msg);
    }
}

// Under `-v`, `cmd` as a shell would read it: `+ ` and each word, quoted
// (`'…'`, a `'` inside as `'\''`) unless it is made of characters no
// shell treats specially.
pub(crate) fn show(cmd: &Command) {
    if !VERBOSE.load(Ordering::Relaxed) {
        return;
    }
    fn quote(w: &std::ffi::OsStr) -> String {
        let w = w.to_string_lossy();
        let plain = !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c));
        if plain {
            w.into_owned()
        } else {
            format!("'{}'", w.replace('\'', "'\\''"))
        }
    }
    let mut line = String::from("+ ");
    line.push_str(&quote(cmd.get_program()));
    for a in cmd.get_args() {
        line.push(' ');
        line.push_str(&quote(a));
    }
    eprintln!("{}", line);
}

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
    "  cobc --check PATH...  check programs statically; build nothing\n",
    "  cobc --help           every option\n",
    "  coby --about          the interpreter\n",
);

const USAGE: &str = "usage: cobc [--run] [--explore N] [--schedule-seed S] [--keep-c] [--report-tracking] [-v] [-s] [-O LEVEL] [-march=CPU] [--cc COMPILER] [-o OUT] FILE.cb [ARG...]\n       cobc --check PATH...";

const HELP: &str = "\
Compile a CobaltC program to a native executable.

Checks FILE.cb with the same front end as `coby`, lowers it to C, and
compiles that C with `cc` (or the compiler --cc names) against the
`cbrt` runtime library found beside this executable.

Options:
  -o OUT       Write the executable to OUT. Default: FILE.cb without its
               extension (`foo.cb` -> `foo`). A program whose modules sit
               in a directory of that name (`foo/`) needs -o.
  --run        Compile to a temporary directory, run the program, forward
               its exit status and output, then delete the directory. With
               `-o`, the executable is written to OUT instead and kept.
               Every ARG after FILE.cb is passed to the program (its
               argument 0 is the first), so options go before FILE.cb.
  --explore N  Compile, then run the program N times, each under another
               order of its threads chosen by a seed (1 to N), and group
               the runs by outcome: exit status, output and errors. One
               thread runs at a time; the turn passes at a `spawn`, a
               wait, a mutex released, a thread's end, a call outside the
               program (a socket, input, sleeping) and now and then
               between two of the program's checks. Each outcome is shown
               with a seed that gives it; the exit status is 1 when the
               runs differ. Implies --run; ARGs and standard input (read
               once, unless a terminal) go to every run.
  --schedule-seed S
               Run once (implies --run) under the order seed S gives, as
               `--explore` ran it: the same seed, the same order, so a
               run that ended differently can be replayed. Setting
               COBALTC_SCHEDULE_SEED=S in a compiled program's
               environment does the same.
  COBC_CACHE   (environment) The object cache: `std` functions that do not
               depend on the program are compiled once and their objects
               kept, by default in `cobc-cache` beside this executable, and
               linked into every later program that uses them, so a program
               that uses much of `std` compiles in a few seconds instead of
               tens. `COBC_CACHE=off` compiles everything every time, as one
               C file; `COBC_CACHE=DIR` keeps the objects in DIR. A rebuilt
               cobc, another C compiler or other options use other objects;
               stale ones are never reused, only left behind (delete the
               directory to reclaim the space).
  --keep-c     Keep the generated C file instead of deleting it after
               `cc`. It sits beside the executable with the extension
               replaced by `.c` (`-o prog` -> `prog.c`, `a.out` -> `a.c`).
               Under `--run` without `-o` it lives in the temporary
               directory and is deleted with it. When `cc` fails the C is
               always kept and its path printed.
  --report-tracking
               Compile nothing: list the lines of the program's own files
               whose generated C still calls the runtime's tracking (paths,
               objects, checked accesses), those inside loops first, with
               the kinds of call on each. Bounds and overflow checks are
               not listed. Chapter P of the guide says what each form
               costs and how to write code the compiler can see through.
  -v, --verbose
               Print each command cobc runs, on standard error, before it
               runs it: the C compiler on the generated C, `cc -c` on each
               C file named by `extern \"…\";`, and with --run the program.
               Each is a line beginning `+ `, quoted so that it can be
               pasted into a shell. Also where the generated C is written,
               and whether it is kept or removed.
  -s, --strip  Leave the symbol table out of the executable too (the
               linker's --strip-all),
               for the smallest file: a hello world is about 0.6 MB
               instead of 0.8 MB. The program runs the same -- its fault
               messages come from the locations compiled into it, not
               from symbols -- but a profiler or debugger can no longer
               name its functions.
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
  --check PATH...
               Check each program statically and build nothing: each FILE.cb
               named, and each `.cb` file in each directory named (hidden
               directories and `target/` skipped). It must be the first
               argument; no other option goes with it. A rejected program
               prints the diagnostic a build would print, and a file named
               by `extern \"./…\";` that is missing is reported, but no C is
               written or compiled. In a directory, a file another program
               there loads as a module (`module m \"./m.cb\";`) is checked
               as part of that program, not on its own. Nothing is printed
               for an accepted program. `coby --check` does the same
               without the `extern` files.
  --about      What cobc is, next to the interpreter coby, and exit.
  -h, --help   Print this help and exit.

The program's own C code: `extern \"./shapes.c\";` (or a `.o`, `.a` or
`.so` file) links a file, its path relative to the file that declares
it; `extern \"z\";` links an installed library, as `cc -lz`. A `.c` file
is compiled on its own with the C compiler's warnings shown; a `.so` is
found from any working directory. Each is linked once, and a missing
one is reported at its declaration.

The executable: on Linux the linker is given `-Wl,--gc-sections` and
`-Wl,--strip-debug`, so it holds only the parts of the runtime the
program uses and no debug information (the generated C is compiled
without `-g`); its symbol table stays, for profilers and debuggers,
unless -s removes it.

Floating point: the C compiler is always given `-ffp-contract=off`, so every float
operation is rounded on its own as the specification requires (spec/06),
even where -march enables fused multiply-add instructions; and
`-fno-optimize-sibling-calls`, so a tail call is still a call and a
recursion too deep for the stack faults (diag.stack-exhausted) whatever
the C compiler and optimization level.

Exit status:
  0    compiled (with --run: the program's own exit status, which is
       main's value for `fn main() : u8`; with --check: every program
       accepted)
  1    the program (with --check: one of them) was rejected statically;
       the diagnostic is on stderr
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

// The checker and the lowering recurse on the program's nesting (a
// 100 000-deep parenthesised expression, a 40 000-term sum): they run on a
// thread whose stack is reserved large, as `coby`'s evaluator does (only
// what is used is committed).
fn main() -> ExitCode {
    let stack = if cfg!(target_pointer_width = "64") { 1usize << 30 } else { 256 << 20 };
    std::thread::Builder::new()
        .stack_size(stack)
        .spawn(real_main)
        .expect("start the compiler thread")
        .join()
        .unwrap_or_else(|e| std::panic::resume_unwind(e))
}

fn real_main() -> ExitCode {
    // `--check`: the front end and the `extern "…";` files only (`coby::check`).
    let all: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if all.first().map_or(false, |a| a == "--check") {
        let paths: Vec<PathBuf> = all[1..].iter().map(PathBuf::from).collect();
        if paths.is_empty() || paths.iter().any(|p| p.to_str().map_or(false, |s| s.starts_with('-'))) {
            return usage();
        }
        let externs = |a: &coby::Analysis| {
            let items = a.items.as_ref().expect("checked");
            link::resolve(&items.extern_code, &a.map).map(|_| ())
        };
        return ExitCode::from(coby::check::run(&paths, "cobc", &externs));
    }
    let mut run = false;
    let mut explore: Option<u64> = None;
    let mut seed: Option<u64> = None;
    let mut keep_c = false;
    let mut report_tracking = false;
    let mut strip = false;
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
            "--explore" => match next(&mut args).and_then(|n| n.parse::<u64>().ok()).filter(|n| *n > 0) {
                Some(n) => {
                    explore = Some(n);
                    run = true;
                }
                None => return usage(),
            },
            "--schedule-seed" => match next(&mut args).and_then(|n| n.parse::<u64>().ok()).filter(|n| *n > 0) {
                Some(n) => {
                    seed = Some(n);
                    run = true;
                }
                None => return usage(),
            },
            "--keep-c" => keep_c = true,
            "--report-tracking" => report_tracking = true,
            "-v" | "--verbose" => VERBOSE.store(true, Ordering::Relaxed),
            "-s" | "--strip" => strip = true,
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
        eprint!("{}", coby::diagnostics::for_stderr(&coby::render_located(&d, "static", &analysis.map)));
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

    let t_lower = std::time::Instant::now();
    let gen = match lower::Gen::new(&items, &analysis.map).generate() {
        Ok(g) => g,
        Err(lower::Unsupported(what)) => {
            eprintln!("unsupported: {}", what);
            return ExitCode::from(3);
        }
    };
    if report_tracking {
        print!("{}", report::report(&gen.combined(), &file));
        return ExitCode::SUCCESS;
    }
    let cache_dir = cache::dir();
    note(&format!("lowered to C in {} ms ({} units, {} cacheable)", t_lower.elapsed().as_millis(), gen.units.len(), gen.units.iter().filter(|u| u.cacheable).count()));

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
    // The single C file: compiled as it is without the cache, kept for
    // inspection with `--keep-c`, written when something fails.
    if cache_dir.is_none() || keep_c {
        std::fs::write(&c_path, gen.combined()).expect("write C");
        note(&format!("wrote the generated C to {}", c_path.display()));
    }

    let obj_dir = std::env::temp_dir().join(format!("cobc-obj-{}-{}", std::process::id(), unique()));
    let linked = std::fs::create_dir_all(&obj_dir)
        .map_err(|e| format!("cannot create {}: {}", obj_dir.display(), e))
        .and_then(|_| link::link_args(&code, &cc, &opt, arch.as_deref(), &obj_dir))
        .and_then(|extra| match &cache_dir {
            Some(dir) => {
                let inp = cache::Inputs { cc: &cc, opt: &opt, arch: arch.as_deref(), cache_dir: dir.clone(), work_dir: &obj_dir };
                let t = std::time::Instant::now();
                let built = cache::build(&gen, &inp)?;
                note(&format!("object cache {}: {} unit(s) reused, {} compiled, in {} ms", dir.display(), built.reused, built.compiled, t.elapsed().as_millis()));
                let t = std::time::Instant::now();
                let r = compile(&cc, &built.objects, &exe, &opt, arch.as_deref(), &extra, strip, &obj_dir);
                note(&format!("linked in {} ms", t.elapsed().as_millis()));
                r
            }
            None => compile(&cc, std::slice::from_ref(&c_path), &exe, &opt, arch.as_deref(), &extra, strip, &obj_dir),
        });
    let cache_failed = linked.is_err() && cache_dir.is_some();
    if cache_failed && !c_path.exists() {
        let _ = std::fs::write(&c_path, gen.combined());
    }
    if !cache_failed {
        let _ = std::fs::remove_dir_all(&obj_dir);
    }
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
        note(&format!("removed {} (--keep-c keeps it)", c_path.display()));
    } else if scratch.is_some() && out.is_none() {
        note(&format!("kept {} until the temporary directory is deleted after the run", c_path.display()));
    } else {
        note(&format!("kept {}", c_path.display()));
    }

    if let Some(n) = explore {
        let code = explore_runs(&exe, &prog_args, n, &file);
        if let Some(d) = &scratch {
            let _ = std::fs::remove_dir_all(d);
        }
        return code;
    }
    let code = if run {
        let mut cmd = Command::new(&exe);
        cmd.args(&prog_args);
        if let Some(s) = seed {
            cmd.env("COBALTC_SCHEDULE_SEED", s.to_string());
        }
        child_guard(&mut cmd);
        show(&cmd);
        let st = cmd.status().expect("run compiled program");
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            match st.signal() {
                Some(24) => eprintln!("cobc: the program used up its CPU time limit (COBALTC_CPU_LIMIT) and was stopped"),
                Some(n) => eprintln!("cobc: the program was stopped by signal {}", n),
                None => {}
            }
        }
        if let Some(d) = &scratch {
            let _ = std::fs::remove_dir_all(d);
        }
        st.code().unwrap_or(1)
    } else {
        0
    };
    ExitCode::from(code as u8)
}

// `--explore N`: the compiled program run under seeds 1..=N
// (`COBALTC_SCHEDULE_SEED`), the runs grouped by outcome, as
// `coby --explore` groups them.
fn explore_runs(exe: &std::path::Path, prog_args: &[std::ffi::OsString], runs: u64, file: &std::path::Path) -> ExitCode {
    use std::io::{Read, Write};
    let mut input = Vec::new();
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        let _ = std::io::stdin().read_to_end(&mut input);
    }
    // (status, stdout, stderr) -> (count, first seed)
    let mut seen: Vec<((Option<i32>, Vec<u8>, Vec<u8>), u64, u64)> = Vec::new();
    for s in 1..=runs {
        let mut cmd = Command::new(exe);
        cmd.args(prog_args).env("COBALTC_SCHEDULE_SEED", s.to_string());
        cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        child_guard(&mut cmd);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("cobc --explore: {}", e);
                return ExitCode::from(2);
            }
        };
        if let Some(mut w) = child.stdin.take() {
            let _ = w.write_all(&input);
        }
        let out = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => {
                eprintln!("cobc --explore: {}", e);
                return ExitCode::from(2);
            }
        };
        let key = (out.status.code(), out.stdout, out.stderr);
        match seen.iter_mut().find(|(k, _, _)| *k == key) {
            Some(e) => e.1 += 1,
            None => seen.push((key, 1, s)),
        }
    }
    let f = file.display();
    println!("explored {} schedules of {}: {} distinct outcome{}", runs, f, seen.len(), if seen.len() == 1 { "" } else { "s" });
    for ((status, so, se), n, s) in &seen {
        let st = status.map_or("killed".to_string(), |c| format!("exit {}", c));
        println!("\n{} run{}, {} -- replay: cobc --schedule-seed {} {}", n, if *n == 1 { "" } else { "s" }, st, s, f);
        for l in String::from_utf8_lossy(so).lines().take(8) {
            println!("  | {}", l);
        }
        for l in String::from_utf8_lossy(se).lines().take(3) {
            println!("  ! {}", l);
        }
    }
    if seen.len() == 1 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

// `--run`'s program never outlives `cobc`: on Linux it is killed when
// `cobc` ends, however that happens (a signal, a killed parent), so a
// runaway test program cannot be left behind as an orphan. With
// `COBALTC_CPU_LIMIT=<seconds>` set, it also gets that much CPU time and
// no more (`RLIMIT_CPU`; the runners set it).
#[cfg(target_os = "linux")]
fn child_guard(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    #[repr(C)]
    struct RLimit {
        cur: u64,
        max: u64,
    }
    extern "C" {
        fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
        fn setrlimit(resource: i32, rlim: *const RLimit) -> i32;
        fn getppid() -> i32;
    }
    const PR_SET_PDEATHSIG: i32 = 1;
    const SIGKILL: u64 = 9;
    const RLIMIT_CPU: i32 = 0;
    let parent = std::process::id() as i32;
    let limit: Option<u64> = std::env::var("COBALTC_CPU_LIMIT").ok().and_then(|v| v.trim().parse().ok());
    unsafe {
        cmd.pre_exec(move || {
            if prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            // `cobc` may have ended before the request took effect.
            if getppid() != parent {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "cobc ended"));
            }
            if let Some(secs) = limit {
                let r = RLimit { cur: secs, max: secs + 1 };
                if setrlimit(RLIMIT_CPU, &r) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
}

#[cfg(not(target_os = "linux"))]
fn child_guard(_cmd: &mut Command) {}

fn unique() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
}

// The runtime beside this executable must be the one built from the
// sources this `cobc` was built with: `cargo test` can rebuild `cobc`
// without refreshing `target/<profile>/libcbrt.a`, and a stale runtime
// links without complaint and misbehaves.
fn check_runtime(lib: &Path) -> Result<(), String> {
    let bytes = std::fs::read(lib).map_err(|e| format!("cannot read {}: {}", lib.display(), e))?;
    let marker = format!("CBRT-STAMP:{}", cbrt::STAMP);
    if bytes.windows(marker.len()).any(|w| w == marker.as_bytes()) {
        return Ok(());
    }
    Err(format!(
        "{} is stale: it was not built from the sources this cobc was (rebuild it: `cargo build --release -p cbrt -p cobc`, or without --release for a debug cobc)",
        lib.display()
    ))
}

// `link_trim`: linker options that leave out of the executable what it does not use
// (GNU ld and lld, on Linux, where `cobc` builds): `--gc-sections` drops
// every section nothing references -- most of the Rust standard library
// the runtime is built against -- and `--strip-debug` the debug
// information, all of it the runtime's and the standard library's (the
// generated C is compiled without `-g`). The symbol table stays, so a
// profiler or debugger still names the program's functions. A hello
// world: 5.9 MB without them, 0.8 MB with them.
// With `-s`, `--strip-all` instead of `--strip-debug`: the symbol table
// goes too. (Adding `cc -s` beside `--strip-debug` does nothing: the
// driver puts its `-s` before the `-Wl,` options, and the linker keeps
// the last strip option it is given.)
fn link_trim(strip: bool) -> &'static [&'static str] {
    match (cfg!(target_os = "linux"), strip) {
        (false, _) => &[],
        (true, false) => &["-Wl,--gc-sections", "-Wl,--strip-debug"],
        (true, true) => &["-Wl,--gc-sections", "-Wl,--strip-all"],
    }
}

// `extra` links the program's own C code (`link::link_args`): after the
// generated program, before the runtime and the system libraries.
// `strip` (`-s`): the symbol table left out as well.
// Compiles and links `inputs` (one C file, or the cache's objects and the
// program's) into `exe`. Many inputs go through a response file in
// `work_dir`: a command line has a length limit (Windows: 32 K).
fn compile(cc: &str, inputs: &[PathBuf], exe: &Path, opt: &str, arch: Option<&str>, extra: &[std::ffi::OsString], strip: bool, work_dir: &Path) -> Result<std::process::Output, String> {
    let here = std::env::current_exe().map_err(|e| e.to_string())?;
    let lib_dir = here.parent().ok_or("no parent dir")?.to_path_buf();
    if !lib_dir.join(cbrt::LIB_FILE).exists() {
        return Err(format!("{} not found in {} (build the workspace: `cargo build --workspace`, or `cargo build -p cbrt -p cobc`)", cbrt::LIB_FILE, lib_dir.display()));
    }
    check_runtime(&lib_dir.join(cbrt::LIB_FILE))?;
    // `-ffp-contract=off`: gnu11 lets the compiler fuse `a*b + c` into one FMA
    // with a single rounding wherever the target has FMA (any -march
    // since Haswell); spec/06 rounds each operation on its own.
    // `-fno-optimize-sibling-calls`: gcc at -O2 turns a tail call into a
    // jump, so a recursion that exhausts the stack in coby (and with
    // clang, and at -O0) would run to completion; `[Call-Stack-Exhausted]`
    // counts calls, and every call stays one.
    // The runtime's system libraries are the runtime's own knowledge
    // (`cbrt::LINK_LIBS`), so this driver holds no per-platform list.
    // `link_trim`: the executable without what nothing in it uses.
    let mut cmd = Command::new(cc);
    cmd.args(cache::CC_FLAGS).arg(opt).args(arch).arg("-o").arg(exe);
    if inputs.len() == 1 {
        cmd.arg(&inputs[0]);
    } else {
        let resp = work_dir.join("objects.rsp");
        let mut text = String::new();
        for p in inputs {
            text.push('"');
            text.push_str(&p.display().to_string().replace('\\', "/"));
            text.push_str("\"\n");
        }
        std::fs::write(&resp, text).map_err(|e| format!("cannot write {}: {}", resp.display(), e))?;
        cmd.arg(format!("@{}", resp.display()));
    }
    cmd.args(extra)
        .arg("-L")
        .arg(&lib_dir)
        .arg("-lcbrt")
        .args(cbrt::LINK_LIBS.iter().map(|l| format!("-l{}", l)))
        .args(link_trim(strip));
    show(&cmd);
    cmd.output().map_err(|e| format!("cannot run the C compiler `{}`: {}", cc, e))
}
