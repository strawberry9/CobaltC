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
    pub phase: Option<String>,
    pub rule: Option<String>,
    pub invariant: Option<String>,
    pub required: Option<String>,
    pub observed: Option<String>,
    pub provenance: Option<String>,
    pub repair: Option<String>,
    pub related: Option<String>,
}

const FIELD_LABELS: &[&str] = &[
    "Phase: ",
    "Rule: ",
    "Invariant: ",
    "Required: ",
    "Observed: ",
    "Provenance: ",
    "Repair: ",
    "Related: ",
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
            phase: fields.get("Phase").cloned(),
            rule: fields.get("Rule").cloned(),
            invariant: fields.get("Invariant").cloned(),
            required: fields.get("Required").cloned(),
            observed: fields.get("Observed").cloned(),
            provenance: fields.get("Provenance").cloned(),
            repair: fields.get("Repair").cloned(),
            related: fields.get("Related").cloned(),
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

/// Splits an internal fault string (possibly `"diag.xxx@42"`, this
/// interpreter's own encoding for "diag id, raised while evaluating
/// source line 42") into the bare id and an optional line number.
/// Faults raised outside the few call sites that track a current line
/// (see `interp.rs`'s and `typecheck.rs`'s `current_line`-tracking
/// wrappers) carry no `@`, and `line` is `None` — reported honestly as
/// "location: unknown", not guessed at.
pub fn split_location(raw: &str) -> (&str, Option<usize>) {
    let (id, line) = match raw.rsplit_once('@') {
        Some((id, n)) => match n.parse::<usize>() {
            Ok(n) => (id, Some(n)),
            Err(_) => (raw, None),
        },
        None => (raw, None),
    };
    // A program-supplied detail (`DETAIL_SEP`) is not part of the id.
    (id.split(DETAIL_SEP).next().unwrap_or(id), line)
}

/// Separates a diagnostic's id from a message the program supplied
/// (`static_assert(c, "…")`, D-0052): `"diag.x\u{1}message@N"`.
pub const DETAIL_SEP: char = '\u{1}';

/// The program-supplied message of a raw diagnostic, if it carries one.
pub fn detail(raw: &str) -> Option<&str> {
    let (_, rest) = raw.split_once(DETAIL_SEP)?;
    Some(match rest.rsplit_once('@') {
        Some((m, n)) if n.parse::<usize>().is_ok() => m,
        _ => rest,
    })
}

/// Renders the full human-facing diagnostic. `phase_actual` is
/// "static" or "dynamic" as *this run* actually determined it (via
/// which pass raised it) — more precise than the registry's own
/// `Phase` field, which documents the union of every phase a
/// `disposition: both` rule may fire under. `at` is the *caller's*
/// resolved (file, line) (e.g. `main.rs`, via `resolve_user_location`:
/// the prelude's own line count subtracted and the loader's source map
/// applied) — this function never interprets a raw `"diag.xxx@N"`
/// string itself, so it can never accidentally report a line number
/// that still points inside invisible prelude source or a spliced file.
pub fn render(id: &str, phase_actual: &str, at: Option<(&str, usize)>) -> String {
    let reg = registry();
    let mut out = String::new();

    match at {
        Some((file, n)) => out.push_str(&format!("error: {} ({})\n  at {}:{}\n", id, phase_actual, file, n)),
        None => out.push_str(&format!("error: {} ({})\n  at: unknown location\n", id, phase_actual)),
    }

    let Some(info) = reg.get(id) else {
        out.push_str(&format!(
            "\n  (no entry for `{}` in spec/registry/diagnostics.md -- this is a gap in the\n   registry or in this interpreter's own diag id, not a fabricated explanation)\n",
            id
        ));
        return out;
    };

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
    if let Some(repair) = &info.repair {
        out.push_str(&format!("\n  repair: {}\n", repair));
    }
    if let Some(related) = &info.related {
        out.push_str(&format!("\n  see: spec/examples.md -> {}\n", related));
    }
    out.push_str("       spec/registry/diagnostics.md\n");
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
    }

    #[test]
    fn render_of_known_diag_contains_registry_content() {
        let msg = render("diag.aliasing-conflict", "dynamic", Some(("prog.cb", 7)));
        assert!(msg.contains("at prog.cb:7"));
        assert!(msg.contains("ex.borrow-conflict"));
        assert!(msg.contains("clash"));
    }

    #[test]
    fn render_of_unknown_diag_is_honest_about_the_gap() {
        let msg = render("diag.totally-made-up", "dynamic", None);
        assert!(msg.contains("no entry for"));
    }

    #[test]
    fn every_diag_id_this_interpreter_emits_is_in_the_registry() {
        // Cross-check against the authoritative list of what src/*.rs
        // actually raises (grepped by hand into this list when this
        // test was written -- see impl/STATUS.md for how it was
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
