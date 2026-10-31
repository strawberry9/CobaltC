//! `--certificate FILE` (D-0299): what the static pass decided before the
//! run, body by body, written for a reader and for an independent checker.
//!
//! A compiled program keeps no record of the references of a function the
//! static pass *decided* (`rule.control.frame-references`, `spec/14` §6,
//! D-0284, D-0292): every access such a function makes through a reference
//! was shown not to conflict, so no check is made there. That verdict is the
//! one place where the tools trust a static argument and the run checks
//! nothing; the certificate makes it visible. For each body the static pass
//! checked it says `decided`, or `left` with every conflict the body leaves
//! to the run (the reason a record is kept), the checks the run makes
//! because only values decide them (point checks), the references whose
//! origin or data the pass does not follow (tier D), and, for the program,
//! the compiler's verdict: `decided` (no record of references at all), or
//! why paths were kept.
//!
//! The static pass appends raw records to the file as it checks each body
//! (`typecheck::tier_report::end`, under `COBALTC_CERTIFICATE`); `finish`
//! rewrites the file in its final form once the program's verdict is known:
//! a header, the verdict, and the bodies sorted by name with their lines
//! mapped to the files they came from. `cobmodel certify CERT FILE` checks a
//! fragment program's certificate against every interleaving of its threads
//! (`impl/model`): an alias or temporal fault the run would find in a body
//! the certificate calls decided is the finding `model.decided-but-false`.
//!
//! The format, one record per line:
//!
//!     CobaltC certificate 1
//!     tool: cobc | coby
//!     program: FILE
//!     verdict: decided | paths kept: WHY | static pass: … | rejected: DIAG
//!     fn KEY: decided | left
//!       left FILE:LINE: WHY          each conflict the body leaves to the run
//!       dynamic FILE:LINE: …         a check kept: a reference may reach on some paths
//!       point FILE:LINE: …           a check made when a call is made (D-0283), or of a value
//!       tier-d FILE:LINE: …          a binding that keeps references in data
//!       origin FILE:LINE: …          a reference whose origin the pass does not know
//!       static-data FILE:LINE: …     a reference that holds static data (no fact)
//!       error DIAG                   the body was rejected
//!
//! `KEY` is the body's qualified name with its type arguments
//! (`tier::body_key`); `std`'s bodies the program reaches are listed after
//! the program's own. Lines are `FILE:LINE` in the file the text came from
//! (`<std>/FILE:LINE` inside the standard library).

use std::io::Write;
use std::path::Path;

/// The environment variable the static pass reads for the raw file.
pub const ENV: &str = "COBALTC_CERTIFICATE";

/// Starts a certificate at `path`: the file is emptied and the static pass
/// told to append its raw records there.
pub fn begin(path: &Path) -> std::io::Result<()> {
    std::fs::write(path, b"")?;
    std::env::set_var(ENV, path);
    Ok(())
}

struct Body {
    key: String,
    lefts: Vec<String>,
    events: Vec<String>,
    error: Option<String>,
}

/// Rewrites `path` from the raw records the static pass appended: the
/// header, `verdict`, and the bodies, each line mapped through `map`. A
/// body checked more than once (the pass over a program rewritten for
/// D-0162) is written once, its last record counting. `verdict` is the
/// compiler's; `None` (a static-only run,
/// `--check`) writes how many bodies the pass decided and how many left a
/// conflict to the run, since the compiler's own reasons for keeping paths
/// (a type that keeps references in data, a library) are not known there.
/// Returns those two counts.
pub fn finish(path: &Path, map: Option<&crate::loader::SourceMap>, tool: &str, program: &Path, verdict: Option<&str>) -> std::io::Result<(usize, usize)> {
    std::env::remove_var(ENV);
    let raw = std::fs::read_to_string(path)?;
    let mut bodies: Vec<Body> = Vec::new();
    let mut cur: Option<Body> = None;
    for line in raw.lines() {
        let (kind, rest) = line.split_once('\t').unwrap_or((line, ""));
        match kind {
            "fn" => cur = Some(Body { key: rest.to_string(), lefts: Vec::new(), events: Vec::new(), error: None }),
            "left" => {
                if let Some(b) = cur.as_mut() {
                    b.lefts.push(rest.to_string());
                }
            }
            "ev" => {
                if let Some(b) = cur.as_mut() {
                    b.events.push(rest.to_string());
                }
            }
            "error" => {
                if let Some(b) = cur.as_mut() {
                    b.error = Some(rest.to_string());
                }
            }
            "end" => {
                // A body checked again (the pass over a program rewritten for
                // D-0162 drops the first pass's facts): the last record counts.
                if let Some(b) = cur.take() {
                    bodies.retain(|x| x.key != b.key);
                    bodies.push(b);
                }
            }
            _ => {}
        }
    }
    // The program's own bodies first, then `std`'s, each group by name;
    // `main` ahead of the rest.
    bodies.sort_by(|a, b| {
        let rank = |k: &str| (k.starts_with("std::"), k != "main");
        rank(&a.key).cmp(&rank(&b.key)).then_with(|| a.key.cmp(&b.key))
    });
    let locate = |line: &str| -> String {
        let at = line.parse::<usize>().ok().and_then(|n| map.and_then(|m| crate::locate(n, m)));
        match at {
            Some((f, l)) => format!("{}:{}", f, l),
            None => format!("line {}", line),
        }
    };
    // `line N: WHY` as the static pass writes a reason.
    let reason = |why: &str| -> String {
        match why.strip_prefix("line ").and_then(|r| r.split_once(": ")) {
            Some((n, rest)) => format!("{}: {}", locate(n), rest),
            None => why.to_string(),
        }
    };
    let mut out = String::new();
    out.push_str("# What the static pass decided before the run (D-0299). A body marked `decided` left no conflict\n");
    out.push_str("# of its frame-held references to the run (`rule.control.frame-references`, spec/14 §6): a conforming\n");
    out.push_str("# compiler keeps no record of them (D-0284, D-0292). A body marked `left` names every conflict the run\n");
    out.push_str("# checks. `cobmodel certify CERT FILE` checks a fragment program's certificate against every interleaving.\n");
    let mut decided = 0;
    let mut left = 0;
    for b in &bodies {
        out.push('\n');
        if b.lefts.is_empty() && b.error.is_none() {
            decided += 1;
            out.push_str(&format!("fn {}: decided\n", b.key));
        } else if b.error.is_some() {
            out.push_str(&format!("fn {}: rejected\n", b.key));
        } else {
            left += 1;
            out.push_str(&format!("fn {}: left\n", b.key));
        }
        for why in &b.lefts {
            out.push_str(&format!("  left {}\n", reason(why)));
        }
        for e in &b.events {
            let f: Vec<&str> = e.split('\t').collect();
            let at = f.get(1).map(|l| locate(l)).unwrap_or_default();
            let rest = |from: usize| f.get(from..).map(|s| s.join(" ")).unwrap_or_default();
            let text = match f.first().copied() {
                Some("U") => match (f.get(2), f.get(3), f.get(4)) {
                    (Some(x), Some(m), Some(h)) if *m == "move" => format!("dynamic {}: a move of `{}`, which `{}` may reach on some paths", at, x, h),
                    (Some(x), Some(m), Some(h)) => format!("dynamic {}: an access to `{}` ({}), which `{}` may reach on some paths", at, x, m.to_lowercase(), h),
                    _ => format!("dynamic {}: {}", at, rest(2)),
                },
                Some("P") => format!("point {}: {}", at, rest(2)),
                Some("D") => format!("tier-d {}: `{}` ({}, from {}) keeps references in data", at, f.get(2).unwrap_or(&""), f.get(3).unwrap_or(&""), f.get(4).unwrap_or(&"")),
                Some("O") => format!("origin {}: what `{}` ({}, from {}) borrows is not known before the run", at, f.get(2).unwrap_or(&""), f.get(3).unwrap_or(&""), f.get(4).unwrap_or(&"")),
                Some("S") => format!("static-data {}: `{}` ({}, from {}) holds static data", at, f.get(2).unwrap_or(&""), f.get(3).unwrap_or(&""), f.get(4).unwrap_or(&"")),
                _ => format!("note {}", e.replace('\t', " ")),
            };
            out.push_str(&format!("  {}\n", text));
        }
        if let Some(e) = &b.error {
            // The diagnostic's id, which the raw text begins with.
            let id: String = e.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-').collect();
            out.push_str(&format!("  error {}\n", if id.is_empty() { e.as_str() } else { id.as_str() }));
        }
    }
    let verdict = match verdict {
        Some(v) => v.to_string(),
        None if left == 0 => format!("static pass: every body decided ({}); the compiler's verdict needs a build", decided),
        None => format!("static pass: {} of {} bodies left a conflict to the run", left, decided + left),
    };
    let head = format!("CobaltC certificate 1\ntool: {}\nprogram: {}\nverdict: {}\n", tool, program.display(), verdict);
    let mut f = std::fs::File::create(path)?;
    f.write_all(head.as_bytes())?;
    f.write_all(out.as_bytes())?;
    Ok((decided, left))
}

/// `finish` for a program the static pass rejected: the verdict names the
/// diagnostic.
pub fn finish_rejected(path: &Path, map: &crate::loader::SourceMap, tool: &str, program: &Path, diag: &str) -> std::io::Result<()> {
    // A lexical or grammatical error is `diag.syntax-error` (CHG-0082).
    let id = if diag.starts_with("parse error at ") || diag.starts_with("lex error at ") { "diag.syntax-error".to_string() } else { crate::resolve_user_location(diag, map).0 };
    finish(path, Some(map), tool, program, Some(&format!("rejected: {}", id))).map(|_| ())
}

/// `--certificate CERT --check FILE`: the static pass alone on one program,
/// as `check::run` does it, the certificate written after it. `extra`
/// checks an accepted program further (`cobc`: its `extern "…";` files).
/// The exit status is `check::run`'s: 0 accepted, 1 rejected, 2 unreadable.
pub fn check_one(file: &Path, tool: &str, extra: &dyn Fn(&crate::Analysis) -> Result<(), String>, cert: &Path) -> u8 {
    let src = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: error reading {}: {}", tool, file.display(), e);
            return 2;
        }
    };
    let a = crate::analyze(&src, Some(file));
    let status = if let Some(d) = &a.error {
        eprint!("{}", crate::diagnostics::for_stderr(&crate::render_located(d, "static", &a.map)));
        let _ = finish_rejected(cert, &a.map, tool, file, d);
        1
    } else {
        let _ = finish(cert, Some(&a.map), tool, file, None);
        match extra(&a) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{}: {}", tool, e);
                2
            }
        }
    };
    status
}
