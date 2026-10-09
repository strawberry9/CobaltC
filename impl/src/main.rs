use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

// Coby, for `--about`.
const COBY_ART: &str = concat!(
    "                        ...::--====-:..\n",
    "                     .:------::--==----*+-.\n",
    "                  -+#%#+-::.:::-=++=-+#@@@%#=.\n",
    "               .+#@%#**#%%%*+*#%%@@@@@@%#####%+.\n",
    "             .:==-=+*#%@@@@@@@%#**+=======:::.=*-\n",
    "            :=-::=#%@@@%#*=-::....::-===-:.-+- .*=\n",
    "           -+:.-#%%%%*-..    ...:::::::.     .. .**\n",
    "          .=:..+%%%%+  :....  ...........-=++-...:#+\n",
    "         :=*+-:-=*%%: ::...::-:........:+##*##*:. +*\n",
    "        =##%%@*+*=+@- ...:+####*-......=#+:..-*=. =#\n",
    "       -#+:-*%@*:-.## ..:*#+--=*#:.....-+:...... .#*\n",
    "       +#:  :*%@-..-@+ .:*+....:-:...... .    ..-##\n",
    "       -%=  .*#%=..:#%+.               ...:-=+*##+.\n",
    "        +#+:=##*:..:**##+=--:::----==++*###%##*=:\n",
    "         :+##*+-:-::-++*######%%%%%%####**+=:.\n",
    "           ... .. .::::::==---======--::.\n",
    "               .-+***+:.:--=-:.   .::. .=#:-=+=:\n",
    "      .-=--:..:=+*%@@@%-.:-*@@%#****######-=*+%@*.\n",
    "      :::-----=---=*###=..-:%@%+*#+===---:::-=-+---\n",
    "       :::::....:::::--=+: =@#::-===------:..:+ =@+\n",
    "     .:.::+==++----:::..:=-%%:...-==--=+===-..+=#@\n",
    "   .+##-..:::-==+**==-:..-=*%:...+*-+*++++#-..#=*#\n",
    " :-#%##=:-.=##- .::::::..:==%#-...::.........*#:**\n",
    ".=.+##-*@#:=#**=..........-=*%%*-:..:----..=#%-\n",
    ".-:#@+ *%#*.-##+..........:=-+*%%#+--+++++#%*:\n",
    " .:###..*##+.-##=..........-- :=*%%%#####%*=.\n",
    "   .=*+-:=*#=.-:  ...:......-:-  =*****++-...\n",
    "      ...  ..       --..:=--=%%+ .:::::.  :*%%+:\n",
    "                  =****#@@@@@@%%- ...:.:-*%@@@@##\n",
    "                .*%##%%@@@@%%###:  :*+*##%%%@@%%%\n",
    "                *#++##%%%%%%###-   *%***##%%%%%%#\n",
    "               -+:..-#%%%%##++-    +=::-*#####*=+\n",
    "              .-.:.:.=+*+++:..     .-....+*====-\n",
    "             :-:....:-+*=-...:    :-:... ..:--+#*\n",
    "            .:..:..::-**=:::...   ..... .....:-=+\n",
    "              ......:-==::..:.         ......::-=\n",
    "                  ..::--:..                  ...:.\n",
);

// What `coby` is, next to `cobc`, for `--about`.
const ABOUT: &str = concat!(
    "\n",
    "coby -- the CobaltC reference interpreter\n",
    "\n",
    "coby runs a CobaltC program straight from its source. It checks the\n",
    "whole program against the specification's static rules, then executes\n",
    "it on an abstract machine that follows the specification's rules as\n",
    "written: every object, reference and access path is tracked, and every\n",
    "check the specification leaves to run time is made. It is the\n",
    "reference implementation, written to follow the specification closely\n",
    "rather than to be fast.\n",
    "\n",
    "cobc, the CobaltC compiler, shares coby's front end -- the parser, name\n",
    "resolution and static checks -- so it accepts and rejects exactly the\n",
    "same programs, with the same diagnostics. It then translates the\n",
    "program to C and compiles that with a C compiler (GCC or Clang) into a\n",
    "native executable, keeping only the run-time checks it cannot prove\n",
    "unnecessary, so compiled programs run many times faster. The test\n",
    "suites require the two to agree on every program's output and\n",
    "diagnostics; coby is the oracle cobc is checked against.\n",
    "\n",
    "Use coby to run a program at once, with no build step; use cobc to\n",
    "build an executable, or to see how fast a program can be.\n",
    "\n",
    "  coby FILE.cb [ARG...]   run a program, passing it ARG...\n",
    "  coby --check PATH...    check programs (a directory: its .cb files)\n",
    "                          statically, running nothing\n",
    "  coby --about            show this\n",
    "  cobc --about            the compiler\n",
);

// D-0077: large enough for deep recursion (hundreds of thousands of
// calls), small enough that a runaway one faults before committing much
// memory. An unoptimized build's eval frames are several times larger, so
// it gets twice the reservation (reserved, not committed). The static
// pass recurses on the program's nesting, so `--check` uses it too.
fn stack_size() -> usize {
    if cfg!(target_pointer_width = "64") {
        if cfg!(debug_assertions) { 4usize << 30 } else { 2 << 30 }
    } else {
        256 << 20
    }
}

fn main() -> ExitCode {
    // Bytes, not text: an argument need not be UTF-8 (`rule.stdlib.args`).
    let args: Vec<OsString> = env::args_os().collect();
    if args.len() == 2 && args[1] == "--about" {
        print!("{}{}", COBY_ART, ABOUT);
        return ExitCode::SUCCESS;
    }
    let usage = || {
        eprintln!("usage: coby <file.cb> [arg...]");
        eprintln!("       coby --check <file.cb or directory>...");
        eprintln!("       coby --schedule-seed <n> <file.cb> [arg...]");
        eprintln!("       coby --explore <runs> <file.cb> [arg...]");
        eprintln!("       coby --about");
        ExitCode::from(2)
    };
    if args.len() < 2 || (args[1] == "--check" && args.len() < 3) {
        return usage();
    }
    // D-0204: `--explore N`: the program N times, under the schedules of
    // seeds 1 to N, each as a process of its own given the same input; the
    // distinct outcomes, each with a seed that replays it. Exit 0 when
    // every run ended alike, 1 when they did not.
    if args[1] == "--explore" {
        let runs: Option<u64> = args.get(2).and_then(|a| a.to_str()).and_then(|a| a.parse().ok());
        let (Some(runs), true) = (runs, args.len() >= 4) else { return usage() };
        return explore(runs, &args[3..]);
    }
    // `--schedule-seed S`: the threads interleaved as seed S decides.
    let args: Vec<OsString> = if args[1] == "--schedule-seed" {
        let seed: Option<u64> = args.get(2).and_then(|a| a.to_str()).and_then(|a| a.parse().ok());
        let (Some(seed), true) = (seed, args.len() >= 4) else { return usage() };
        coby::interp::set_schedule_seed(seed.max(1));
        std::iter::once(args[0].clone()).chain(args[3..].iter().cloned()).collect()
    } else {
        args
    };
    // `--check`: the static pass alone, on each file and each directory's
    // `.cb` files; nothing runs (`coby::check`). Exit 0 all accepted, 1 one
    // rejected, 2 a missing or unreadable file.
    if args[1] == "--check" {
        let paths: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
        let checker = std::thread::Builder::new()
            .stack_size(stack_size())
            .spawn(move || coby::check::run(&paths, "coby", &|_| Ok(())))
            .expect("start the checker thread");
        return ExitCode::from(checker.join().unwrap_or_else(|e| std::panic::resume_unwind(e)));
    }
    let path = Path::new(&args[1]);
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {}", path.display(), e);
            return ExitCode::from(2);
        }
    };
    // Everything after the file is the program's, argument 0 first: its
    // bytes on Unix; on Windows, whose arguments are UTF-16, their WTF-8
    // form -- UTF-8 for any well-formed argument, and not UTF-8 (so
    // `arg` reports it) for one with an unpaired surrogate.
    let prog_args: Vec<Vec<u8>> = args[2..].iter().map(|a| a.as_encoded_bytes().to_vec()).collect();
    // The interpreter recurses as the program does; the main thread's
    // default stack (8 MiB, 1 MiB on Windows) ended a program some ten
    // thousand calls deep with a host crash. The stack is reserved, not
    // committed, so a large one costs only what is used.
    let owned_path = path.to_path_buf();
    let stack = stack_size();
    let runner = std::thread::Builder::new()
        .stack_size(stack)
        .spawn(move || {
            // D-0077: a call too deep for this stack is a fault, not a crash.
            coby::interp::set_stack_budget(stack);
            coby::run_program_with_args(&src, Some(&owned_path), prog_args)
        })
        .expect("start the interpreter thread");
    let (outcome, map) = runner.join().unwrap_or_else(|e| std::panic::resume_unwind(e));
    let (phase, d) = match outcome {
        // `ok(s)`: `main`'s exit status (`rule.fn.program`).
        coby::OutcomeLocated::Ok(s) => return ExitCode::from(s),
        coby::OutcomeLocated::Static(d) => ("static", d),
        coby::OutcomeLocated::Dynamic(d) => ("dynamic", d),
    };
    // A construct this interpreter cannot run (an `extern fn` it cannot
    // call) is reported as `cobc` reports what it cannot compile: exit 3.
    if let Some(msg) = d.strip_prefix("unsupported: ") {
        eprintln!("unsupported: {}", msg.split('@').next().unwrap_or(msg));
        return ExitCode::from(3);
    }
    eprint!("{}", coby::diagnostics::for_stderr(&coby::render_located(&d, phase, &map)));
    ExitCode::FAILURE
}

// `--explore`: see `main`.
fn explore(runs: u64, rest: &[OsString]) -> ExitCode {
    use std::io::{Read, Write};
    // The input, read once for every run -- unless it is a terminal, which
    // gives none.
    let mut input = Vec::new();
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        let _ = std::io::stdin().read_to_end(&mut input);
    }
    let me = match env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("coby --explore: {}", e);
            return ExitCode::from(2);
        }
    };
    // (status, stdout, stderr) -> (count, first seed)
    let mut seen: Vec<((Option<i32>, Vec<u8>, Vec<u8>), u64, u64)> = Vec::new();
    for seed in 1..=runs {
        let child = std::process::Command::new(&me)
            .arg("--schedule-seed")
            .arg(seed.to_string())
            .args(rest)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                eprintln!("coby --explore: {}", e);
                return ExitCode::from(2);
            }
        };
        if let Some(mut w) = child.stdin.take() {
            let _ = w.write_all(&input);
        }
        let out = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => {
                eprintln!("coby --explore: {}", e);
                return ExitCode::from(2);
            }
        };
        let key = (out.status.code(), out.stdout, out.stderr);
        match seen.iter_mut().find(|(k, _, _)| *k == key) {
            Some(e) => e.1 += 1,
            None => seen.push((key, 1, seed)),
        }
    }
    let file = rest[0].to_string_lossy();
    println!("explored {} schedules of {}: {} distinct outcome{}", runs, file, seen.len(), if seen.len() == 1 { "" } else { "s" });
    for ((status, so, se), n, seed) in &seen {
        let st = status.map_or("killed".to_string(), |c| format!("exit {}", c));
        println!("\n{} run{}, {} -- replay: coby --schedule-seed {} {}", n, if *n == 1 { "" } else { "s" }, st, seed, file);
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
