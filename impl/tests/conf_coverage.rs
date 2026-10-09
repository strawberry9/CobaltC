// Every `conf.*` case in spec/conformance.md must be referenced, by its
// id, somewhere in the suites: as a `conf.<id>` string in
// tests/conformance.rs (a `// covers:` comment), as a test named
// `conf_<id with underscores>` / `ex_e2e_<...>`, or in the `facility:`
// or description lines of a file-based case under impl/conformance/.
// A new spec case without a test fails here, by name.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn read_dir_cb(dir: &Path, out: &mut String) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                read_dir_cb(&p, out);
            } else if p.extension().map_or(false, |x| x == "cb") {
                out.push_str(&fs::read_to_string(&p).unwrap_or_default());
                out.push('\n');
            }
        }
    }
}

#[test]
fn every_spec_conformance_case_is_referenced_by_a_test() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = fs::read_to_string(root.join("../spec/conformance.md")).expect("spec/conformance.md");
    let mut ids = BTreeSet::new();
    for line in spec.lines() {
        if let Some(rest) = line.strip_prefix("| `conf.") {
            if let Some(end) = rest.find('`') {
                ids.insert(format!("conf.{}", &rest[..end]));
            }
        }
    }
    assert!(ids.len() > 100, "parsed only {} case ids from spec/conformance.md", ids.len());
    let mut corpus = fs::read_to_string(root.join("tests/conformance.rs")).unwrap();
    read_dir_cb(&root.join("conformance"), &mut corpus);
    let mut mentioned: BTreeSet<String> = BTreeSet::new();
    for m in corpus.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')) {
        if let Some(rest) = m.strip_prefix("conf.") {
            mentioned.insert(format!("conf.{}", rest));
        } else if let Some(rest) = m.strip_prefix("conf_") {
            mentioned.insert(format!("conf.{}", rest.replace('_', "-")));
        } else if let Some(rest) = m.strip_prefix("ex_e2e_") {
            mentioned.insert(format!("conf.e2e-{}", rest.replace('_', "-")));
        }
    }
    let missing: Vec<&String> = ids.iter().filter(|id| !mentioned.contains(*id)).collect();
    assert!(missing.is_empty(), "spec/conformance.md cases with no test referencing them: {:?}", missing);
}
