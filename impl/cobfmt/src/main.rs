// cobfmt — CobaltC's code formatter (`private/cobfmt-proposal.md`).
//
// Rewrites `.cb` files into the house style: layout only, never a token.
// Every result is checked before it is written (`verify`): the same tokens,
// the same comments, and it still parses. Rust's standard library only, so
// it builds and runs on Windows as on Linux.

mod diff;
mod files;
mod format;
mod verify;
mod width;

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
cobfmt — formats CobaltC source files

usage: cobfmt [OPTIONS] PATH...
       cobfmt [OPTIONS] -          (standard input to standard output)

A PATH is a .cb file, or a directory searched for .cb files (hidden
directories and target/ are skipped). Files are rewritten in place.

options:
  --check       write nothing; list the files that would change, exit 1 if any
  --diff        write nothing; show what would change, as a unified diff
  --stdout      write the result to standard output instead (one file only)
  --backup      keep each changed file's previous contents beside it, as
                NAME.cb.format.bak (then .format.bak.1, .format.bak.2, ...)
  --width N     break lines longer than N characters (by default, never)
  --quiet, -q   print nothing but errors
  --version, -V print the version
  --help, -h    print this text

exit status: 0 done, 1 --check or --diff found changes, 2 a usage error or
a file that cannot be read or does not parse, 3 an internal error (the
file is left as it was; please report it).

example: cobfmt --backup src/        format every .cb file under src/
";

struct Opts {
    check: bool,
    diff: bool,
    stdout: bool,
    backup: bool,
    quiet: bool,
    width: Option<usize>,
    paths: Vec<String>,
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("cobfmt: {}", msg);
    eprintln!("run `cobfmt --help` for the options");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    // The parser recurses on nesting, as coby's and cobc's do: the same
    // large stack (reserved, not committed) keeps a deeply nested file
    // from crashing the formatter.
    let stack = if cfg!(target_pointer_width = "64") { 1usize << 30 } else { 256 << 20 };
    std::thread::Builder::new()
        .stack_size(stack)
        .spawn(real_main)
        .expect("start the formatter thread")
        .join()
        .unwrap_or_else(|e| std::panic::resume_unwind(e))
}

fn real_main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprint!("{}", USAGE);
        return ExitCode::from(2);
    }
    let mut o = Opts { check: false, diff: false, stdout: false, backup: false, quiet: false, width: None, paths: Vec::new() };
    let mut i = 0;
    let mut only_paths = false;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if only_paths || a == "-" || !a.starts_with('-') {
            o.paths.push(a.clone());
            continue;
        }
        match a.as_str() {
            "--" => only_paths = true,
            "--help" | "-h" => {
                print!("{}", USAGE);
                return ExitCode::SUCCESS;
            }
            "--version" | "-V" => {
                println!("cobfmt {} (CobaltC, the formatting of the guide, showcases and examples)", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--check" => o.check = true,
            "--diff" => o.diff = true,
            "--stdout" => o.stdout = true,
            "--backup" => o.backup = true,
            "--quiet" | "-q" => o.quiet = true,
            "--width" => {
                let Some(v) = args.get(i) else { return usage_error("--width needs a number") };
                i += 1;
                match v.parse::<usize>() {
                    Ok(n) if n >= 20 => o.width = Some(n),
                    _ => return usage_error(&format!("--width wants a number of at least 20, not `{}`", v)),
                }
            }
            _ if a.starts_with("--width=") => match a["--width=".len()..].parse::<usize>() {
                Ok(n) if n >= 20 => o.width = Some(n),
                _ => return usage_error(&format!("--width wants a number of at least 20, not `{}`", &a["--width=".len()..])),
            },
            _ => return usage_error(&format!("unknown option `{}`", a)),
        }
    }
    if o.paths.is_empty() {
        return usage_error("no file given (use `-` for standard input)");
    }
    if o.check && o.diff {
        return usage_error("--check and --diff cannot be used together");
    }
    if o.stdout && (o.check || o.diff || o.backup) {
        return usage_error("--stdout cannot be combined with --check, --diff or --backup");
    }
    if o.paths.iter().any(|p| p == "-") {
        if o.paths.len() > 1 {
            return usage_error("`-` (standard input) must be the only path");
        }
        if o.backup {
            return usage_error("--backup has no file to keep when reading standard input");
        }
        return run_stdin(&o);
    }
    // The files: each path, directories searched.
    let mut list: Vec<PathBuf> = Vec::new();
    for p in &o.paths {
        let path = Path::new(p);
        if path.is_dir() {
            files::collect(path, &mut list);
        } else if path.exists() {
            list.push(path.to_path_buf());
        } else {
            eprintln!("cobfmt: {}: no such file or directory", p);
            return ExitCode::from(2);
        }
    }
    if o.stdout && list.len() != 1 {
        return usage_error("--stdout formats exactly one file");
    }
    let mut worst = 0u8;
    let mut changed = 0usize;
    for f in &list {
        let st = run_file(&o, f, &mut changed);
        worst = worst.max(st);
    }
    if !o.quiet && !o.stdout && !o.check && !o.diff && list.len() > 1 {
        eprintln!("cobfmt: {} file(s), {} changed", list.len(), changed);
    }
    ExitCode::from(worst)
}

// Formats `src`; the result, or the exit status and message on failure.
// Whether `src` parses as coby parses it: its file-backed modules read
// (relative to `origin`), after the prelude, whose type names the parser
// needs (`Vec<String> v` is a declaration because `Vec` is a type).
pub fn parses(src: &str, origin: Option<&Path>) -> Result<(), String> {
    let ex = coby::loader::expand(src.trim_start_matches('\u{feff}'), origin);
    let pre = coby::prelude::source().lines().count() + 1;
    let place = |line: usize| -> String {
        let mapped = if ex.error.is_some() { None } else { line.checked_sub(pre).and_then(|l| ex.map.resolve(l)) };
        match mapped {
            Some((f, l)) => format!("{}:{}", f, l),
            None => format!("line {}", line.saturating_sub(pre)),
        }
    };
    // A module file that cannot be read (missing, duplicated, a cycle) is
    // the other files' problem, not this one's layout: the file is parsed
    // on its own instead.
    let text = if ex.error.is_some() { src.trim_start_matches('\u{feff}').to_string() } else { ex.text.clone() };
    let combined = format!("{}\n{}", coby::prelude::source(), text);
    match coby::parser::parse(&combined) {
        Ok(_) => Ok(()),
        // A program the parser builds but rejects for a reason other than
        // its syntax (a duplicate local, a constant that is not constant)
        // has a structure, and its layout can be formatted.
        Err(e) if e.starts_with("diag.") && !e.starts_with("diag.syntax-error") => Ok(()),
        Err(e) => Err(describe_parse_error(&e, &place)),
    }
}

fn format_checked(src: &str, width: Option<usize>, name: &str, origin: Option<&Path>) -> Result<String, (u8, String)> {
    // A file that does not parse is not formatted: its structure is unknown.
    if let Err(e) = parses(src, origin) {
        return Err((2, format!("does not parse: {}", e)));
    }
    let out = match format::format(src) {
        Ok(f) => f.text,
        Err(format::FmtError::Lex(m)) => return Err((2, format!("{}: {}", name, m))),
    };
    let out = match width {
        Some(w) => width::break_long_lines(&out, w),
        None => out,
    };
    if let Err(why) = verify::same_program(src, &out, width.is_some(), origin) {
        return Err((3, format!("{}: internal error, file left as it was: {} (please report it)", name, why)));
    }
    if cfg!(debug_assertions) {
        // Idempotence: formatting the result changes nothing.
        if let Ok(again) = format::format(&out) {
            let again = match width {
                Some(w) => width::break_long_lines(&again.text, w),
                None => again.text,
            };
            if again != out {
                return Err((3, format!("{}: internal error: formatting is not idempotent here (please report it)", name)));
            }
        }
    }
    Ok(out)
}

// `parse error at 3:13: expected …` as coby reports it: `3: expected … (column 13)`
// (`NAME:` is prefixed by the caller).
fn describe_parse_error(e: &str, place: &dyn Fn(usize) -> String) -> String {
    for prefix in ["parse error at ", "lex error at "] {
        if let Some(rest) = e.strip_prefix(prefix) {
            if let Some((pos, msg)) = rest.split_once(": ") {
                if let Some((l, c)) = pos.split_once(':') {
                    if let Ok(l) = l.parse::<usize>() {
                        return format!("{}: {} (column {})", place(l), msg, c);
                    }
                }
            }
        }
    }
    let (id, line) = coby::diagnostics::split_location(e);
    match line {
        Some(l) => format!("{}: {}", place(l), id),
        None => id.to_string(),
    }
}

fn run_stdin(o: &Opts) -> ExitCode {
    let mut src = String::new();
    if std::io::stdin().read_to_string(&mut src).is_err() {
        eprintln!("cobfmt: standard input is not readable UTF-8 text");
        return ExitCode::from(2);
    }
    match format_checked(&src, o.width, "<stdin>", None) {
        Ok(out) => {
            if o.check {
                if out != src {
                    if !o.quiet {
                        println!("<stdin>");
                    }
                    return ExitCode::from(1);
                }
                return ExitCode::SUCCESS;
            }
            if o.diff {
                if out != src {
                    print!("{}", diff::unified(&src, &out, "<stdin>"));
                    return ExitCode::from(1);
                }
                return ExitCode::SUCCESS;
            }
            let _ = std::io::stdout().write_all(out.as_bytes());
            ExitCode::SUCCESS
        }
        Err((code, msg)) => {
            eprintln!("cobfmt: {}", msg);
            ExitCode::from(code)
        }
    }
}

fn run_file(o: &Opts, path: &Path, changed: &mut usize) -> u8 {
    let name = path.display().to_string();
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cobfmt: {}: cannot read: {}", name, e);
            return 2;
        }
    };
    let src = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("cobfmt: {}: not UTF-8 (a CobaltC source file is UTF-8)", name);
            return 2;
        }
    };
    let out = match format_checked(&src, o.width, &name, Some(path)) {
        Ok(out) => out,
        Err((code, msg)) => {
            eprintln!("cobfmt: {}", msg);
            return code;
        }
    };
    if o.stdout {
        let _ = std::io::stdout().write_all(out.as_bytes());
        return 0;
    }
    if out == src {
        return 0;
    }
    *changed += 1;
    if o.check {
        if !o.quiet {
            println!("{}", name);
        }
        return 1;
    }
    if o.diff {
        print!("{}", diff::unified(&src, &out, &name));
        return 1;
    }
    if o.backup {
        match files::backup(path, src.as_bytes()) {
            Ok(b) => {
                if !o.quiet {
                    eprintln!("cobfmt: {}: previous contents kept as {}", name, b.display());
                }
            }
            Err(e) => {
                eprintln!("cobfmt: {}: cannot write the backup ({}); file left as it was", name, e);
                return 2;
            }
        }
    }
    if let Err(e) = files::replace(path, out.as_bytes()) {
        eprintln!("cobfmt: {}: cannot write: {}", name, e);
        return 2;
    }
    if !o.quiet && o.backup == false {
        eprintln!("cobfmt: formatted {}", name);
    }
    0
}
