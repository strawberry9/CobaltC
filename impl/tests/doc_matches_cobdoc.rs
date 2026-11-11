// CHG-0446: `--doc` reads `std`'s documentation as `impl/tools/cobdoc.py`
// does (the guide's std reference is built from cobdoc.py): every item, in
// the same order, with the same module, kind, signature, documentation,
// number of requirements and examples' code. Skipped without python3.

use std::process::Command;

const DUMP: &str = r#"
import json, sys
sys.path.insert(0, sys.argv[1])
import cobdoc
out = []
cobdoc.items_of(cobdoc.STD, "std", out, set())
out += cobdoc.builtins()
def esc(s):
    return (s or "").replace("\\", "\\\\").replace("\n", "\\n").replace("\t", "\\t")
for x in out:
    ex = " | ".join(esc(e["code"]) for e in x.get("examples", []))
    print("\t".join([x["module"], x["item"], x["kind"], esc(x["signature"]), esc(x["doc"]), str(len(x.get("requires", []))), ex]))
"#;

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t")
}

#[test]
fn doc_matches_cobdoc() {
    let tools = concat!(env!("CARGO_MANIFEST_DIR"), "/tools");
    let out = match Command::new("python3").arg("-c").arg(DUMP).arg(tools).output() {
        Ok(o) if o.status.success() => o,
        Ok(o) => panic!("cobdoc.py failed: {}", String::from_utf8_lossy(&o.stderr)),
        Err(_) => {
            eprintln!("doc_matches_cobdoc: no python3, skipped");
            return;
        }
    };
    let want: Vec<String> = String::from_utf8(out.stdout).unwrap().lines().map(|l| l.to_string()).collect();
    let got: Vec<String> = coby::doc::std_items()
        .iter()
        .map(|x| {
            let ex = x.examples.iter().map(|(_, c)| esc(c)).collect::<Vec<_>>().join(" | ");
            [x.module.clone(), x.item.clone(), x.kind.clone(), esc(&x.signature), esc(x.doc.as_deref().unwrap_or("")), x.requires.len().to_string(), ex].join("\t")
        })
        .collect();
    for (k, (w, g)) in want.iter().zip(got.iter()).enumerate() {
        assert_eq!(w, g, "item {} differs", k);
    }
    assert_eq!(want.len(), got.len(), "item counts differ");
}

#[test]
fn doc_answers_an_item_a_type_and_a_misspelling() {
    let r = coby::doc::std_records();
    let push = coby::doc::lookup(r, Some("Vec::push")).unwrap();
    assert!(push.starts_with("export fn Vec::push<T>"), "{}", push);
    let vec = coby::doc::lookup(r, Some("Vec")).unwrap();
    assert!(vec.contains("The functions of `Vec`:") && vec.contains("Vec::len"), "{}", vec);
    let e = coby::doc::lookup(r, Some("Vec::psh")).unwrap_err();
    assert!(e.contains("did you mean `Vec::push`?"), "{}", e);
    // Every name the hints table offers for `T::name` is a function of some
    // `std` type (CHG-0445).
    let names: Vec<&str> = r.iter().map(|x| x.item.rsplit("::").next().unwrap()).collect();
    for word in ["length", "push_back", "pop_back", "includes", "put", "from", "to_string", "erase", "at", "equals", "startsWith", "endsWith"] {
        for target in coby::hints::fn_name(word) {
            assert!(names.contains(target), "`{}` (for `{}`) is no std function's name", target, word);
        }
    }
}
