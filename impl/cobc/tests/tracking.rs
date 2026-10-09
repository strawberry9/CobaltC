// What `cobc` proves about a program, seen in what it leaves for the
// runtime to track: `--report-tracking` lists, per source line, the
// tracking calls written in the generated C. Each test here pins down one
// thing the compiler leaves out (or must not leave out), so that a change
// that quietly loses the proof, or extends it where it does not hold,
// fails here rather than only in a benchmark. The behaviour of the same
// forms is checked by the conformance suite under both tools
// (`16-aggregates/field_vec_*`, `21-standard-library-semantics/
// append_every_kind_ok.cb`, `hashmap_get_mut_match_ok.cb`,
// `as_view_argument_ok.cb`).

mod oracle;

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cobc-tracking-{}-{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

// The report for `src`: the text of each listed line, keyed by its line
// number in `prog.cb`.
fn report(tag: &str, src: &str) -> std::collections::HashMap<usize, String> {
    let dir = scratch(tag);
    fs::write(dir.join("prog.cb"), src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).args(["--report-tracking", "prog.cb"]).current_dir(&dir).output().expect("run cobc");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = fs::remove_dir_all(&dir);
    let mut lines = std::collections::HashMap::new();
    for l in text.lines() {
        let Some(rest) = l.trim_start().strip_prefix("prog.cb:") else { continue };
        let n: usize = rest.split_whitespace().next().unwrap().parse().unwrap();
        lines.insert(n, l.to_string());
    }
    lines
}

fn line_of(src: &str, needle: &str) -> usize {
    src.lines().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("`{}` not in the program", needle)) + 1
}

// A `Vec` field of a local struct, used only by indexing and `Vec::len`:
// its elements are read with the bounds check alone. The same loop in a
// function that also pushes to the field keeps its checks.
#[test]
fn field_vec_indexed_only_is_untracked() {
    let src = "import std;\n\nstruct H\n{\n    Vec<i64> v;\n    i64 n;\n}\n\nfn only(usize n) : i64\n{\n    H h = H { .v = Vec::new(), .n = 0 };\n    i64 t = 0;\n    foreach (i in 0..Vec::len(&h.v))\n    {\n        t += h.v[i]; // only\n    }\n    h.n = narrow<i64>(n);\n    t + h.n\n}\n\nfn pushed(usize n) : i64\n{\n    H h = H { .v = Vec::new(), .n = 0 };\n    Vec::push(&mut h.v, narrow<i64>(n));\n    i64 t = 0;\n    foreach (i in 0..Vec::len(&h.v))\n    {\n        t += h.v[i]; // pushed\n    }\n    t\n}\n\nfn borrowed(usize n) : i64\n{\n    H h = H { .v = Vec::new(), .n = 0 };\n    i64 t = 0;\n    foreach (i in 0..Vec::len(&h.v))\n    {\n        ref<i64, shared> e = &h.v[i];\n        t += *e + h.v[i]; // borrowed\n    }\n    t + narrow<i64>(n)\n}\n\nfn main()\n{\n    printf(\"%v %v %v\\n\", only(1), pushed(2), borrowed(3));\n}\n";
    let r = report("field", src);
    assert!(!r.contains_key(&line_of(src, "// only")), "the indexed-only field is tracked: {:?}", r);
    assert!(r.contains_key(&line_of(src, "// pushed")), "a pushed field's loop is not tracked: {:?}", r);
    assert!(r.contains_key(&line_of(src, "// borrowed")), "a field whose element is borrowed is not tracked: {:?}", r);
}

// `match (HashMap::get_mut(…)) { Some(c) : *c += 1, None : … }` and the
// `foreach` over `String` keys it is fed by: no object for the match's
// `Option`, no binding object, no element path per word.
#[test]
fn counting_with_get_mut_makes_no_objects() {
    let src = "import std;\n\nfn main()\n{\n    Vec<String> words = Vec::new();\n    Vec::push(&mut words, String::from_str(\"a\"));\n    HashMap<String, u32> counts = HashMap::new();\n    foreach (w in &words)\n    {\n        match (HashMap::get_mut(&mut counts, w)) // m\n        {\n            Some(c) : *c += 1, // s\n            None    : { _ = HashMap::insert(&mut counts, String::clone(w), 1); },\n        }\n    }\n    printf(\"%v\\n\", HashMap::len(&counts));\n}\n";
    let r = report("getmut", src);
    let m = r.get(&line_of(src, "// m")).cloned().unwrap_or_default();
    for k in ["new", "bind", "recv_datum", "elem_borrow", "store_ref", "load_ref"] {
        assert!(!m.contains(&format!("{} x", k)), "the match line has `{}`: {}", k, m);
    }
    assert!(!r.contains_key(&line_of(src, "// s")), "the `Some` arm is tracked: {:?}", r);
    let f = r.get(&line_of(src, "foreach (w in &words)")).cloned().unwrap_or_default();
    assert!(!f.contains("frame_hold x2"), "each word is held: {}", f);
}

// A match on a function of the program's own returning `Option<ref<T>>`:
// the reference is received alone, with no temporary object.
#[test]
fn option_ref_result_needs_no_temporary() {
    let src = "import std;\n\nfn first(ref<Vec<i64>, exclusive> v) : Option<ref<i64, exclusive>>\n{\n    if (Vec::len(v) == 0)\n    {\n        return None;\n    }\n    Some(&mut v[0])\n}\n\nfn main()\n{\n    Vec<i64> v = Vec::new();\n    Vec::push(&mut v, 1);\n    foreach (i in 0..3)\n    {\n        match (first(&mut v)) // m\n        {\n            Some(x) : { *x += 1; *x *= 2; },\n            None    : { },\n        }\n    }\n    printf(\"%v\\n\", v[0]);\n}\n";
    let r = report("optref", src);
    let m = r.get(&line_of(src, "// m")).cloned().unwrap_or_default();
    assert!(m.contains("recv_datum_tok"), "the reference is not received alone: {}", m);
    for k in ["new", "move_to", "store_ref"] {
        assert!(!m.contains(&format!("{} x", k)), "the match line has `{}`: {}", k, m);
    }
}

// `String::append(&mut s, x)` for a local `s`: the borrow is checked and
// none is minted; `String::as_view(&t)` handed to a function that only
// reads the view: its checks, and no path, slot or object.
#[test]
fn append_and_as_view_mint_nothing() {
    let src = "import std;\n\nfn reads(StringView s) : usize\n{\n    StringView::len(s)\n}\n\nfn main()\n{\n    String out = String::new();\n    String text = String::from_str(\"abc\");\n    usize total = 0;\n    foreach (i in 0..3)\n    {\n        String::append(&mut out, i); // n\n        String::append(&mut out, \",\"); // l\n        total += reads(String::as_view(&text)); // v\n    }\n    printf(\"%v %v\\n\", String::len(&out), total);\n}\n";
    let r = report("append", src);
    for tag in ["// n", "// l", "// v"] {
        let l = r.get(&line_of(src, tag)).cloned().unwrap_or_default();
        for k in ["borrow", "borrow_range", "new", "store_ref", "absorb"] {
            assert!(!l.contains(&format!(" {} x", k)), "`{}` has `{}`: {}", tag, k, l);
        }
    }
}

// A loop over a struct's `Vec` field that hands an element on is not
// confined, nor is one in a function where a closure names the struct.
#[test]
fn field_vec_named_elsewhere_stays_tracked() {
    let src = "import std;\n\nstruct H\n{\n    Vec<i64> v;\n}\n\nfn captured() : i64\n{\n    H h = H { .v = Vec::new() };\n    i64 t = 0;\n    foreach (i in 0..Vec::len(&h.v))\n    {\n        t += h.v[i]; // c\n    }\n    auto f = [h]() : usize { Vec::len(&h.v) * 0 };\n    t + narrow<i64>(f())\n}\n\nfn main()\n{\n    printf(\"%v\\n\", captured());\n}\n";
    let r = report("captured", src);
    assert!(r.contains_key(&line_of(src, "// c")), "a captured struct's field is untracked: {:?}", r);
}

// A block's frame and statement scope are left out when nothing is bound
// or tracked in them, whatever text the block writes: a string literal is
// data. Two loops alike but for appending a literal or a `str` local get
// the same number of scope operations.
#[test]
fn string_literals_keep_no_scopes() {
    let src = "import std;\n\nfn lit(ref<String, exclusive> s)\n{\n    foreach (i in 0..3)\n    {\n        String::append(s, \",\");\n    }\n}\n\nfn var(ref<String, exclusive> s, str sep)\n{\n    foreach (i in 0..3)\n    {\n        String::append(s, sep);\n    }\n}\n\nfn main()\n{\n    String s = String::new();\n    lit(&mut s);\n    var(&mut s, \",\");\n    printf(\"%v\\n\", &s);\n}\n";
    for cc in oracle::c_compilers() {
        let dir = scratch(&format!("lit-{}", cc));
        fs::write(dir.join("prog.cb"), src).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_cobc")).args(["--cc", cc, "--keep-c", "-o", "prog", "prog.cb"]).current_dir(&dir).env("COBC_CACHE", "off").output().expect("run cobc");
        assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
        let c = fs::read_to_string(dir.join("prog.c")).unwrap();
        // The definition's body: the line naming the function that is not
        // its prototype (which ends in `;`), to the closing brace. A
        // program's own function is `static` since the object cache's
        // `Gen::localize`.
        let body = |name: &str| -> String {
            let head = c
                .lines()
                .position(|l| l.trim_start_matches("static ").starts_with(&format!("void {}(", name)) && !l.trim_end().ends_with(';'))
                .unwrap_or_else(|| panic!("{} not defined in the C", name));
            let lines: Vec<&str> = c.lines().skip(head).collect();
            let end = lines.iter().position(|l| *l == "}").expect("its end");
            lines[..end].join("\n")
        };
        let scopes = |b: &str| b.matches("cb_frame_push").count() + b.matches("cb_stmt_push").count();
        let (l, v) = (body("f_lit"), body("f_var"));
        assert_eq!(scopes(&l), scopes(&v), "[{}] the literal's loop keeps scopes:\n{}\n--- vs ---\n{}", cc, l, v);
        let _ = fs::remove_dir_all(&dir);
    }
}

// A shared reference to an element held across a loop: the loop's reads
// meet only shared paths, so they are bounds-checked only. An exclusive
// element borrow in the function keeps every access tracked.
#[test]
fn reads_beside_a_held_shared_element_ref_are_untracked() {
    let src = "import std;\n\nfn build(usize n) : Vec<i64>\n{\n    Vec<i64> v = Vec::new();\n    foreach (i in 0..n)\n    {\n        Vec::push(&mut v, narrow<i64>(i));\n    }\n    v\n}\n\nfn held(usize n) : i64\n{\n    Vec<i64> v = build(n);\n    ref<i64, shared> first = &v[0];\n    i64 t = 0;\n    foreach (i in 0..Vec::len(&v))\n    {\n        t += v[i]; // held\n    }\n    t + *first\n}\n\nfn excl(usize n) : i64\n{\n    Vec<i64> v = build(n);\n    ref<i64, exclusive> e = &mut v[0];\n    *e = 1;\n    i64 t = 0;\n    foreach (i in 1..Vec::len(&v))\n    {\n        t += v[i]; // excl\n    }\n    t\n}\n\nfn main()\n{\n    printf(\"%v %v\\n\", held(3), excl(3));\n}\n";
    let r = report("heldref", src);
    assert!(!r.contains_key(&line_of(src, "// held")), "reads beside a shared element ref are tracked: {:?}", r);
    assert!(r.contains_key(&line_of(src, "// excl")), "reads beside an exclusive element borrow are untracked: {:?}", r);
}

// A `String` local only ever handed to readers has no runtime object: the
// received object's record ends at once and the local is dropped where it
// ends. So has one also appended to by `std`'s builders (D-0196); one lent
// `&mut` to a function of the program keeps its object.
#[test]
fn read_only_string_local_has_no_object() {
    let src = "import std;\n\nfn reads(usize n) : usize\n{\n    usize t = 0;\n    foreach (i in 0..n)\n    {\n        String s = sprintf(\"%v\", i); // r\n        t += String::len(&s);\n    }\n    t\n}\n\nfn grows(usize n) : usize\n{\n    usize t = 0;\n    foreach (i in 0..n)\n    {\n        String s = sprintf(\"%v\", i); // a\n        String::append(&mut s, \"!\");\n        t += String::len(&s);\n    }\n    t\n}\n\nfn touch(ref<String, exclusive> s)\n{\n    String::append(s, \"?\");\n}\n\nfn lent(usize n) : usize\n{\n    usize t = 0;\n    foreach (i in 0..n)\n    {\n        String s = sprintf(\"%v\", i); // g\n        touch(&mut s);\n        t += String::len(&s);\n    }\n    t\n}\n\nfn main()\n{\n    printf(\"%v %v %v\\n\", reads(3), grows(3), lent(3));\n}\n";
    let r = report("objectless", src);
    let l = r.get(&line_of(src, "// r")).cloned().unwrap_or_default();
    for k in ["bind", "move_to"] {
        assert!(!l.contains(&format!("{} x", k)), "the read-only String is bound: {}", l);
    }
    let a = r.get(&line_of(src, "// a")).cloned().unwrap_or_default();
    for k in ["bind", "move_to"] {
        assert!(!a.contains(&format!("{} x", k)), "the String a builder appends to is bound: {}", a);
    }
    let g = r.get(&line_of(src, "// g")).cloned().unwrap_or_default();
    assert!(g.contains("bind x"), "the String lent to the program's function has no object: {}", g);
}

// `printf("%v", &s)` of a `String` local named by no other argument: read
// where it is formatted (no path minted), and for a local only read and
// printed, no object at all.
#[test]
fn printing_a_string_local_mints_nothing() {
    let src = "import std;\n\nfn main()\n{\n    foreach (i in 0..3)\n    {\n        String s = sprintf(\"row %v\", i); // s\n        printf(\"%v|\", &s); // p\n    }\n    String t = String::new();\n    String::append(&mut t, 1);\n    printf(\"%v\\n\", &t); // t\n    printf(\"%v %v\\n\", &t, &t); // tt\n}\n";
    let r = report("printf", src);
    let s = r.get(&line_of(src, "// s")).cloned().unwrap_or_default();
    assert!(!s.contains("bind x"), "the printed-only String is bound: {}", s);
    assert!(!r.contains_key(&line_of(src, "// p")), "printing the objectless String is tracked: {:?}", r);
    let t = r.get(&line_of(src, "// t")).cloned().unwrap_or_default();
    assert!(!t.contains(" borrow x"), "printing a String local mints a path: {}", t);
    let tt = r.get(&line_of(src, "// tt")).cloned().unwrap_or_default();
    assert!(tt.contains("borrow x2"), "the same String twice is not borrowed: {}", tt);
}

// A confined vector passed to a confining helper, or pushed onto, beside a
// plain local whose last use is that call (the local is moved into the
// call, `Scan::block`): the vector stays confined, so the helper runs its
// unchecked copy and nothing in `main` or the helper is tracked.
#[test]
fn last_use_argument_beside_a_confined_vector() {
    let src = "import std;\n\nfn mark(ref<Vec<bool>, exclusive> v, usize limit)\n{\n    foreach (m in 0..limit)\n    {\n        v[m] = false; // mark\n    }\n}\n\nfn main()\n{\n    usize n = 100;\n    Vec<bool> v = Vec::new();\n    foreach (i in 0..100)\n    {\n        Vec::push(&mut v, true); // fill\n    }\n    bool b = true;\n    Vec::push(&mut v, b); // push\n    mark(&mut v, n); // call\n    printf(\"%v\\n\", v[5]); // read\n}\n";
    let r = report("lastuse", src);
    for k in ["// mark", "// fill", "// push", "// call", "// read"] {
        assert!(!r.contains_key(&line_of(src, k)), "`{}` is tracked: {:?}", k, r);
    }
}

// D-0188: a function whose references are one exclusive and others
// shared, all to plain data and used directly, is emitted as a dispatcher
// over an unchecked `__fast` body and the checked `__slow` one; past
// `DUAL_LINES` (lower.rs) of fast body it is emitted once, checked. Both
// give the same results, and the dispatcher's in-flight assertion holds.
#[test]
fn state_and_input_gets_a_dual_body_within_the_cap() {
    let dir = scratch("dual");
    let big: String = (0..700).map(|k| format!("    a.n += widen<usize>(data[{}]);\n", k % 4)).collect();
    let src = format!("import std;\n\nstruct Acc\n{{\n    u64 sum;\n    usize n;\n}}\n\nfn feed(ref<Acc, exclusive> a, slice<u8, shared> data)\n{{\n    for (usize i = 0; i < slice_len(data); i += 1)\n    {{\n        a.sum += widen<u64>(data[i]);\n        a.n += 1;\n    }}\n}}\n\nfn feed_big(ref<Acc, exclusive> a, slice<u8, shared> data)\n{{\n{}}}\n\nfn main()\n{{\n    Vec<u8> v = Vec::filled(10, 3: u8);\n    Acc a = Acc {{ .sum = 0, .n = 0 }};\n    feed(&mut a, &v[0..$]);\n    feed_big(&mut a, &v[2..6]);\n    printf(\"%v %v\\n\", a.sum, a.n);\n}}\n", big);
    fs::write(dir.join("prog.cb"), &src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).args(["--keep-c", "-o", "prog", "prog.cb"]).current_dir(&dir).output().expect("run cobc");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let c = fs::read_to_string(dir.join("prog.c")).unwrap();
    let defines = |name: &str| c.lines().any(|l| !l.ends_with(';') && l.contains(&format!(" {}(", name)));
    assert!(defines("f_feed__fast") && defines("f_feed__slow") && defines("f_feed"), "`feed` has no dual body");
    assert!(!defines("f_feed_big__fast") && !defines("f_feed_big__slow") && defines("f_feed_big"), "`feed_big` is past the cap but has a dual body");
    let run = Command::new(dir.join("prog")).output().expect("run prog");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "30 2110\n", "stderr: {}", String::from_utf8_lossy(&run.stderr));
    let _ = fs::remove_dir_all(&dir);
}

// D-0189: `swap`, `reverse`, `insert`, `remove`, `truncate`, `clear`,
// `contains`, `index_of`, `binary_search` and `sort` keep a local vector
// confined (a literal key passed by address, no object), so a loop that
// calls them is untracked; a vector also handed to a function that may
// keep it is not.
#[test]
fn confining_calls_keep_a_local_vector_untracked() {
    let src = "import std;\n\nfn keep(ref<Vec<i64>, shared> v) : ref<Vec<i64>, shared>\n{\n    v\n}\n\nfn main()\n{\n    Vec<i64> v = Vec::new();\n    foreach (i in 0..100: i64)\n    {\n        Vec::push(&mut v, i);\n        Vec::swap(&mut v, 0, Vec::len(&v) - 1); // swap\n        if (Vec::contains(&v, &7)) // contains\n        {\n            Vec::reverse(&mut v);\n        }\n    }\n    Vec::sort(&mut v);\n    match (Vec::binary_search(&v, &50)) // search\n    {\n        Ok(k) : printf(\"%v\\n\", k),\n        Err(k) : printf(\"-%v\\n\", k),\n    }\n    Vec<i64> w = Vec::new();\n    foreach (i in 0..100: i64)\n    {\n        Vec::push(&mut w, i);\n        Vec::swap(&mut w, 0, Vec::len(&w) - 1); // kept\n    }\n    printf(\"%v\\n\", Vec::len(keep(&w)));\n}\n";
    let r = report("confining", src);
    for k in ["// swap", "// contains", "// search"] {
        assert!(!r.contains_key(&line_of(src, k)), "`{}` is tracked: {:?}", k, r);
    }
    assert!(r.contains_key(&line_of(src, "// kept")), "a vector handed to `keep` is untracked: {:?}", r);
}

// D-0189: a function whose one reference is to a struct, looping over the
// struct's `Vec` field, gets a dual body; a field also borrowed whole
// (`&g.cells` handed to a function) does not.
#[test]
fn struct_field_loop_gets_a_dual_body() {
    let dir = scratch("fielddual");
    let src = "import std;\n\nstruct G\n{\n    Vec<i64> cells;\n    i64 total;\n}\n\nfn count(ref<Vec<i64>, shared> v) : usize\n{\n    Vec::len(v)\n}\n\nfn step(ref<G, exclusive> g)\n{\n    for (usize i = 0; i < Vec::len(&g.cells); i += 1)\n    {\n        g.cells[i] = g.cells[i] + 1;\n        g.total += g.cells[i];\n    }\n}\n\nfn other(ref<G, exclusive> g)\n{\n    for (usize i = 0; i < count(&g.cells); i += 1)\n    {\n        g.total += g.cells[i];\n    }\n}\n\nfn main()\n{\n    G g = G { .cells = Vec::filled(10, 1: i64), .total = 0 };\n    step(&mut g);\n    other(&mut g);\n    printf(\"%v\\n\", g.total);\n}\n";
    fs::write(dir.join("prog.cb"), src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).args(["--keep-c", "-o", "prog", "prog.cb"]).current_dir(&dir).output().expect("run cobc");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let c = fs::read_to_string(dir.join("prog.c")).unwrap();
    let defines = |name: &str| c.lines().any(|l| !l.ends_with(';') && l.contains(&format!(" {}(", name)));
    assert!(defines("f_step__fast") && defines("f_step__slow"), "`step` has no dual body");
    assert!(!defines("f_other__fast"), "`other` hands `&g.cells` on but has a dual body");
    let run = Command::new(dir.join("prog")).output().expect("run prog");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "40\n");
    let _ = fs::remove_dir_all(&dir);
}
