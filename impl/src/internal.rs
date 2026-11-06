// WI-15: an internal error. A panic anywhere in a tool is a bug in the
// tool, never the program's fault (a program's faults are diagnostics),
// so it is not shown as Rust shows a panic: it is one line on stderr,
//
//     internal error in cobc: <what> (<the tool's file>:<line>); please report it with the program
//
// and exit status 4 (3 is `unsupported`), with Rust's backtrace after it
// only when RUST_BACKTRACE asks for one. Both `coby` and `cobc` install
// this first thing in `main`; the threads they start inherit it.

use std::sync::Mutex;

// Held by the first thread that reports, until the process exits: a
// second panicking thread waits here, so one error is shown.
static REPORTING: Mutex<()> = Mutex::new(());

pub fn install_hook(tool: &'static str) {
    std::panic::set_hook(Box::new(move |info| {
        let _held = REPORTING.lock().unwrap_or_else(|e| e.into_inner());
        let payload = info.payload();
        let what = match (payload.downcast_ref::<&str>(), payload.downcast_ref::<String>()) {
            (Some(s), _) => s.to_string(),
            (None, Some(s)) => s.clone(),
            (None, None) => "a panic with no message".to_string(),
        };
        let what = what.split_whitespace().collect::<Vec<_>>().join(" ");
        // The tool's own output could not be written (`print!`): not a bug
        // in the tool. A reader that stopped early (`cobc --explore … |
        // head`; EPIPE, or ERROR_NO_DATA on Windows) ends it quietly, as
        // it ends a Unix tool; any other failure (a full disk) is said
        // plainly.
        if let Some(rest) = what.strip_prefix("failed printing to stdout: ").or_else(|| what.strip_prefix("failed printing to stderr: ")) {
            if rest.contains("(os error 32)") || rest.contains("(os error 232)") {
                std::process::exit(1);
            }
            eprintln!("{}: cannot write its output: {}", tool, rest);
            std::process::exit(1);
        }
        let at = match info.location() {
            Some(l) => format!(" ({}:{})", l.file().replace('\\', "/"), l.line()),
            None => String::new(),
        };
        eprintln!("internal error in {}: {}{}; please report it with the program", tool, what, at);
        if std::env::var_os("RUST_BACKTRACE").map_or(false, |v| v != "0") {
            eprintln!("{}", std::backtrace::Backtrace::force_capture());
        }
        std::process::exit(4);
    }));
}

// The variable that makes a tool panic on purpose (the driver tests of
// the report above): checked once, in the thread that does the work.
pub const TEST_VARIABLE: &str = "COBALTC_TEST_INTERNAL_ERROR";

#[track_caller]
pub fn test_point() {
    if std::env::var_os(TEST_VARIABLE).is_some() {
        panic!("{} is set", TEST_VARIABLE);
    }
}
