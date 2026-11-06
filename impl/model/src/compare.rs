// Runs each program the enumerator wrote under the tools and compares what
// they do with the outcome set the model gives it (`// MODEL:` lines):
//
//   cobmodel compare DIR --coby PATH --cobc PATH [--explore N] [--cc CC]...
//                        [--jobs J] [--report FILE]
//
// `coby` runs each program once; `cobc` compiles it once per C compiler
// and runs it under N schedules (`--explore N`). Every outcome a tool
// shows must be in the model's set; a model outcome no tool showed is
// listed as unobserved (advisory: the tools explore a few schedules, the
// model all of them). Exit status 1 when any outcome is outside the set.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn run(cmd: &mut Command, limit: Duration) -> Option<std::process::Output> {
    let mut child = match cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("compare: cannot start {:?}: {}", cmd.get_program(), e);
            return None;
        }
    };
    let start = Instant::now();
    let out = Arc::new(Mutex::new(None));
    // Drain the pipes on threads so a chatty program cannot block.
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let ho = std::thread::spawn(move || {
        let mut v = Vec::new();
        std::io::Read::read_to_end(&mut so, &mut v).ok();
        v
    });
    let he = std::thread::spawn(move || {
        let mut v = Vec::new();
        std::io::Read::read_to_end(&mut se, &mut v).ok();
        v
    });
    loop {
        if let Ok(Some(st)) = child.try_wait() {
            *out.lock().unwrap() = Some(st);
            break;
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let status = out.lock().unwrap().take().unwrap();
    Some(std::process::Output { status, stdout: ho.join().unwrap(), stderr: he.join().unwrap() })
}

// One run's outcome from its stdout lines and its first two stderr lines.
fn outcome(code: Option<i32>, stdout: &str, err: &[&str]) -> String {
    let out: Vec<&str> = stdout.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    let end = if code == Some(0) {
        "ok".to_string()
    } else {
        let first = err.first().map_or("", |l| l.trim());
        let second = err.get(1).map_or("", |l| l.trim());
        // D-0228: `error[diag.x]: …` (static) or `fault[diag.x]: …`
        // (dynamic), then `--> file:line:col` (`file:line` without a column).
        let diag = match (first.strip_prefix("error["), first.strip_prefix("fault[")) {
            (Some(r), _) => r.split(']').next().map_or(String::new(), |id| format!("{} (static)", id)),
            (_, Some(r)) => r.split(']').next().map_or(String::new(), |id| format!("{} (dynamic)", id)),
            _ => first.trim_start_matches("error: ").to_string(),
        };
        let place = second.trim_start_matches("--> ");
        let parts: Vec<&str> = place.rsplitn(3, ':').collect();
        let line = match parts.as_slice() {
            [c, l, _] if c.parse::<usize>().is_ok() && l.parse::<usize>().is_ok() => l.to_string(),
            [l, ..] => l.to_string(),
            [] => "?".to_string(),
        };
        if diag.is_empty() {
            format!("exit {:?}", code)
        } else {
            format!("{} line {}", diag, line)
        }
    };
    if out.is_empty() {
        end
    } else {
        format!("[{}] {}", out.join(" "), end)
    }
}

fn plain(o: &std::process::Output) -> String {
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    let lines: Vec<&str> = err.lines().collect();
    outcome(o.status.code(), &String::from_utf8_lossy(&o.stdout), &lines)
}

// `cobc --explore` groups its runs by outcome: a header line
// `N run(s), exit S -- replay: …`, then `  | ` lines of output and `  ! `
// lines of errors.
fn explored(o: &std::process::Output) -> Vec<String> {
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
    if !text.starts_with("explored") {
        return vec![plain(o)];
    }
    let mut res = Vec::new();
    let mut code: Option<i32> = None;
    let mut out = String::new();
    let mut err: Vec<String> = Vec::new();
    let mut open = false;
    let flush = |res: &mut Vec<String>, code: Option<i32>, out: &str, err: &[String]| {
        let e: Vec<&str> = err.iter().map(|s| s.as_str()).collect();
        res.push(outcome(code, out, &e));
    };
    for l in text.lines().skip(1) {
        if l.contains(" -- replay:") {
            if open {
                flush(&mut res, code, &out, &err);
            }
            open = true;
            out.clear();
            err.clear();
            code = l.split("exit ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok());
        } else if let Some(x) = l.strip_prefix("  | ") {
            out.push_str(x);
            out.push('\n');
        } else if l == "  |" {
            out.push('\n');
        } else if let Some(x) = l.strip_prefix("  ! ") {
            err.push(x.to_string());
        }
    }
    if open {
        flush(&mut res, code, &out, &err);
    }
    res
}

struct Cfg {
    coby: PathBuf,
    cobc: PathBuf,
    explore: usize,
    ccs: Vec<String>,
}

struct Verdict {
    file: String,
    model: BTreeSet<String>,
    seen: Vec<(String, String)>, // (tool, outcome)
    stale_header: bool,
}

fn check_one(cfg: &Cfg, path: &Path) -> Verdict {
    let src = std::fs::read_to_string(path).unwrap_or_default();
    // The model's outcomes are computed afresh, so a sample never goes stale;
    // a header that disagrees is reported.
    let header: BTreeSet<String> = src.lines().filter_map(|l| l.strip_prefix("// MODEL: ")).map(|s| strip_phase(s)).collect();
    let model: BTreeSet<String> = match crate::parse::parse(&src) {
        Ok(p) => crate::explore::run(&p, true).outcomes.iter().map(|o| o.text()).collect(),
        Err(e) => [format!("model cannot read it: {}", e)].into_iter().collect(),
    };
    let stale_header = !header.is_empty() && header != model;
    let dir = path.parent().unwrap();
    let name = path.file_name().unwrap();
    let limit = Duration::from_secs(120);
    let mut seen = Vec::new();
    let mut c = Command::new(&cfg.coby);
    c.arg(name).current_dir(dir).env("COBALTC_CPU_LIMIT", "60").env("NO_COLOR", "1");
    match run(&mut c, limit) {
        Some(o) => seen.push(("coby".into(), plain(&o))),
        None => seen.push(("coby".into(), "timeout".into())),
    }
    for cc in &cfg.ccs {
        let mut c = Command::new(&cfg.cobc);
        c.arg("--cc").arg(cc).current_dir(dir).env("COBALTC_CPU_LIMIT", "60").env("NO_COLOR", "1");
        if cfg.explore > 0 {
            c.arg("--explore").arg(cfg.explore.to_string());
        } else {
            c.arg("--run");
        }
        c.arg(name);
        match run(&mut c, limit) {
            Some(o) => {
                for x in explored(&o) {
                    seen.push((format!("cobc[{}]", cc), x));
                }
            }
            None => seen.push((format!("cobc[{}]", cc), "timeout".into())),
        }
    }
    Verdict { file: path.display().to_string(), model, seen, stale_header }
}

// The model writes `diag.x (phase) line N`; the tools `diag.x (phase)` and
// the line from `at FILE:N`: both normalised to `diag.x (phase) line N`.
fn strip_phase(s: &str) -> String {
    s.trim().to_string()
}

pub fn main(args: &[String]) -> ExitCode {
    let Some(dir) = args.first() else {
        eprintln!("compare: DIR required");
        return ExitCode::from(2);
    };
    let mut cfg = Cfg { coby: "coby".into(), cobc: "cobc".into(), explore: 0, ccs: vec![] };
    let mut jobs = 2usize;
    let mut report_path: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        let v = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--coby" => cfg.coby = v.into(),
            "--cobc" => cfg.cobc = v.into(),
            "--explore" => cfg.explore = v.parse().unwrap_or(0),
            "--cc" => cfg.ccs.push(v),
            "--jobs" => jobs = v.parse().unwrap_or(2),
            "--report" => report_path = Some(v),
            _ => {
                eprintln!("compare: unknown option {}", args[i]);
                return ExitCode::from(2);
            }
        }
        i += 2;
    }
    if cfg.ccs.is_empty() {
        cfg.ccs = vec!["gcc".into(), "clang".into()];
    }
    // Each tool runs in its program's directory: a relative path given here
    // is made absolute first.
    for p in [&mut cfg.coby, &mut cfg.cobc] {
        if p.components().count() > 1 {
            if let Ok(a) = std::fs::canonicalize(&*p) {
                *p = a;
            }
        }
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).expect("dir").filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map_or(false, |x| x == "cb")).collect();
    files.sort();
    let cfg = Arc::new(cfg);
    let queue = Arc::new(Mutex::new(files.clone()));
    let results = Arc::new(Mutex::new(Vec::new()));
    let done = Arc::new(Mutex::new(0usize));
    let total = files.len();
    let hs: Vec<_> = (0..jobs)
        .map(|_| {
            let (cfg, queue, results, done) = (cfg.clone(), queue.clone(), results.clone(), done.clone());
            std::thread::spawn(move || loop {
                let next = queue.lock().unwrap().pop();
                let Some(p) = next else { break };
                let v = check_one(&cfg, &p);
                results.lock().unwrap().push(v);
                let mut d = done.lock().unwrap();
                *d += 1;
                if *d % 100 == 0 {
                    eprintln!("compare: {} / {}", *d, total);
                }
            })
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
    let mut results = Arc::try_unwrap(results).ok().unwrap().into_inner().unwrap();
    results.sort_by(|a, b| a.file.cmp(&b.file));
    let mut rep = String::new();
    let (mut bad, mut truncated, mut unobserved) = (0, 0, 0);
    for v in &results {
        let outside: Vec<&(String, String)> = v.seen.iter().filter(|(_, o)| !v.model.contains(o)).collect();
        let observed: BTreeSet<&String> = v.seen.iter().map(|(_, o)| o).collect();
        let missing: Vec<&String> = v.model.iter().filter(|m| !observed.contains(m)).collect();
        if v.stale_header {
            writeln!(rep, "header-stale {} (the // MODEL: lines differ from the model's set now)", v.file).unwrap();
        }
        // A set the explorer cut short at its state limit (`too-big`) is
        // incomplete: an outcome beyond it is not a disagreement, only an
        // outcome the model did not reach. Reported apart, as advisory.
        let cut_short = v.model.contains("too-big");
        if !outside.is_empty() && cut_short {
            truncated += 1;
            writeln!(rep, "truncated {}: {}", v.file, outside.iter().map(|(t, o)| format!("{}: {}", t, o)).collect::<Vec<_>>().join(" | ")).unwrap();
        } else if !outside.is_empty() {
            bad += 1;
            writeln!(rep, "OUTSIDE {}", v.file).unwrap();
            for m in &v.model {
                writeln!(rep, "    model: {}", m).unwrap();
            }
            for (t, o) in &outside {
                writeln!(rep, "    {}: {}", t, o).unwrap();
            }
        } else if !missing.is_empty() {
            unobserved += 1;
            writeln!(rep, "unobserved {}: {}", v.file, missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" | ")).unwrap();
        }
    }
    writeln!(rep, "compare: {} programs, {} with an outcome outside the model's set, {} beyond a set cut short at the state limit, {} with a model outcome no tool showed", results.len(), bad, truncated, unobserved).unwrap();
    match report_path {
        Some(p) => std::fs::write(p, &rep).expect("report"),
        None => print!("{}", rep),
    }
    eprintln!("compare: {} programs, {} outside, {} truncated, {} unobserved", results.len(), bad, truncated, unobserved);
    if bad > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
