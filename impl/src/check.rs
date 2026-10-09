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
