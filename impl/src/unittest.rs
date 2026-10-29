// D-0230: `--unittest`. The program's `unittest` blocks are the hidden,
// exported functions `__unittest_N` of their modules (`parser.rs`,
// `parse_unittest`), checked like every other function and never run by
// the program itself. Under `--unittest` both tools add a `main` of their
// own, written here in CobaltC, which runs each selected test in a thread
// of its own and reports it (`spec/18` `rule.test.run`). The program's
// own `main`, if it has one, is renamed `__unittest_main` and not run.
// Because the runner is CobaltC, `coby` and `cobc` report a run alike by
// construction.

use crate::ast::UnitTest;
use crate::loader::SourceMap;
use std::sync::Mutex;

#[derive(Clone, Debug, Default)]
pub struct Mode {
    // `--unittest-filter TEXT`: only the tests whose name contains TEXT.
    pub filter: Option<String>,
    // `--unittest-std`: `std`'s own tests, in place of the program's.
    pub std_tests: bool,
}

static MODE: Mutex<Option<Mode>> = Mutex::new(None);

// The tools call this before `analyze`, from their command line.
pub fn set_mode(m: Option<Mode>) {
    *MODE.lock().unwrap() = m;
}

pub fn mode() -> Option<Mode> {
    MODE.lock().unwrap().clone()
}

// The prefix of a refusal the runner makes before anything runs (a filter
// that matches no test): shown as it is, exit status 1 (`render_located`).
pub const REFUSAL: &str = "unittest: ";

// A test's place, as the report names it: `file.cb:LINE`.
fn place(t: &UnitTest, map: &SourceMap) -> String {
    let user = t.line.checked_sub(crate::prelude::line_count()).filter(|&n| n >= 1);
    match user.and_then(|n| map.resolve(n)) {
        Some((f, l)) => format!("{}:{}", f, l),
        None => match crate::prelude::std_location(t.line) {
            Some((f, l)) => format!("<std>/{}:{}", f, l),
            None => format!("line {}", t.line),
        },
    }
}

// A text as a CobaltC `str` literal.
fn literal(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// The runner's source: a `main` that runs the selected tests in order
// (the order of the program's text, a file-backed module's tests where the
// module is declared), each as `try_join(spawn(test))`, and reports
//
//     test FILE:LINE NAME ... ok
//     test FILE:LINE ... FAILED
//     <the fault's report, as the program would have printed it>
//     N tests: P passed, F failed[, K filtered out]
//
// on standard output; its exit status is 1 when a test failed.
pub fn runner(tests: &[UnitTest], mode: &Mode, map: &SourceMap) -> Result<String, String> {
    let std_line = crate::prelude::line_count();
    let mine: Vec<&UnitTest> = tests.iter().filter(|t| (t.line <= std_line) == mode.std_tests).collect();
    let chosen: Vec<&UnitTest> = match &mode.filter {
        Some(f) => mine.iter().copied().filter(|t| t.name.as_deref().map_or(false, |n| n.contains(f.as_str()))).collect(),
        None => mine.clone(),
    };
    if let Some(f) = &mode.filter {
        if chosen.is_empty() {
            return Err(format!("{}no test's name contains {:?} ({} test{} in all)", REFUSAL, f, mine.len(), if mine.len() == 1 { "" } else { "s" }));
        }
    }
    let mut s = String::from("fn main() : u8\n{\n    usize passed = 0;\n    usize failed = 0;\n");
    for t in &chosen {
        let label = match &t.name {
            Some(n) => format!("test {} {} ... ", place(t, map), n),
            None => format!("test {} ... ", place(t, map)),
        };
        let path = if t.module.is_empty() { t.fn_name.clone() } else { format!("{}::{}", t.module.join("::"), t.fn_name) };
        s.push_str(&format!("    std::printf(\"%v\", {});\n", literal(&label)));
        s.push_str(&format!("    match (std::try_join(spawn({})))\n    {{\n", path));
        s.push_str("        Ok(_)  : { std::printf(\"ok\\n\"); passed += 1; },\n");
        s.push_str("        Err(f) : { std::printf(\"FAILED\\n%v\", &std::ThreadFailure::text(&f)); failed += 1; },\n");
        s.push_str("    }\n");
    }
    let filtered = match &mode.filter {
        Some(_) => format!(", {} filtered out", mine.len() - chosen.len()),
        None => String::new(),
    };
    s.push_str(&format!("    std::printf(\"%v test%v: %v passed, %v failed%v\\n\", passed + failed, {}, passed, failed, {});\n", literal(if chosen.len() == 1 { "" } else { "s" }), literal(&filtered)));
    s.push_str("    if (failed > 0) { 1 } else { 0 }\n}\n");
    Ok(s)
}
