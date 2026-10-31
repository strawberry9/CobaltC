// Renders a `diag.*` id into a human-readable, spec-grounded message.
//
// The content is never hand-invented: this module parses
// `spec/registry/diagnostics.md` itself (embedded verbatim at compile
// time via `include_str!`, so it can never silently drift out of sync
// with a future edit to that file the way a hand-transcribed copy
// could) and renders its own Phase/Rule/Invariant/Required/Observed/
// Provenance/Repair/Related fields for the id actually raised. If an id
// this interpreter emits has no matching registry entry (or vice
// versa), that is reported honestly as a gap, not papered over with
// invented prose.

use std::collections::HashMap;

const REGISTRY_SRC: &str = include_str!("../../spec/registry/diagnostics.md");

#[derive(Debug, Clone, Default)]
pub struct DiagInfo {
    pub ids: Vec<String>,
    pub summary: Option<String>,
    pub phase: Option<String>,
    pub rule: Option<String>,
    pub invariant: Option<String>,
    pub required: Option<String>,
    pub observed: Option<String>,
    pub provenance: Option<String>,
    pub repair: Option<String>,
    pub related: Option<String>,
    pub location: Option<String>,
}

const FIELD_LABELS: &[&str] = &[
    "Summary: ",
    "Phase: ",
    "Rule: ",
    "Invariant: ",
    "Required: ",
    "Observed: ",
    "Provenance: ",
    "Repair: ",
    "Related: ",
    "Location: ",
];

fn extract_diag_ids(names_part: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = names_part;
    while let Some(start) = rest.find('`') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('`') else { break };
        let candidate = &after[..end];
        if candidate.starts_with("diag.") {
            ids.push(candidate.to_string());
        }
        rest = &after[end + 1..];
    }
    ids
}

fn parse_fields(fields_part: &str) -> HashMap<&'static str, String> {
    // Find where each known label starts (if it appears at all), then
    // slice each field's text from its label to the next label found
    // (or end of line for the last one). Position-based, not
    // period-based, so a field's own value may safely contain periods
    // (parentheticals, formulas) without truncating early.
    let mut positions: Vec<(usize, &'static str)> = FIELD_LABELS
        .iter()
        .filter_map(|&label| fields_part.find(label).map(|pos| (pos, label)))
        .collect();
    positions.sort_by_key(|&(pos, _)| pos);

    let mut out = HashMap::new();
    for (i, &(pos, label)) in positions.iter().enumerate() {
        let value_start = pos + label.len();
        let value_end = positions.get(i + 1).map(|&(p, _)| p).unwrap_or(fields_part.len());
        let mut value = fields_part[value_start..value_end].trim();
        value = value.trim_end_matches('.').trim_end_matches(' ');
        let key = label.trim_end_matches(": ");
        out.insert(key, value.to_string());
    }
    out
}

/// Parses `spec/registry/diagnostics.md`'s bullet list. One bullet may
/// name several `diag.*` ids sharing one description (e.g.
/// `diag.bad-destructor-signature`, `diag.direct-destructor-call`);
/// each gets its own map entry pointing at the same shared `DiagInfo`.
pub fn parse_registry(text: &str) -> HashMap<String, DiagInfo> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with("- `diag.") {
            continue;
        }
        let rest = &line[2..]; // drop "- "
        let Some((names_part, fields_part)) = rest.split_once(" — ") else {
            continue;
        };
        let ids = extract_diag_ids(names_part);
        if ids.is_empty() {
            continue;
        }
        let fields = parse_fields(fields_part);
        let info = DiagInfo {
            ids: ids.clone(),
            summary: fields.get("Summary").cloned(),
            phase: fields.get("Phase").cloned(),
            rule: fields.get("Rule").cloned(),
            invariant: fields.get("Invariant").cloned(),
            required: fields.get("Required").cloned(),
            observed: fields.get("Observed").cloned(),
            provenance: fields.get("Provenance").cloned(),
            repair: fields.get("Repair").cloned(),
            related: fields.get("Related").cloned(),
            location: fields.get("Location").cloned(),
        };
        for id in &ids {
            map.insert(id.clone(), info.clone());
        }
    }
    map
}

pub fn registry() -> HashMap<String, DiagInfo> {
    parse_registry(REGISTRY_SRC)
}

/// Splits an internal fault string (possibly `"diag.xxx@42"` or
/// `"diag.xxx@42:7"`, this interpreter's own encoding for "diag id,
/// raised while evaluating source line 42, column 7") into the bare id
/// and an optional line number. Faults raised outside the few call
/// sites that track a current line (see `interp.rs`'s and
/// `typecheck.rs`'s `current_line`-tracking wrappers) carry no `@`, and
/// `line` is `None` — reported honestly as "location: unknown", not
/// guessed at.
pub fn split_location(raw: &str) -> (&str, Option<usize>) {
    let (id, line, _) = split_location_col(raw);
    (id, line)
}

/// As `split_location`, with the column too (0 when the raw diagnostic
/// names none).
pub fn split_location_col(raw: &str) -> (&str, Option<usize>, usize) {
    let (id, line, col) = match raw.rsplit_once('@').and_then(|(id, n)| parse_at(n).map(|(l, c)| (id, l, c))) {
        Some((id, l, c)) => (id, Some(l), c),
        None => (raw, None, 0),
    };
    // A program-supplied detail (`DETAIL_SEP`) is not part of the id.
    (id.split(DETAIL_SEP).next().unwrap_or(id), line, col)
}

// `42` or `42:7`.
fn parse_at(s: &str) -> Option<(usize, usize)> {
    match s.split_once(':') {
        Some((l, c)) => Some((l.parse().ok()?, c.parse().ok()?)),
        None => Some((s.parse().ok()?, 0)),
    }
}

/// The location suffix of a raw diagnostic: `@42`, or `@42:7` when the
/// column is known.
pub fn at(line: usize, col: usize) -> String {
    if col == 0 { format!("@{}", line) } else { format!("@{}:{}", line, col) }
}

/// Separates a diagnostic's id from a message the program supplied
/// (`static_assert(c, "…")`, D-0052): `"diag.x\u{1}message@N"`.
pub const DETAIL_SEP: char = '\u{1}';

/// Whether a raw diagnostic already ends in its location (`"…@42"`,
/// `"…@42:7"`); a program's message may itself contain `@`.
pub fn has_location(raw: &str) -> bool {
    raw.rsplit_once('@').map_or(false, |(_, n)| parse_at(n).is_some())
}

/// The id and the phase a shown diagnostic's first line names:
/// `error[diag.x]: …` is static, `fault[diag.x]: …` dynamic (`render`).
pub fn first_line_id(line: &str) -> Option<(&str, &'static str)> {
    let (phase, rest) = if let Some(r) = line.strip_prefix("error[") {
        ("static", r)
    } else if let Some(r) = line.strip_prefix("fault[") {
        ("dynamic", r)
    } else {
        return None;
    };
    let id = &rest[..rest.find(']')?];
    id.starts_with("diag.").then_some((id, phase))
}

/// Whether a diagnostic is shown with its full registry entry (`--explain`,
/// or `COBALTC_EXPLAIN` set to anything but empty or `0`), rather than
/// in the short form.
pub fn explain() -> bool {
    static EXPLAIN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *EXPLAIN.get_or_init(|| std::env::var_os("COBALTC_EXPLAIN").map_or(false, |v| !v.is_empty() && v != "0"))
}

/// Whether diagnostics written to stderr are colored: only on a
/// terminal, never when `NO_COLOR` is set (no-color.org), and on Windows
/// only once the console accepts ANSI sequences.
pub fn stderr_color() -> bool {
    use std::io::IsTerminal;
    static COLOR: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *COLOR.get_or_init(|| {
        std::env::var_os("NO_COLOR").map_or(true, |v| v.is_empty())
            && std::io::stderr().is_terminal()
            && enable_ansi()
    })
}

#[cfg(windows)]
fn enable_ansi() -> bool {
    // Older consoles print ANSI sequences raw unless asked to process them.
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    extern "system" {
        fn GetStdHandle(n: u32) -> *mut core::ffi::c_void;
        fn GetConsoleMode(h: *mut core::ffi::c_void, mode: *mut u32) -> i32;
        fn SetConsoleMode(h: *mut core::ffi::c_void, mode: u32) -> i32;
    }
    unsafe {
        let h = GetStdHandle(STD_ERROR_HANDLE);
        let mut mode = 0u32;
        GetConsoleMode(h, &mut mode) != 0
            && (mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0
                || SetConsoleMode(h, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) != 0)
    }
}

#[cfg(not(windows))]
fn enable_ansi() -> bool {
    true
}

/// A rendered diagnostic as it goes to stderr: colored the way rustc
/// colors its own when `stderr_color()` says so, otherwise unchanged.
pub fn for_stderr(out: &str) -> String {
    if stderr_color() { paint(out) } else { out.to_string() }
}

/// Colors a rendered diagnostic: `error`/`fault` bold red and the rest
/// of its first line bold, the `-->` location and the source excerpt's
/// gutter bold blue, the caret bold red, `= repair:` bold cyan. Only the
/// lines `render` lays out are touched, and of an excerpt only its
/// gutter, so a message's or a source line's own text is never painted
/// as one of them.
pub fn paint(out: &str) -> String {
    const RESET: &str = "\x1b[0m";
    let mut s = String::with_capacity(out.len() + 64);
    // The line number's width, once the `-->` line gave it: the excerpt's
    // rows are laid out on it (`render`).
    let mut width: Option<usize> = None;
    for (i, line) in out.split_inclusive('\n').enumerate() {
        let (body, nl) = match line.strip_suffix('\n') {
            Some(b) => (b, "\n"),
            None => (line, ""),
        };
        let lead = body.len() - body.trim_start_matches(' ').len();
        let styled = if i == 0 && (body.starts_with("error[") || body.starts_with("fault[")) {
            format!("\x1b[1;31m{}{}\x1b[1m{}{}", &body[..5], RESET, &body[5..], RESET)
        } else if width.is_none() && body[lead..].starts_with("--> ") {
            width = Some(lead);
            format!("{}\x1b[1;34m-->{}{}", &body[..lead], RESET, &body[lead + 3..])
        } else if let Some(w) = width {
            let gut = " ".repeat(w + 1);
            if body == format!("{}|", gut) {
                format!("\x1b[1;34m{}{}", body, RESET)
            } else if let Some(rest) = body.strip_prefix(&format!("{}= repair: ", gut)) {
                format!("{}\x1b[1;36m= repair:{} {}", gut, RESET, rest)
            } else if let Some(rest) = body.strip_prefix(&format!("{}| ", gut)) {
                let carets = rest.len() - rest.trim_end_matches('^').len();
                let (pad, hat) = rest.split_at(rest.len() - carets);
                format!("\x1b[1;34m{}|{} {}\x1b[1;31m{}{}", gut, RESET, pad, hat, RESET)
            } else if body.len() > w + 2 && body[..w].bytes().all(|b| b.is_ascii_digit()) && body[w..].starts_with(" | ") {
                format!("\x1b[1;34m{}{}{}", &body[..w + 2], RESET, &body[w + 2..])
            } else {
                body.to_string()
            }
        } else {
            body.to_string()
        };
        s.push_str(&styled);
        s.push_str(nl);
    }
    s
}

/// The program-supplied message of a raw diagnostic, if it carries one.
pub fn detail(raw: &str) -> Option<&str> {
    let (_, rest) = raw.split_once(DETAIL_SEP)?;
    Some(match rest.rsplit_once('@') {
        Some((m, n)) if parse_at(n).is_some() => m,
        _ => rest,
    })
}

/// The text of line `line` (1-based) of `file`, read when the diagnostic
/// is rendered; None for a file that cannot be read (or `std`'s, which
/// has no file of its own beside the program).
pub fn source_line(file: &str, line: usize) -> Option<String> {
    if file.starts_with("<std>/") || line == 0 {
        return None;
    }
    let text = std::fs::read(file).ok()?;
    let text = String::from_utf8_lossy(&text);
    text.split('\n').nth(line - 1).map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
}

// Where on `src` the caret goes and how far it reaches, for the byte
// column `col` (1-based; 0 when the diagnostic names none, which puts it
// at the line's first token): (the excerpt's padding, the column as
// characters, the caret's length). An identifier, keyword or number is
// underlined whole, a text literal to its closing quote, anything else
// one character.
fn caret(src: &str, col: usize) -> Option<(String, usize, usize)> {
    let b = src.as_bytes();
    let mut k = if col == 0 { b.iter().position(|c| *c != b' ' && *c != b'\t')? } else { col - 1 };
    k = k.min(b.len());
    while !src.is_char_boundary(k) {
        k -= 1;
    }
    let pad: String = src[..k].chars().map(|c| if c == '\t' { '\t' } else { ' ' }).collect();
    let chars = src[..k].chars().count() + 1;
    let rest = &src[k..];
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let len = match rest.chars().next() {
        None => 1,
        Some(c) if word(c) => rest.chars().take_while(|c| word(*c)).count(),
        Some('"') => {
            let mut n = 1;
            let mut esc = false;
            for c in rest.chars().skip(1) {
                n += 1;
                if esc {
                    esc = false;
                } else if c == '\\' {
                    esc = true;
                } else if c == '"' {
                    break;
                }
            }
            n
        }
        Some(_) => 1,
    };
    Some((pad, chars, len))
}

/// Renders a diagnostic. `phase_actual` is "static" or "dynamic" as
/// *this run* determined it (via which pass raised it) — more precise
/// than the registry's own `Phase` field, which documents the union of
/// every phase a `disposition: both` rule may fire under: a static one
/// is an `error`, a dynamic one a `fault`. `at` is the *caller's*
/// resolved (file, line, byte column; 0 when unknown), e.g. `main.rs`
/// via `resolve_user_location` — this function never interprets a raw
/// `"diag.xxx@N"` string itself, so it can never report a line that
/// still points inside invisible prelude source or a spliced file.
/// `message` is what the program or the rule said about this occurrence;
/// without one, the registry's summary of the diagnostic.
///
/// The short form (WI-9, D-0228): the first line, the location, the
/// source line read from the file with a caret under the token, and the
/// repair. Under `explain()`, the registry's rule, invariant, required,
/// observed and provenance follow, and the examples it cites.
pub fn render(id: &str, phase_actual: &str, at: Option<(&str, usize, usize)>, message: Option<&str>) -> String {
    let reg = registry();
    let info = reg.get(id);
    let kind = if phase_actual == "dynamic" { "fault" } else { "error" };
    let summary = info.and_then(|i| i.summary.clone());
    let text = match (message, summary) {
        (Some(m), _) => m.to_string(),
        (None, Some(s)) => s,
        (None, None) => String::from("(no summary)"),
    };
    let mut out = format!("{}[{}]: {}\n", kind, id, text);
    // A dynamic diagnostic is located by its line alone, its caret under
    // the line's first token (D-0228): the two implementations check some
    // operations of one statement at different sub-expressions, and agree
    // on the line.
    let at = at.map(|(f, l, c)| (f, l, if phase_actual == "dynamic" { 0 } else { c }));
    let excerpt = at.and_then(|(f, l, c)| source_line(f, l).map(|s| (s, l, c)));
    let width = at.map_or(1, |(_, l, _)| l.to_string().len());
    let gut = " ".repeat(width + 1);
    match (at, &excerpt) {
        (Some((file, line, col)), Some((src, _, _))) => match caret(src, col) {
            Some((pad, chars, len)) => {
                out.push_str(&format!("{}--> {}:{}:{}\n", " ".repeat(width), file, line, chars));
                out.push_str(&format!("{}|\n", gut));
                out.push_str(&format!("{} | {}\n", line, src));
                out.push_str(&format!("{}| {}{}\n", gut, pad, "^".repeat(len)));
            }
            None => out.push_str(&format!("{}--> {}:{}\n", " ".repeat(width), file, line)),
        },
        (Some((file, line, col)), None) => {
            if col == 0 {
                out.push_str(&format!("{}--> {}:{}\n", " ".repeat(width), file, line));
            } else {
                out.push_str(&format!("{}--> {}:{}:{}\n", " ".repeat(width), file, line, col));
            }
        }
        (None, _) => out.push_str(&format!("{}--> unknown location\n", " ".repeat(width))),
    }
    let Some(info) = info else {
        out.push_str(&format!(
            "{}= note: no entry for `{}` in spec/registry/diagnostics.md -- a gap in the registry or in this tool's diag id, not a fabricated explanation\n",
            gut, id
        ));
        return out;
    };
    if let Some(repair) = &info.repair {
        out.push_str(&format!("{}= repair: {}\n", gut, repair));
    }
    if !explain() {
        return out;
    }
    out.push('\n');
    if let Some(rule) = &info.rule {
        out.push_str(&format!("  rule:      {}\n", rule));
    }
    if let Some(inv) = &info.invariant {
        out.push_str(&format!("  invariant: {}\n", inv));
    }
    if let Some(req) = &info.required {
        out.push_str(&format!("  required:  {}\n", req));
    }
    if let Some(obs) = &info.observed {
        out.push_str(&format!("  observed:  {}\n", obs));
    }
    if let Some(prov) = &info.provenance {
        out.push_str(&format!("  provenance: {}\n", prov));
    }
    if let Some(related) = &info.related {
        out.push_str(&format!("\n  see: spec/examples.md -> {}\n", related));
        out.push_str("       spec/registry/diagnostics.md\n");
    } else {
        out.push_str("\n  see: spec/registry/diagnostics.md\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_simple_entry() {
        let reg = registry();
        let info = reg.get("diag.div-by-zero").expect("diag.div-by-zero should be in the registry");
        assert_eq!(info.phase.as_deref(), Some("both"));
        assert_eq!(info.rule.as_deref(), Some("`[Div-By-Zero]`"));
        assert!(info.repair.as_deref().unwrap().contains("checked_div"));
        assert!(info.related.as_deref().unwrap().contains("ex.checked-arithmetic"));
    }

    #[test]
    fn parses_an_entry_with_no_invariant_or_observed() {
        let reg = registry();
        let info = reg.get("diag.ambiguous-name").expect("present");
        assert_eq!(info.invariant, None);
        assert!(info.required.is_some());
    }

    #[test]
    fn shared_bullet_covers_every_named_id() {
        let reg = registry();
        let a = reg.get("diag.bad-destructor-signature").expect("present");
        let b = reg.get("diag.direct-destructor-call").expect("present");
        assert_eq!(a.repair, b.repair);
    }

    #[test]
    fn field_values_containing_periods_are_not_truncated() {
        let reg = registry();
        let info = reg.get("diag.capture-list-mismatch").expect("present");
        // The real Repair text has an internal ';' but also runs well
        // past the first '.' in "the list documents an already-derived
        // fact" -- position-based slicing must keep all of it.
        let repair = info.repair.as_deref().unwrap();
        assert!(repair.contains("add the missing name"));
        assert!(repair.contains("cannot select a different capture set"));
    }

    #[test]
    fn split_location_parses_embedded_line() {
        assert_eq!(split_location("diag.arith-overflow@42"), ("diag.arith-overflow", Some(42)));
        assert_eq!(split_location("diag.arith-overflow"), ("diag.arith-overflow", None));
        assert_eq!(split_location_col("diag.arith-overflow@42:7"), ("diag.arith-overflow", Some(42), 7));
        assert!(has_location("diag.x@3:9") && has_location("diag.x@3") && !has_location("diag.x@here"));
        assert_eq!(at(3, 0), "@3");
        assert_eq!(at(3, 9), "@3:9");
    }

    #[test]
    fn first_line_names_id_and_phase() {
        assert_eq!(first_line_id("error[diag.type-mismatch]: x"), Some(("diag.type-mismatch", "static")));
        assert_eq!(first_line_id("fault[diag.arith-overflow]: arithmetic overflow"), Some(("diag.arith-overflow", "dynamic")));
        assert_eq!(first_line_id("unsupported: no C"), None);
    }

    #[test]
    fn every_entry_has_a_summary() {
        let reg = registry();
        let missing: Vec<&String> = reg.iter().filter(|(_, i)| i.summary.is_none()).map(|(k, _)| k).collect();
        assert!(missing.is_empty(), "registry entries without a Summary: {:?}", missing);
    }

    #[test]
    fn render_of_known_diag_is_the_short_form() {
        let msg = render("diag.aliasing-conflict", "dynamic", Some(("no-such-file.cb", 7, 0)), None);
        assert!(msg.starts_with("fault[diag.aliasing-conflict]: "), "{}", msg);
        assert!(msg.contains("--> no-such-file.cb:7\n"), "{}", msg);
        assert!(msg.contains("= repair: "), "{}", msg);
        assert!(!msg.contains("rule:"), "{}", msg);
    }

    #[test]
    fn caret_counts_characters_and_keeps_tabs() {
        assert_eq!(caret("\tx = é + bad;", 11), Some(("\t        ".to_string(), 10, 3)));
        assert_eq!(caret("    i32 x;", 0), Some(("    ".to_string(), 5, 3)));
        assert_eq!(caret("  f(\"a\\\"b\", 1)", 5), Some(("    ".to_string(), 5, 6)));
    }

    #[test]
    fn paint_colors_only_the_fixed_parts_and_keeps_the_text() {
        let out = "error[diag.static-assert-failed]: --> not a location\n --> p.cb:3:5\n  |\n3 |     x | y\n  |     ^\n  = repair: fix it\n";
        let painted = paint(out);
        assert!(painted.starts_with("\x1b[1;31merror\x1b[0m\x1b[1m[diag.static-assert-failed]: --> not a location\x1b[0m"), "{:?}", painted);
        assert!(painted.contains(" \x1b[1;34m-->\x1b[0m p.cb:3:5\n"), "{:?}", painted);
        assert!(painted.contains("\x1b[1;34m3 |\x1b[0m     x | y\n"), "{:?}", painted);
        assert!(painted.contains("\x1b[1;31m^\x1b[0m"), "{:?}", painted);
        assert!(painted.contains("\x1b[1;36m= repair:\x1b[0m fix it"), "{:?}", painted);
        let plain: String = {
            let mut s = String::new();
            let mut it = painted.chars();
            while let Some(c) = it.next() {
                if c == '\x1b' {
                    for c in it.by_ref() {
                        if c == 'm' {
                            break;
                        }
                    }
                } else {
                    s.push(c);
                }
            }
            s
        };
        assert_eq!(plain, out);
    }

    #[test]
    fn render_of_unknown_diag_is_honest_about_the_gap() {
        let msg = render("diag.totally-made-up", "dynamic", None, None);
        assert!(msg.contains("no entry for"));
    }

    #[test]
    fn every_diag_id_this_interpreter_emits_is_in_the_registry() {
        // Cross-check against the authoritative list of what src/*.rs
        // actually raises (grepped by hand into this list when this
        // test was written -- see impl/CHANGELOG.md for how it was
        // derived and kept in sync).
        let emitted = crate::interp::ALL_EMITTED_DIAGS;
        let reg = registry();
        let mut missing = Vec::new();
        for id in emitted {
            if !reg.contains_key(*id) {
                missing.push(*id);
            }
        }
        assert!(missing.is_empty(), "diag ids emitted by the interpreter but absent from spec/registry/diagnostics.md: {:?}", missing);
    }
}
