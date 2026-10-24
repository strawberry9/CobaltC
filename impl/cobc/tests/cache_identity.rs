// The object cache's premise (cobc/src/cache.rs): a `std` function
// instantiated at `std` types alone lowers to the same C text in every
// program, so its object, kept under a key that hashes that text, serves
// every later program that needs it. The lowering keeps that identity by
// construction (temporaries numbered per function, string literals named
// by content, type ids hashed from the type's name), and a lowering change
// that loses it shows up late and obliquely: as a stale object linked, a
// unit compiled again in every program, or two helpers named alike
// (CHG-0337). Each test here compiles programs into one fresh cache and
// reads `-v`'s "N unit(s) reused, M compiled" line: what one program put
// there, the next must find.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "oracle/exe.rs"]
mod exe;
use exe::cobc_exe;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cobc-identity-{}-{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

// Compiles `src` as `name` in its own directory under `root`, into the
// cache at `cache`; gives (reused, compiled) from the verbose line.
fn build(root: &Path, name: &str, src: &str, cache: &Path) -> (usize, usize) {
    let dir = root.join(name);
    fs::create_dir_all(&dir).unwrap();
    let file = format!("{}.cb", name);
    fs::write(dir.join(&file), src).unwrap();
    let out = Command::new(cobc_exe())
        .args(["-v", "-o", "prog_out", &file])
        .env("COBC_CACHE", cache)
        .current_dir(&dir)
        .output()
        .expect("run cobc");
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "{} failed to compile: {}", name, err);
    let line = err
        .lines()
        .find(|l| l.contains("object cache ") && l.contains("reused"))
        .unwrap_or_else(|| panic!("no cache line for {}: {}", name, err));
    let words: Vec<&str> = line.split_whitespace().collect();
    let at = |after: &str| -> usize {
        let i = words.iter().position(|w| *w == after).unwrap();
        words[i - 1].parse().unwrap()
    };
    (at("unit(s)"), at("compiled,"))
}

// The `std` use every program below shares: a `Vec<i64>` grown, sorted
// and popped, a `String` searched, a `HashMap<String, i64>` filled and
// read -- each at `std` types alone, so each a cacheable unit.
const SHARED: &str = r#"
    Vec<i64> v = Vec::new();
    Vec::push(&mut v, 3);
    Vec::push(&mut v, 1);
    Vec::sort(&mut v);
    Option<i64> last = Vec::pop(&mut v);
    String s = String::from_str("alpha beta");
    Option<usize> at = String::find(&s, "beta");
    HashMap<String, i64> m = HashMap::new();
    HashMap::insert(&mut m, String::from_str("k"), 7);
    String key = String::from_str("k");
    Option<ref<i64, shared>> got = HashMap::get(&m, &key);
    printf("%v %v %v %v %v\n", Vec::len(&v), Option::is_some(&last), Option::is_some(&at), Option::is_some(&got), String::len(&s));
"#;

fn program_a() -> String {
    format!("import std;\n\nfn main()\n{{{}}}\n", SHARED)
}

// The same `std` use inside a different program: other names, its own
// struct and functions, other literals, more locals around the calls.
fn program_b() -> String {
    format!(
        r#"import std;

struct Tally {{ i64 count; String label; }}

fn describe_tally(ref<Tally, shared> t) : i64
{{
    printf("tally %v %v\n", t.count, &t.label);
    t.count * 2
}}

fn work(i64 seed) : i64
{{
    i64 acc = seed;
    Tally t = Tally {{ .count = seed, .label = String::from_str("first run") }};
    acc = acc + describe_tally(&t);
    {}
    acc
}}

fn main()
{{
    i64 total = work(11) + work(12);
    printf("total %v\n", total);
}}
"#,
        SHARED
    )
}

// Program A once more under another name, with a helper of the program's
// own and a literal of its own beside the shared use.
fn program_a_again() -> String {
    format!(
        "import std;\n\n// A copy, named otherwise.\nfn helper(i64 x) : i64\n{{\n    printf(\"helper %v\\n\", x);\n    x + 1\n}}\n\nfn main()\n{{\n    i64 extra = helper(41);\n{}    printf(\"%v\\n\", extra);\n}}\n",
        SHARED
    )
}

#[test]
fn std_units_are_found_again_by_another_program() {
    let root = scratch("across");
    let cache = root.join("cache");
    let (reused_a, compiled_a) = build(&root, "first", &program_a(), &cache);
    assert_eq!(reused_a, 0, "a fresh cache holds nothing");
    assert!(
        compiled_a > 0,
        "the shared `std` use lowers to cacheable units"
    );
    // Every unit the first program put there, the second finds: its
    // `std` use is a superset, so a unit of the first compiled again
    // here is one whose text depends on the program around it.
    let (reused_b, _) = build(&root, "second", &program_b(), &cache);
    assert_eq!(
        reused_b, compiled_a,
        "every `std` unit of the first program is found again by the second"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn std_units_are_found_again_by_the_same_program_renamed() {
    let root = scratch("renamed");
    let cache = root.join("cache");
    let (_, compiled_a) = build(&root, "original", &program_a(), &cache);
    // The same `std` use, in a program with a function and a literal of
    // its own: nothing to compile but the program's unit.
    let (reused, compiled) = build(&root, "copy", &program_a_again(), &cache);
    assert_eq!(
        (reused, compiled),
        (compiled_a, 0),
        "the copy reuses every unit and compiles none"
    );
    let _ = fs::remove_dir_all(&root);
}
