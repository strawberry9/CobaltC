use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
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
    "  coby --about            show this\n",
    "  cobc --about            the compiler\n",
);

fn main() -> ExitCode {
    // Bytes, not text: an argument need not be UTF-8 (`rule.stdlib.args`).
    let args: Vec<OsString> = env::args_os().collect();
    if args.len() == 2 && args[1] == "--about" {
        print!("{}{}", COBY_ART, ABOUT);
        return ExitCode::SUCCESS;
    }
    if args.len() < 2 {
        eprintln!("usage: coby <file.cb> [arg...]");
        eprintln!("       coby --about");
        return ExitCode::from(2);
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
    let runner = std::thread::Builder::new()
        .stack_size(if cfg!(target_pointer_width = "64") { 4 << 30 } else { 256 << 20 })
        .spawn(move || coby::run_program_with_args(&src, Some(&owned_path), prog_args))
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
    eprint!("{}", coby::render_located(&d, phase, &map));
    ExitCode::FAILURE
}
