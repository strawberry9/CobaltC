// `coby --check` and `cobc --check`: the static pass alone -- the loader,
// the parser, name resolution and every static check -- over each file
// named and each `.cb` file in each directory named; nothing is run or
// built. A rejected program prints exactly the diagnostic a run (or a
// build) would print. Standard library only: the same on Windows and
// Linux.
//
// A directory holds module files too (`module m "./m.cb";` splices one
// into the program that declares it, spec/17 §5), which are not programs
// on their own: a file found by searching a directory that another file
// being checked loads as a module is checked as part of that program,
// not alone. A file named on the command line is always checked as a
// program.

use crate::Analysis;
use std::path::{Path, PathBuf};

/// The `.cb` files under `dir`, sorted; hidden directories and `target/`
/// skipped.
pub fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if p.is_dir() {
            if name.starts_with('.') || name == "target" {
                continue;
            }
            collect(&p, out);
        } else if name.ends_with(".cb") {
            out.push(p);
        }
    }
}

/// Checks `paths`; `tool` names the command in its own messages, and
/// `extra` checks an accepted program further (`cobc`: the files its
/// `extern "…";` declarations name), its error printed as `TOOL: error`.
/// The exit status: 0 every program accepted; 1 one was rejected
/// statically; 2 a path that does not exist, a file that cannot be read,
/// or `extra` failed. Every file is checked whatever an earlier one gave.
pub fn run(paths: &[PathBuf], tool: &str, extra: &dyn Fn(&Analysis) -> Result<(), String>) -> u8 {
    let mut worst = 0u8;
    // Each file, and whether a directory search found it.
    let mut files: Vec<(PathBuf, bool)> = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut found = Vec::new();
            collect(p, &mut found);
            files.extend(found.into_iter().map(|f| (f, true)));
        } else if p.exists() {
            files.push((p.clone(), false));
        } else {
            eprintln!("{}: {}: no such file or directory", tool, p.display());
            worst = 2;
        }
    }
    // A file named twice (or found twice) is checked once.
    let mut seen: Vec<PathBuf> = Vec::new();
    files.retain(|(f, _)| {
        let canon = std::fs::canonicalize(f).unwrap_or_else(|_| f.clone());
        let fresh = !seen.contains(&canon);
        seen.push(canon);
        fresh
    });
    // Read every file once; the module files the found ones load (the
    // loader alone: no program is checked yet).
    let sources: Vec<Result<String, String>> = files.iter().map(|(f, _)| std::fs::read_to_string(f).map_err(|e| e.to_string())).collect();
    let mut modules: Vec<PathBuf> = Vec::new();
    for ((f, _), src) in files.iter().zip(&sources) {
        if let Ok(src) = src {
            modules.extend(crate::loader::expand(src, Some(f)).loaded);
        }
    }
    let many = files.len() > 1;
    for ((f, found), src) in files.iter().zip(sources) {
        if *found && std::fs::canonicalize(f).map_or(false, |c| modules.contains(&c)) {
            continue;
        }
        let src = match src {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: error reading {}: {}", tool, f.display(), e);
                worst = 2;
                continue;
            }
        };
        let a = crate::analyze(&src, Some(f));
        if let Some(d) = &a.error {
            if many {
                eprintln!("{}: rejected", f.display());
            }
            eprint!("{}", crate::diagnostics::for_stderr(&crate::render_located(d, "static", &a.map)));
            worst = worst.max(1);
        } else if let Err(e) = extra(&a) {
            eprintln!("{}: {}", tool, e);
            worst = 2;
        }
    }
    worst
}

/// Whether `src` parses, as `cobfmt` requires before it formats a file and
/// of what it writes: its file-backed modules read (relative to `origin`),
/// after the prelude, whose type names the parser needs (`Vec<String> v`
/// is a declaration because `Vec` is a type). A program the parser builds
/// but rejects for a reason other than its syntax (a duplicate local, a
/// constant that is not constant) has a structure, and parses. The error
/// is `PLACE: MESSAGE (column C)`, PLACE a `FILE:LINE` or `line N`.
pub fn syntax(src: &str, origin: Option<&Path>) -> Result<(), String> {
    let ex = crate::loader::expand(src.trim_start_matches('\u{feff}'), origin);
    let pre = crate::prelude::source().lines().count() + 1;
    let place = |line: usize| -> String {
        let mapped = if ex.error.is_some() { None } else { line.checked_sub(pre).and_then(|l| ex.map.resolve(l)) };
        match mapped {
            Some((f, l)) => format!("{}:{}", f, l),
            None => format!("line {}", line.saturating_sub(pre)),
        }
    };
    // A module file that cannot be read (missing, duplicated, a cycle) is
    // the other files' problem, not this one's syntax: the file is parsed
    // on its own instead, its file-backed modules empty.
    let text = if ex.error.is_some() { crate::loader::stub_modules(src.trim_start_matches('\u{feff}')) } else { ex.text.clone() };
    let combined = format!("{}\n{}", crate::prelude::source(), text);
    match crate::parser::parse(&combined) {
        Ok(_) => Ok(()),
        Err(e) if e.starts_with("diag.") && !e.starts_with("diag.syntax-error") => Ok(()),
        Err(e) => Err(describe_parse_error(&e, &place)),
    }
}

// `parse error at 3:13: expected …` as the parser reports it: `3: expected … (column 13)`.
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
    let (id, line) = crate::diagnostics::split_location(e);
    match line {
        Some(l) => format!("{}: {}", place(l), id),
        None => id.to_string(),
    }
}

/// `coby --check-syntax` and `cobc --check-syntax` (`args` after the
/// option): `[--origin PATH] FILE` or `[--origin PATH] -`, the text read
/// from FILE or standard input, its modules found relative to PATH (by
/// default FILE; none for standard input). Whether it parses as `syntax`
/// decides: exit 0 it does; 1 it does not, the error on standard error;
/// 2 a usage error or text that cannot be read. For a formatter written
/// in CobaltC (`impl/selfhost/cobfmt`), which has no parser of its own.
pub fn run_syntax(args: &[String], tool: &str) -> u8 {
    let usage = || {
        eprintln!("usage: {} --check-syntax [--origin PATH] (FILE | -)", tool);
        2
    };
    let (origin, rest) = match args {
        [o, p, rest @ ..] if o == "--origin" => (Some(PathBuf::from(p)), rest),
        _ => (None, args),
    };
    let [file] = rest else { return usage() };
    let src = if file == "-" {
        let mut s = String::new();
        if std::io::Read::read_to_string(&mut std::io::stdin(), &mut s).is_err() {
            eprintln!("{}: standard input is not readable UTF-8 text", tool);
            return 2;
        }
        s
    } else {
        match std::fs::read(file).map(String::from_utf8) {
            Ok(Ok(s)) => s,
            Ok(Err(_)) => {
                eprintln!("{}: {}: not UTF-8 (a CobaltC source file is UTF-8)", tool, file);
                return 2;
            }
            Err(e) => {
                eprintln!("{}: {}: cannot read: {}", tool, file, e);
                return 2;
            }
        }
    };
    let origin = origin.or_else(|| if file == "-" { None } else { Some(PathBuf::from(file)) });
    match syntax(&src, origin.as_deref()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

/// How far `run_dump` takes a program before printing it: the parse
/// (`--dump-ast`), name resolution (`--dump-resolved`, CHG-0352), the
/// constants folded and the text literals viewed (`--dump-folded`,
/// CHG-0354), which is what the type checker reads, or the whole static
/// pass, whose result alone is printed, `ok` (`--dump-checked`, CHG-0355).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DumpStage {
    Parsed,
    Resolved,
    Folded,
    Checked,
}

/// `coby --dump-ast [--origin PATH] (FILE | -)` (CHG-0350): the program's
/// own part of its parse, in `astdump`'s text, for comparing the parser
/// written in CobaltC with this one. A text that does not parse prints the
/// error `syntax` gives (exit 1); one the parser builds but rejects for
/// another reason prints that diagnostic, `PLACE: ID` (exit 1).
pub fn run_dump(args: &[String], tool: &str, stage: DumpStage) -> u8 {
    let resolved = stage != DumpStage::Parsed;
    let option = match stage {
        DumpStage::Parsed => "--dump-ast",
        DumpStage::Resolved => "--dump-resolved",
        DumpStage::Folded => "--dump-folded",
        DumpStage::Checked => "--dump-checked",
    };
    let (origin, rest) = match args {
        [o, p, rest @ ..] if o == "--origin" => (Some(PathBuf::from(p)), rest),
        _ => (None, args),
    };
    let [file] = rest else {
        eprintln!("usage: {} {} [--origin PATH] (FILE | -)", tool, option);
        return 2;
    };
    let src = if file == "-" {
        let mut s = String::new();
        if std::io::Read::read_to_string(&mut std::io::stdin(), &mut s).is_err() {
            eprintln!("{}: standard input is not readable UTF-8 text", tool);
            return 2;
        }
        s
    } else {
        match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: {}: cannot read: {}", tool, file, e);
                return 2;
            }
        }
    };
    let origin = origin.or_else(|| if file == "-" { None } else { Some(PathBuf::from(file)) });
    let origin = origin.as_deref();
    let ex = crate::loader::expand(src.trim_start_matches('\u{feff}'), origin);
    let pre = crate::prelude::source().lines().count() + 1;
    let place = |line: usize| -> String {
        let mapped = if ex.error.is_some() { None } else { line.checked_sub(pre).and_then(|l| ex.map.resolve(l)) };
        match mapped {
            Some((f, l)) => format!("{}:{}", f, l),
            None => format!("line {}", line.saturating_sub(pre)),
        }
    };
    // A module file that cannot be loaded is the program's error once
    // names are resolved, as in a run: its line is one of the expansion's.
    if let (true, Some(err)) = (resolved, &ex.error) {
        let at = |line: usize| match ex.map.resolve(line) {
            Some((f, l)) => format!("{}:{}", f, l),
            None => format!("line {}", line),
        };
        eprintln!("{}", describe_diagnostic(err, &at));
        return 1;
    }
    let text = if ex.error.is_some() { crate::loader::stub_modules(src.trim_start_matches('\u{feff}')) } else { ex.text.clone() };
    let combined = format!("{}\n{}", crate::prelude::source(), text);
    let state = match crate::parser::prelude_state(crate::prelude::source()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: the prelude does not parse: {}", tool, e);
            return 2;
        }
    };
    if stage == DumpStage::Checked {
        return match crate::static_check(&text) {
            Ok(()) => {
                println!("ok");
                0
            }
            Err(e) if e.starts_with("diag.") => {
                eprintln!("{}", describe_diagnostic(&e, &place));
                1
            }
            Err(e) => {
                eprintln!("{}", describe_parse_error(&e, &place));
                1
            }
        };
    }
    let parsed = crate::parser::parse(&combined).and_then(|mut p| {
        if resolved {
            crate::resolve_front(&mut p)?;
        }
        if stage == DumpStage::Folded {
            crate::consts::process(&mut p)?;
            crate::views::process(&mut p);
        }
        Ok(p)
    });
    match parsed {
        Ok(p) => {
            print!("{}", crate::astdump::program(&p, 1, state.tests, pre));
            0
        }
        Err(e) if resolved && e.starts_with("diag.") => {
            eprintln!("{}", describe_diagnostic(&e, &place));
            1
        }
        Err(e) => {
            eprintln!("{}", describe_parse_error(&e, &place));
            1
        }
    }
}

// `diag.x\u{1}message@N` as `PLACE: diag.x: message`, the message when
// there is one (`--dump-resolved`, which compares them).
fn describe_diagnostic(e: &str, place: &dyn Fn(usize) -> String) -> String {
    let (id, line) = crate::diagnostics::split_location(e);
    let body = e.rsplit_once('@').filter(|_| line.is_some()).map_or(e, |(b, _)| b);
    let shown = match body.split_once(crate::diagnostics::DETAIL_SEP) {
        Some((_, detail)) => format!("{}: {}", id, detail),
        None => id.to_string(),
    };
    match line {
        Some(l) => format!("{}: {}", place(l), shown),
        None => shown,
    }
}

/// `coby --prelude-summary` (CHG-0350): `impl/selfhost/parser/prelude.cb`,
/// what the parser written in CobaltC needs of the prelude: the skeleton
/// the pre-scans read and the counters parsing it leaves.
pub fn run_prelude_summary(tool: &str) -> u8 {
    let src = crate::prelude::source();
    let (skel, decls, st, bodies) = match (crate::parser::prelude_skeleton(src), crate::parser::prelude_declarations(src), crate::parser::prelude_state(src), crate::parser::prelude_bodies(src)) {
        (Ok(k), Ok(d), Ok(s), Ok(b)) => (k, d, s, b),
        (Err(e), _, _, _) | (_, Err(e), _, _) | (_, _, Err(e), _) | (_, _, _, Err(e)) => {
            eprintln!("{}: {}", tool, e);
            return 2;
        }
    };
    // Each body's record, `key module line col fresh local_consts offset length`, the
    // offset and length in bytes of `PRELUDE_BODIES`, their texts end to end.
    let (mut index, mut texts) = (String::new(), String::new());
    for b in &bodies {
        index.push_str(&format!("{} {} {} {} {} {} {} {}\n", b.key, b.module, b.line, b.col, b.fresh, b.local_consts, texts.len(), b.text.len()));
        texts.push_str(&b.text);
    }
    let mut intrinsics = String::from(" ");
    for n in crate::typecheck::INTRINSIC_NAMES.iter().filter(|n| !n.contains("::") && !n.starts_with('$')) {
        intrinsics.push_str(n);
        intrinsics.push(' ');
    }
    // The diagnostics `fault` may name: phase `dynamic` or `both`.
    let mut dynamic: Vec<String> = crate::diagnostics::registry().into_iter().filter(|(_, i)| matches!(i.phase.as_deref(), Some("dynamic") | Some("both"))).map(|(id, _)| id).collect();
    dynamic.sort();
    let dynamic = format!(" {} ", dynamic.join(" "));
    let mut all_intrinsics = String::from(" ");
    for n in crate::typecheck::INTRINSIC_NAMES.iter() {
        all_intrinsics.push_str(n);
        all_intrinsics.push(' ');
    }
    let float_fns: Vec<&str> = crate::typecheck::FLOAT_FNS.iter().chain(crate::typecheck::FLOAT2_FNS).map(|f| f.trim_start_matches("std::")).collect();
    let float_fns = float_fns.join(" ");
    // `text` as a CobaltC string literal, in pieces of about 100 bytes,
    // each ending at a character's end.
    let literal = |text: &str| -> String {
        let mut lit = String::new();
        let mut piece = 0usize;
        for c in text.chars() {
            if piece == 0 {
                lit.push_str("    \"");
            }
            match c {
                '"' => lit.push_str("\\\""),
                '\\' => lit.push_str("\\\\"),
                '\n' => lit.push_str("\\n"),
                '\t' => lit.push_str("\\t"),
                '\r' => lit.push_str("\\r"),
                _ => lit.push(c),
            }
            piece += c.len_utf8();
            if piece >= 100 {
                lit.push_str("\"\n");
                piece = 0;
            }
        }
        if piece > 0 {
            lit.push_str("\"\n");
        }
        lit.trim_end_matches('\n').to_string()
    };
    print!(
        "// Generated by `coby --prelude-summary` (CHG-0350, CHG-0352); do not edit.\n\
         // What the front end written in CobaltC needs of the prelude (`std`): the\n\
         // tokens the parser's pre-scans read (`prelude_skeleton`), the counters\n\
         // parsing it leaves, from which a program's hidden names continue, and its\n\
         // declarations, every function's body empty (`prelude_declarations`), for\n\
         // name resolution. `impl/selfhost/compare_parser.sh` checks it is current.\n\
         \n\
         export const str PRELUDE_SKELETON =\n{};\n\
         \n\
         export const usize PRELUDE_FRESH = {};\n\
         export const usize PRELUDE_LOCAL_CONSTS = {};\n\
         export const usize PRELUDE_TESTS = {};\n\
         \n\
         export const str PRELUDE_DECLS =\n{};\n\
         \n\
         // The intrinsics' unqualified names (`modres::is_intrinsic_name`), each\n\
         // between spaces, and the floating-point functions of `std::math`.\n\
         export const str PRELUDE_INTRINSICS =\n{};\n\
         export const str PRELUDE_FLOAT_FNS =\n{};\n\
         // Every intrinsic's name, qualified and `$` names too\n\
         // (`typecheck::INTRINSIC_NAMES`), each between spaces.\n\
         export const str PRELUDE_ALL_INTRINSICS =\n{};\n\
         // The diagnostics that happen at run time (phase `dynamic` or `both`\n\
         // in spec/registry/diagnostics.md), each between spaces, for `fault`.\n\
         export const str PRELUDE_DYNAMIC_DIAGS =\n{};\n\
         \n\
         // Every function of `std` with a body, for a parse of its own when the\n\
         // type checker first reaches it (`prelude_bodies`, CHG-0355): one record a\n\
         // line, `key module line col fresh local_consts offset length`, into the\n\
         // texts.\n\
         export const str PRELUDE_BODY_INDEX =\n{};\n\
         export const str PRELUDE_BODIES =\n{};\n",
        literal(&skel),
        st.fresh,
        st.local_consts,
        st.tests,
        literal(&decls),
        literal(&intrinsics),
        literal(&float_fns),
        literal(&all_intrinsics),
        literal(&dynamic),
        literal(&index),
        literal(&texts)
    );
    0
}
