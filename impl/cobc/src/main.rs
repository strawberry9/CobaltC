// cobc -- the CobaltC compiler driver (impl/COBC-PLAN.md §3.7).
//
//   cobc [--run] [--keep-c] [-v] [-s] [-O LEVEL] [-march=CPU] [--cc COMPILER] [-o OUT] FILE.cb
//   cobc --check PATH...
//   cobc --check-syntax [--origin PATH] (FILE | -)
//   cobc --emit-c [--c-name NAME] [-o OUT] FILE
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

mod cc;
mod link;
mod lower;
mod report;
mod target;
// D-0255: the runtime's link knowledge, from its own sources; `cobc` does
// not link the runtime crate (on Windows that pulls its C-facing code in).
#[path = "../../cbrt/src/link_info.rs"]
mod cbrt_link;
// The stamp of the runtime sources this `cobc` was built with (`build.rs`).
const CBRT_STAMP: &str = env!("CBRT_STAMP");

use std::fmt::Write as _;
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

const USAGE: &str = "usage: cobc [--explain] [--warn-shadow] [--unittest [--unittest-filter TEXT] [--unittest-std]] [--run] [--explore N] [--schedule-seed S] [--keep-c] [--no-line-directives] [-g] [--report-tracking] [--all-checks] [--bootstrap-form] [-v] [-s] [-O LEVEL] [-march=CPU] [--target TRIPLE] [--cc COMPILER] [-o OUT] FILE.cb [ARG...]\n       cobc [--explain] --lib [-o libNAME.a] [--target TRIPLE] [--cc COMPILER] FILE.cb\n       cobc [--explain] [--warn-shadow] --check PATH...\n       cobc --check-syntax [--origin PATH] (FILE | -)\n       cobc --emit-c [--c-name NAME] [-o OUT] FILE";

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
  --lib        Build a library (D-0257): the program's `extern fn` definitions
               as a static archive (default libNAME.a beside the source), the
               runtime libcbrt.a copied beside it, and NAME.h with the C
               prototypes, the structs that cross by value, and the link line.
               A library has no `main`; nothing runs.
  --keep-c     Keep the generated C file instead of deleting it after
               `cc`. It sits beside the executable with the extension
               replaced by `.c` (`-o prog` -> `prog.c`, `a.out` -> `a.c`).
               Under `--run` without `-o` it lives in the temporary
               directory and is deleted with it. When `cc` fails the C is
               always kept and its path printed.
  --no-line-directives
               Leave the `#line` directives out of the generated C. By
               default each line of C made from one of the program's own
               statements is preceded by `#line N \"FILE.cb\"`, so the C
               compiler's messages, a debugger and a sanitizer name the
               program's file and line rather than the C's; code made for
               `std` keeps the C's own lines.
  -g           Compile the generated C with debug information (`cc -g`)
               and keep it in the executable. With `-O0 -g` a debugger
               such as gdb steps through the program's `.cb` lines, and a
               sanitizer's report (`--cc` naming a compiler wrapper that
               adds `-fsanitize=address`) names them.
  --report-tracking
               Compile nothing: list the lines of the program's own files
               whose generated C still calls the runtime's tracking (paths,
               objects, checked accesses), those inside loops first, with
               the kinds of call on each. Bounds and overflow checks are
               not listed. Chapter P of the guide says what each form
               costs and how to write code the compiler can see through.
  --all-checks
               Use no proof about a function's reference parameters: every
               access through one is checked where it is made (D-0209). For
               comparing against the elided forms; slower.
  --bootstrap-form
               Emit the general checked form only: --all-checks, no
               elisions (D-0274). The reference for the code
               generator written in CobaltC; much slower.
               COBALTC_BOOTSTRAP_UPTO=N switches off D-0274's stages 1..N
               only (0 the default form, 10 this one; CHG-0368).
  --warn-shadow
               Note, on standard error, each item of the program whose name
               hides an imported one (a `struct Vec` beside `import std;`):
               the program's own item is the one used (`spec/17`), and this
               is the only place that says so. Exit status unchanged.
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
  --target TRIPLE
               The platform to compile for (D-0255): x86_64-unknown-linux-gnu
               or x86_64-pc-windows-gnu. Default: the one cobc runs on. For
               Windows from Linux the C compiler is MinGW-w64's
               (`x86_64-w64-mingw32-gcc`, or --cc), the runtime is the one in
               target/x86_64-pc-windows-gnu/release (built with Rust 1.77.2),
               and the executable is FILE.exe. --run and --explore run it with
               the program named by COBALTC_RUNNER, such as `wine64`.
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
  --emit-c [--c-name NAME] [-o OUT] FILE
               The program's C, nothing compiled or linked: the file
               `--keep-c` keeps, written to OUT, or on standard output
               without -o or with `-o -`. NAME is the name its `#line`
               directives give it (default OUT, else a.c). A program the
               static pass rejects gets its diagnostic and status 1, and
               OUT is not written.
  --check-syntax [--origin PATH] (FILE | -)
               Whether one text parses, and nothing else: FILE, or standard
               input for `-`, its `module m \"./m.cb\";` files found
               relative to PATH (FILE by default). Exit 0 it parses, 1 it
               does not (the error, `PLACE: MESSAGE (column C)`, on
               standard error), 2 it cannot be read. A program the parser
               builds but the static pass would reject parses. For the
               formatter written in CobaltC (impl/selfhost/cobfmt), which
               refuses what this refuses; `coby --check-syntax` is the same.
  --unittest   Build the program's tests instead of the program (D-0230):
               each `unittest` block in its files, run in order by a
               `main` of their own, each in a thread of its own, its
               fault reported and the next one run; the program's own
               `main` is checked but not run, and a file of tests needs
               none. The executable prints `test FILE:LINE NAME ... ok`
               (or `FAILED` and the fault) for each and a summary, and
               exits 1 when one failed. With --run, run them now: `cobc
               --unittest --run lib.cb`; with -o, keep the test program.
  --unittest-filter TEXT
               With --unittest: only the tests whose name contains TEXT
               (an unnamed test matches none); a filter that matches no
               test is an error.
  --unittest-std
               Run `std`'s own tests, the examples of the std reference,
               in place of the program's (implies --unittest).
  --explain    Show each diagnostic with its full entry from the
               specification's registry: the rule, the invariant, what was
               required and observed, the provenance and the examples, after
               the short form (the message, the location with the source line
               and a caret, and the repair). A compiled program shows its
               faults that way when COBALTC_EXPLAIN=1 is set in its
               environment, which --explain sets for --run.
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
without `-g` unless -g is given, which also keeps it); its symbol table
stays, for profilers and debuggers, unless -s removes it.

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
  4    internal error: a bug in cobc, reported as one line (with Rust's
       backtrace when RUST_BACKTRACE is set); please report it with
       the program
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
    coby::internal::install_hook("cobc");
    let stack = if cfg!(target_pointer_width = "64") { 1usize << 30 } else { 256 << 20 };
    std::thread::Builder::new()
        .stack_size(stack)
        .spawn(real_main)
        .expect("start the compiler thread")
        .join()
        .unwrap_or_else(|e| std::panic::resume_unwind(e))
}

fn real_main() -> ExitCode {
    coby::internal::test_point();
    // `--check`: the front end and the `extern "…";` files only (`coby::check`).
    let mut all: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    // `--explain` before `--check` (WI-9).
    while all.first().map_or(false, |a| a == "--explain" || a == "--warn-shadow") {
        std::env::set_var(if all[0] == "--explain" { "COBALTC_EXPLAIN" } else { "COBALTC_WARN_SHADOW" }, "1");
        all.remove(0);
    }
    if all.first().map_or(false, |a| a == "--check-syntax") {
        let rest: Vec<String> = all[1..].iter().map(|a| a.to_string_lossy().into_owned()).collect();
        return ExitCode::from(coby::check::run_syntax(&rest, "cobc"));
    }
    if all.first().map_or(false, |a| a == "--check") {
        // `--warn-shadow` may follow `--check` too (CHG-0333).
        all.retain(|a| if a == "--warn-shadow" { std::env::set_var("COBALTC_WARN_SHADOW", "1"); false } else { true });
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
    // `--emit-c [--c-name NAME] [-o OUT] FILE`: the C, nothing compiled;
    // written to OUT, or on standard output (as `cbc` has it) without `-o`
    // or with `-o -`. NAME is the name its `#line` directives give the C
    // file (default OUT, else `a.c`).
    let mut emit_c: Option<(String, Option<PathBuf>, PathBuf)> = None;
    if all.first().map_or(false, |a| a == "--emit-c") {
        let rest: Vec<String> = all[1..].iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let (mut name, mut to, mut file) = (None, None, None);
        let mut i = 0;
        while i < rest.len() {
            match rest[i].as_str() {
                "--c-name" if name.is_none() && i + 1 < rest.len() => {
                    name = Some(rest[i + 1].clone());
                    i += 2;
                }
                "-o" if to.is_none() && i + 1 < rest.len() => {
                    to = Some(rest[i + 1].clone());
                    i += 2;
                }
                f if file.is_none() && !f.starts_with('-') => {
                    file = Some(f.to_string());
                    i += 1;
                }
                _ => {
                    file = None;
                    break;
                }
            }
        }
        let Some(file) = file else {
            eprintln!("usage: cobc --emit-c [--c-name NAME] [-o OUT] FILE");
            return ExitCode::from(2);
        };
        let to = to.filter(|o| o != "-");
        let name = name.or_else(|| to.clone()).unwrap_or_else(|| "a.c".to_string());
        emit_c = Some((name, to.map(PathBuf::from), PathBuf::from(file)));
    }
    let mut run = false;
    let mut explore: Option<u64> = None;
    let mut seed: Option<u64> = None;
    let mut keep_c = false;
    let mut lib = false;
    let mut report_tracking = false;
    let mut strip = false;
    let mut debug = false;
    let mut line_directives = true;
    let mut unittest: Option<coby::unittest::Mode> = None;
    let mut opt = String::from("-O2");
    let mut arch: Option<String> = None;
    let mut cc: Option<String> = None;
    let mut target: Option<target::Target> = None;
    let mut out: Option<PathBuf> = None;
    let mut file: Option<PathBuf> = None;
    let mut prog_args: Vec<std::ffi::OsString> = Vec::new();
    if let Some((_, _, f)) = &emit_c {
        file = Some(f.clone());
    }
    // Bytes, not text: a program's argument need not be UTF-8. (`--emit-c`'s
    // command line was read above: none is left.)
    let mut args = std::env::args_os().skip(if emit_c.is_some() { usize::MAX } else { 1 });
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
            "--lib" => lib = true,
            "--report-tracking" => report_tracking = true,
            "--all-checks" => std::env::set_var("COBALTC_ALL_CHECKS", "1"),
            "--bootstrap-form" => std::env::set_var("COBALTC_BOOTSTRAP_FORM", "1"),
            "--warn-shadow" => std::env::set_var("COBALTC_WARN_SHADOW", "1"),
            "--explain" => std::env::set_var("COBALTC_EXPLAIN", "1"),
            "--unittest" => {
                unittest.get_or_insert_with(Default::default);
            }
            "--unittest-std" => unittest.get_or_insert_with(Default::default).std_tests = true,
            "--unittest-filter" => match next(&mut args) {
                Some(t) => unittest.get_or_insert_with(Default::default).filter = Some(t),
                None => return usage(),
            },
            "-v" | "--verbose" => VERBOSE.store(true, Ordering::Relaxed),
            "-s" | "--strip" => strip = true,
            "-g" => debug = true,
            "--no-line-directives" => line_directives = false,
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
                Some(c) if !c.is_empty() => cc = Some(c),
                _ => return usage(),
            },
            _ if a.starts_with("--cc=") && a.len() > "--cc=".len() => cc = Some(a["--cc=".len()..].to_string()),
            "--target" => match next(&mut args).as_deref().map(|t| (t.to_string(), target::Target::parse(t))) {
                Some((_, Some(t))) => target = Some(t),
                Some((t, None)) => return bad_target(&t),
                None => return usage(),
            },
            _ if a.starts_with("--target=") => match target::Target::parse(&a["--target=".len()..]) {
                Some(t) => target = Some(t),
                None => return bad_target(&a["--target=".len()..]),
            },
            _ if a.starts_with('-') => return usage(),
            _ if file.is_some() => return usage(),
            _ => file = Some(PathBuf::from(a)),
        }
    }
    let Some(file) = file else { return usage() };
    // D-0255: the target, and the C compiler it implies unless --cc names one.
    let host = target::Target::host();
    let Some(target) = target.or(host) else {
        eprintln!("cobc: this cobc was built for a platform it cannot compile natively for (build it for x86_64-pc-windows-gnu on Windows); name one with --target");
        return ExitCode::from(2);
    };
    let cc = cc.unwrap_or_else(|| target.default_cc(host).to_string());
    let foreign = Some(target) != host;
    let src = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {}", file.display(), e);
            return ExitCode::from(2);
        }
    };

    coby::unittest::set_mode(unittest);
    let analysis = coby::analyze(&src, Some(&file));
    if let Some(d) = analysis.error {
        eprint!("{}", coby::diagnostics::for_stderr(&coby::render_located(&d, "static", &analysis.map)));
        return ExitCode::FAILURE;
    }
    let items = analysis.items.expect("checked");
    // D-0257: a library is functions C calls (`extern fn` with a body) and
    // no `main`, built with `--lib`; a program is built without it.
    let has_main = items.fns.contains_key("main");
    let has_entries = items.fns.values().any(|f| f.c_entry);
    if lib && has_main {
        eprintln!("cobc: --lib builds a library, which has no `main`: remove `main`, or build the program without --lib");
        return ExitCode::from(2);
    }
    if lib && !has_entries {
        eprintln!("cobc: --lib builds a library of functions C calls, and {} defines none (`extern fn name(…) : τ {{ … }}`)", file.display());
        return ExitCode::from(2);
    }
    if !lib && !has_main {
        eprintln!(
            "cobc: {} is a library (it defines functions C calls and no `main`): build it with `cobc --lib -o lib{}.a {}`",
            file.display(),
            file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            file.display()
        );
        return ExitCode::from(2);
    }
    if lib && (run || explore.is_some()) {
        eprintln!("cobc: --lib builds a library; nothing runs (--run, --explore)");
        return ExitCode::from(2);
    }
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
    let lower_as = |decided: bool| {
        let mut lowering = lower::Gen::new(&items, &analysis.map);
        lowering.line_directives = line_directives && !report_tracking;
        lowering.lib = lib;
        // D-0284: a decided program is lowered with no record of references;
        // `--all-checks` (the generator refuses it) and COBALTC_PATHLESS=0 keep it.
        lowering.pathless = std::env::var("COBALTC_PATHLESS").map_or(true, |v| v != "0");
        lowering.decided = decided;
        lowering.generate()
    };
    let gen = match lower_as(false) {
        Ok(g) => g,
        Err(lower::Unsupported(what)) => {
            eprintln!("unsupported: {}", what);
            return ExitCode::from(3);
        }
    };
    // D-0285: a program the first lowering found decided is lowered again
    // knowing it (`Gen::decided`); COBALTC_DECIDED=0 keeps the first.
    let gen = if gen.pathless.is_ok() && std::env::var("COBALTC_DECIDED").map_or(true, |v| v != "0") {
        match lower_as(true) {
            Ok(g) if g.pathless.is_ok() => g,
            Ok(g) => {
                note(&format!("decided lowering kept paths ({}); the first lowering is used", g.pathless.err().unwrap_or_default()));
                gen
            }
            Err(lower::Unsupported(what)) => {
                note(&format!("decided lowering: unsupported: {}; the first lowering is used", what));
                gen
            }
        }
    } else {
        gen
    };
    match &gen.pathless {
        Ok(()) if gen.decided => note("lowered pathless and decided (D-0284, D-0285)"),
        Ok(()) => note("lowered pathless (D-0284)"),
        Err(why) => note(&format!("kept paths: {}", why)),
    }
    if report_tracking {
        match &gen.pathless {
            Ok(()) => println!("no run-time tracking: every conflict is decided before the run, so the program keeps no record of references (D-0284)"),
            Err(why) => print!("The program keeps a record of references: {}\n{}", why, report::report(&gen.combined("report.c"), &file)),
        }
        return ExitCode::SUCCESS;
    }
    note(&format!("lowered to C in {} ms ({} units)", t_lower.elapsed().as_millis(), gen.units.len()));
    if let Some((name, to, _)) = &emit_c {
        use std::io::Write as _;
        let c = gen.combined(name);
        if let Some(to) = to {
            if let Err(e) = std::fs::write(to, c.as_bytes()) {
                eprintln!("cobc: cannot write {}: {}", to.display(), e);
                return ExitCode::from(1);
            }
            return ExitCode::SUCCESS;
        }
        let mut out = std::io::stdout().lock();
        let ok = out.write_all(c.as_bytes()).and_then(|_| out.flush()).is_ok();
        return if ok { ExitCode::SUCCESS } else { ExitCode::from(1) };
    }

    let scratch = if run {
        let d = std::env::temp_dir().join(format!("cobc-{}-{}", std::process::id(), unique()));
        std::fs::create_dir_all(&d).expect("temp dir");
        Some(d)
    } else {
        None
    };
    let exe = match (&out, &scratch) {
        (Some(o), _) => o.clone(),
        (None, Some(d)) => if target.windows() { d.join("a.exe") } else { d.join("a.out") },
        (None, None) if lib => file.with_file_name(format!("lib{}.a", file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())),
        (None, None) => target.exe_name(&file.with_extension("")),
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
    // D-0277: the program's single C file, compiled every time; kept for
    // inspection with `--keep-c`, and when something fails.
    std::fs::write(&c_path, gen.combined(&c_path.display().to_string())).expect("write C");
    note(&format!("wrote the generated C to {}", c_path.display()));

    let obj_dir = std::env::temp_dir().join(format!("cobc-obj-{}-{}", std::process::id(), unique()));
    let linked = std::fs::create_dir_all(&obj_dir)
        .map_err(|e| format!("cannot create {}: {}", obj_dir.display(), e))
        .and_then(|_| link::link_args(&code, &cc, &opt, arch.as_deref(), &obj_dir, target))
        .and_then(|extra| {
            let t = std::time::Instant::now();
            let r = if lib {
                let obj = obj_dir.join("program.o");
                cc::compile_one(&cc, &opt, arch.as_deref(), debug, &c_path, &obj)?;
                archive(&cc, std::slice::from_ref(&obj), &extra, &exe, target)
            } else {
                build_exe(&gen, &cc, &c_path, &exe, &opt, arch.as_deref(), &extra, strip, debug, &obj_dir, target)
            };
            note(&format!("compiled and linked in {} ms", t.elapsed().as_millis()));
            r
        });
    let _ = std::fs::remove_dir_all(&obj_dir);
    let output = match linked {
        Ok(o) => o,
        Err(e) => {
            eprintln!("cobc: {}", e);
            return ExitCode::from(2);
        }
    };
    if lib && output.status.success() {
        match write_header(&exe, &gen.header_text, &code, target) {
            Ok(p) => note(&format!("wrote the header {}", p.display())),
            Err(e) => {
                eprintln!("cobc: {}", e);
                return ExitCode::from(2);
            }
        }
    }
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

    // A program for another platform runs under COBALTC_RUNNER (`wine64`).
    let runner: Option<Vec<String>> = if foreign && (run || explore.is_some()) {
        match std::env::var("COBALTC_RUNNER").ok().filter(|r| !r.trim().is_empty()) {
            Some(r) => Some(r.split_whitespace().map(str::to_string).collect()),
            None => {
                eprintln!("cobc: built {} for {}; running it here needs COBALTC_RUNNER, the program that runs it (such as `wine64`)", exe.display(), target.triple());
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    if let Some(n) = explore {
        let code = explore_runs(&exe, &prog_args, n, &file, runner.as_deref());
        if let Some(d) = &scratch {
            let _ = std::fs::remove_dir_all(d);
        }
        return code;
    }
    let code = if run {
        let mut cmd = run_command(&exe, runner.as_deref());
        cmd.args(&prog_args);
        if let Some(s) = seed {
            cmd.env("COBALTC_SCHEDULE_SEED", s.to_string());
        }
        child_guard(&mut cmd);
        show(&cmd);
        let st = cmd.spawn().and_then(|mut c| {
            let _job = job_guard(&c);
            c.wait()
        });
        let st = st.expect("run compiled program");
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
// D-0255: the command that runs a built program: itself, or its target's
// runner with it as the first argument.
fn run_command(exe: &std::path::Path, runner: Option<&[String]>) -> Command {
    match runner {
        Some([prog, rest @ ..]) => {
            let mut c = Command::new(prog);
            c.args(rest).arg(exe);
            c
        }
        _ => Command::new(exe),
    }
}

fn bad_target(t: &str) -> ExitCode {
    let all: Vec<&str> = target::TRIPLES.iter().map(|(s, _)| *s).collect();
    eprintln!("cobc: --target {}: not a target cobc compiles for; it compiles for {}", t, all.join(" and "));
    ExitCode::from(2)
}

fn explore_runs(exe: &std::path::Path, prog_args: &[std::ffi::OsString], runs: u64, file: &std::path::Path, runner: Option<&[String]>) -> ExitCode {
    use std::io::{Read, Write};
    use std::sync::{Arc, Condvar, Mutex};
    // Every run reads the same standard input. It is gathered as it
    // arrives, by a thread of its own, and each run is given what has
    // arrived and then the rest as it comes, its end only at the input's
    // end: a program that never reads, beside an input that is never
    // closed (a socket or pipe a supervisor holds open), used to wait for
    // that end before its first run, for ever.
    let input: Arc<(Mutex<(Vec<u8>, bool)>, Condvar)> = Arc::new((Mutex::new((Vec::new(), false)), Condvar::new()));
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        input.0.lock().unwrap().1 = true;
    } else {
        let gather = input.clone();
        std::thread::spawn(move || {
            let mut chunk = vec![0u8; 1 << 16];
            let mut stdin = std::io::stdin();
            loop {
                let n = stdin.read(&mut chunk).unwrap_or(0);
                let mut st = gather.0.lock().unwrap();
                if n == 0 {
                    st.1 = true;
                } else {
                    st.0.extend_from_slice(&chunk[..n]);
                }
                let ended = st.1;
                drop(st);
                gather.1.notify_all();
                if ended {
                    return;
                }
            }
        });
    }
    // (status, stdout, stderr) -> (count, first seed)
    let mut seen: Vec<((Option<i32>, Vec<u8>, Vec<u8>), u64, u64)> = Vec::new();
    for s in 1..=runs {
        let mut cmd = run_command(exe, runner);
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
        let _job = job_guard(&child);
        // The run's input, fed until the input ends (its pipe then closed)
        // or the run does (`over`).
        let over = Arc::new(AtomicBool::new(false));
        let feeder = child.stdin.take().map(|mut w| {
            let (input, over) = (input.clone(), over.clone());
            std::thread::spawn(move || {
                let mut sent = 0;
                loop {
                    let (more, ended) = {
                        let mut st = input.0.lock().unwrap();
                        while st.0.len() == sent && !st.1 && !over.load(Ordering::SeqCst) {
                            st = input.1.wait(st).unwrap();
                        }
                        if over.load(Ordering::SeqCst) {
                            return;
                        }
                        (st.0[sent..].to_vec(), st.1)
                    };
                    if !more.is_empty() && w.write_all(&more).is_err() {
                        return;
                    }
                    sent += more.len();
                    if ended {
                        return;
                    }
                }
            })
        });
        let out = child.wait_with_output();
        over.store(true, Ordering::SeqCst);
        drop(input.0.lock().unwrap());
        input.1.notify_all();
        if let Some(f) = feeder {
            let _ = f.join();
        }
        let out = match out {
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

// D-0255: on Windows, the program `--run` and `--explore` start goes into a
// job object that ends it when `cobc` ends (the job's last handle closes),
// with `COBALTC_CPU_LIMIT` seconds of processor time if set: what
// `child_guard` does on Linux. A test runner that stops `cobc` at its
// timeout no longer leaves the program running. Best effort: where the
// program cannot be put in a job (Windows 7 has no nested jobs, and some
// consoles run every process in one), it runs without. The job is held
// while the returned value lives.
#[cfg(windows)]
fn job_guard(child: &std::process::Child) -> Option<JobHandle> {
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    #[derive(Default)]
    struct BasicLimit {
        per_process_user_time: i64,
        per_job_user_time: i64,
        flags: u32,
        min_working_set: usize,
        max_working_set: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimit {
        basic: BasicLimit,
        io: [u64; 6],
        process_memory: usize,
        job_memory: usize,
        peak_process_memory: usize,
        peak_job_memory: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attrs: *mut u8, name: *const u16) -> *mut u8;
        fn SetInformationJobObject(job: *mut u8, class: u32, info: *const u8, len: u32) -> i32;
        fn AssignProcessToJobObject(job: *mut u8, process: *mut u8) -> i32;
    }
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;
    const KILL_ON_JOB_CLOSE: u32 = 0x2000;
    const PROCESS_TIME: u32 = 0x2;
    let limit: Option<i64> = std::env::var("COBALTC_CPU_LIMIT").ok().and_then(|v| v.trim().parse().ok());
    unsafe {
        let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
        if job.is_null() {
            return None;
        }
        let mut info = ExtendedLimit::default();
        info.basic.flags = KILL_ON_JOB_CLOSE;
        if let Some(secs) = limit {
            info.basic.flags |= PROCESS_TIME;
            info.basic.per_process_user_time = secs * 10_000_000;
        }
        let set = SetInformationJobObject(job, JOB_OBJECT_EXTENDED_LIMIT_INFORMATION, &info as *const ExtendedLimit as *const u8, std::mem::size_of::<ExtendedLimit>() as u32);
        let handle = JobHandle(job);
        if set == 0 || AssignProcessToJobObject(job, child.as_raw_handle() as *mut u8) == 0 {
            return None;
        }
        Some(handle)
    }
}

#[cfg(windows)]
struct JobHandle(*mut u8);

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        #[link(name = "kernel32")]
        extern "system" {
            fn CloseHandle(h: *mut u8) -> i32;
        }
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(not(windows))]
fn job_guard(_child: &std::process::Child) -> Option<()> {
    None
}

fn unique() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
}

// The runtime beside this executable must be the one built from the
// sources this `cobc` was built with: `cargo test` can rebuild `cobc`
// without refreshing `target/<profile>/libcbrt.a`, and a stale runtime
// links without complaint and misbehaves.
fn check_runtime(lib: &Path, target: target::Target) -> Result<(), String> {
    let bytes = std::fs::read(lib).map_err(|e| format!("cannot read {}: {}", lib.display(), e))?;
    let marker = format!("CBRT-STAMP:{}", CBRT_STAMP);
    if bytes.windows(marker.len()).any(|w| w == marker.as_bytes()) {
        return Ok(());
    }
    Err(format!(
        "{} is stale: it was not built from the sources this cobc was (rebuild it: {})",
        lib.display(),
        target.runtime_build()
    ))
}

// `link_trim`: linker options that leave out of the executable what it does not use
// (GNU ld and lld; MinGW-w64's GNU ld for Windows): `--gc-sections` drops
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
fn link_trim(strip: bool, debug: bool, _target: target::Target) -> &'static [&'static str] {
    // GNU ld on both targets (MinGW's for Windows).
    match (true, strip, debug) {
        (false, _, _) => &[],
        (true, false, false) => &["-Wl,--gc-sections", "-Wl,--strip-debug"],
        (true, false, true) => &["-Wl,--gc-sections"],
        (true, true, _) => &["-Wl,--gc-sections", "-Wl,--strip-all"],
    }
}

// `extra` links the program's own C code (`link::link_args`): after the
// generated program, before the runtime and the system libraries.
// `strip` (`-s`): the symbol table left out as well; `debug` (`-g`):
// debug information compiled in and kept (unless `-s`).
// Compiles and links `inputs` (the program's C file) into `exe`. Many inputs go through a response file in
// `work_dir`: a command line has a length limit (Windows: 32 K).
#[allow(clippy::too_many_arguments)]
// D-0257 (`--lib`): the program's objects, and the objects of its own C
// and assembly files, into one static archive; the runtime stays its own
// archive, `libcbrt.a`, copied beside the output so that a C build has
// both at hand (two libraries in one program share it). A library name
// the program links (`extern "z";`) or a shared library cannot go into an
// archive: the header names them for the C build.
fn archive(cc: &str, objects: &[PathBuf], extra: &[std::ffi::OsString], out: &Path, target: target::Target) -> Result<std::process::Output, String> {
    let here = std::env::current_exe().map_err(|e| e.to_string())?;
    let lib_dir = target.runtime_dir(here.parent().ok_or("no parent dir")?);
    let runtime = lib_dir.join(target.lib_file());
    if !runtime.exists() {
        return Err(format!("the runtime for {}, {}, is not in {} (build it: {})", target.triple(), target.lib_file(), lib_dir.display(), target.runtime_build()));
    }
    check_runtime(&runtime, target)?;
    // The archiver beside the C compiler: a cross compiler's own.
    let ar = match cc.rsplit_once("-gcc").or_else(|| cc.rsplit_once("-clang")) {
        Some((prefix, _)) if prefix.contains("mingw32") => format!("{}-ar", prefix),
        _ => "ar".to_string(),
    };
    let _ = std::fs::remove_file(out);
    let mut cmd = Command::new(&ar);
    cmd.arg("rcs").arg(out);
    for o in objects {
        cmd.arg(o);
    }
    for e in extra {
        if e.to_string_lossy().ends_with(".o") {
            cmd.arg(e);
        }
    }
    show(&cmd);
    let output = cmd.output().map_err(|e| format!("cannot run the archiver `{}`: {}", ar, e))?;
    if output.status.success() {
        let dest = out.parent().unwrap_or(Path::new(".")).join(target.lib_file());
        if std::fs::canonicalize(&dest).ok() != std::fs::canonicalize(&runtime).ok() {
            std::fs::copy(&runtime, &dest).map_err(|e| format!("cannot copy {} to {}: {}", runtime.display(), dest.display(), e))?;
            note(&format!("copied the runtime to {}", dest.display()));
        }
    }
    Ok(output)
}

// D-0257: the header beside the library: a guard, the link line for its
// target, and what the lowering declared (`Generated::header_text`).
fn write_header(out: &Path, text: &str, code: &[link::Code], target: target::Target) -> Result<PathBuf, String> {
    let stem = out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "cobalt".to_string());
    let short = stem.strip_prefix("lib").unwrap_or(&stem).to_string();
    let path = out.with_file_name(format!("{}.h", short));
    let guard: String = format!("COBALT_{}_H", short.to_uppercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>());
    let mut libs: Vec<String> = vec![format!("-l{}", short), "-lcbrt".to_string()];
    for c in code {
        match c {
            link::Code::Name(n) => libs.push(format!("-l{}", n)),
            link::Code::File(p, link::Kind::Static) | link::Code::File(p, link::Kind::Shared) => libs.push(p.display().to_string()),
            _ => {}
        }
    }
    libs.extend(target.link_libs().iter().map(|l| format!("-l{}", l)));
    let mut h = String::new();
    writeln!(h, "/* {}: the C interface of {}, written by cobc (D-0257).", path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), out.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()).unwrap();
    writeln!(h, "   Link, with this file's directory on the library path (-L): {}", libs.join(" ")).unwrap();
    writeln!(h, "   The runtime, {}, is beside the library; a program that links two CobaltC libraries links it once. */", target.lib_file()).unwrap();
    writeln!(h, "#ifndef {g}
#define {g}
", g = guard).unwrap();
    h.push_str(text);
    writeln!(h, "
#endif /* {} */", guard).unwrap();
    std::fs::write(&path, h).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    Ok(path)
}

// The runtime's directory for `target`, checked present and current.
fn runtime_dir(target: target::Target) -> Result<PathBuf, String> {
    let here = std::env::current_exe().map_err(|e| e.to_string())?;
    let lib_dir = target.runtime_dir(here.parent().ok_or("no parent dir")?);
    if !lib_dir.join(target.lib_file()).exists() {
        return Err(format!("the runtime for {}, {}, is not in {} (build it: {})", target.triple(), target.lib_file(), lib_dir.display(), target.runtime_build()));
    }
    check_runtime(&lib_dir.join(target.lib_file()), target)?;
    Ok(lib_dir)
}

// D-0277: the program's C compiled and linked into `exe`. A large program's
// C is compiled in parts (`cc::part_count`, `Generated::parts`), all at once,
// each written into `work_dir` under its own name, then linked; with
// link-time optimisation where the C compiler has it (`cc::Lto`). One part
// is the C file itself, compiled and linked in one command.
#[allow(clippy::too_many_arguments)]
fn build_exe(gen: &lower::Generated, cc: &str, c_path: &Path, exe: &Path, opt: &str, arch: Option<&str>, extra: &[std::ffi::OsString], strip: bool, debug: bool, work_dir: &Path, target: target::Target) -> Result<std::process::Output, String> {
    let lib_dir = runtime_dir(target)?;
    let k = cc::part_count(gen.units.iter().map(|u| u.text.len()).sum());
    // A program calling into the runtime, linked as the program will be.
    let probe = |flags: &[&str]| -> bool {
        let src = work_dir.join("lto-probe.c");
        if std::fs::write(&src, cc::LTO_PROBE).is_err() {
            return false;
        }
        let mut cmd = Command::new(cc);
        cmd.args(cc::cc_flags(cc)).arg(opt).args(arch).args(flags).arg("-o").arg(work_dir.join("lto-probe")).arg(&src).arg("-L").arg(&lib_dir).arg("-lcbrt").args(target.link_libs().iter().map(|l| format!("-l{}", l)));
        show(&cmd);
        cmd.output().map_or(false, |o| o.status.success())
    };
    let lto = cc::lto(cc, opt, k, &probe);
    if k == 1 {
        return compile(cc, &[c_path.to_path_buf()], exe, opt, arch, extra, strip, debug, work_dir, target, &lto.link_flags());
    }
    let mut children = Vec::new();
    let mut objects = Vec::new();
    for (i, text) in gen.parts(k).into_iter().enumerate() {
        let part = work_dir.join(format!("part{}.c", i));
        let obj = work_dir.join(format!("part{}.o", i));
        std::fs::write(&part, lower::settle_lines(&text, &part.display().to_string())).map_err(|e| format!("cannot write {}: {}", part.display(), e))?;
        let mut cmd = Command::new(cc);
        cmd.args(cc::cc_flags(cc)).arg(opt).args(arch).args(debug.then_some("-g")).args(lto.part_flags()).arg("-c").arg("-o").arg(&obj).arg(&part);
        cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        show(&cmd);
        let child = cmd.spawn().map_err(|e| format!("cannot run the C compiler `{}`: {}", cc, e))?;
        children.push((part, child));
        objects.push(obj);
    }
    let mut failed: Option<String> = None;
    for (part, child) in children {
        let out = child.wait_with_output().map_err(|e| format!("cannot run the C compiler `{}`: {}", cc, e))?;
        if !out.status.success() && failed.is_none() {
            failed = Some(format!("{}{} failed on {}", String::from_utf8_lossy(&out.stderr), cc, part.display()));
        }
    }
    if let Some(e) = failed {
        return Err(e);
    }
    compile(cc, &objects, exe, opt, arch, extra, strip, debug, work_dir, target, &lto.link_flags())
}

#[allow(clippy::too_many_arguments)]
fn compile(cc: &str, inputs: &[PathBuf], exe: &Path, opt: &str, arch: Option<&str>, extra: &[std::ffi::OsString], strip: bool, debug: bool, work_dir: &Path, target: target::Target, lto: &[&str]) -> Result<std::process::Output, String> {
    let lib_dir = runtime_dir(target)?;
    // `-ffp-contract=off`: gnu11 lets the compiler fuse `a*b + c` into one FMA
    // with a single rounding wherever the target has FMA (any -march
    // since Haswell); spec/06 rounds each operation on its own.
    // `-fno-optimize-sibling-calls`: gcc at -O2 turns a tail call into a
    // jump, so a recursion that exhausts the stack in coby (and with
    // clang, and at -O0) would run to completion; `[Call-Stack-Exhausted]`
    // counts calls, and every call stays one.
    // The runtime's system libraries are the runtime's own knowledge
    // (`cbrt/src/link_info.rs`), so this driver holds no per-platform list.
    // `link_trim`: the executable without what nothing in it uses.
    let mut cmd = Command::new(cc);
    cmd.args(cc::cc_flags(cc)).arg(opt).args(arch).args(debug.then_some("-g")).args(lto).arg("-o").arg(exe);
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
        .args(target.link_libs().iter().map(|l| format!("-l{}", l)))
        .args(link_trim(strip, debug, target));
    show(&cmd);
    cmd.output().map_err(|e| format!("cannot run the C compiler `{}`: {}", cc, e))
}
