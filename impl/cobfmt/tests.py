#!/usr/bin/env python3
"""The formatter's tests: the layout rules on texts given to `cobfmt -`, and the
command line.

    python3 -I impl/cobfmt/tests.py [COBFMT]

COBFMT (default: impl/target/release/cobfmt, the CobaltC formatter
impl/build.sh installs there). Python 3's standard library only. Exit status 0
when every test passes, 1 otherwise; each failure is printed with what was
expected and what came.
"""

import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
COBFMT = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else os.path.join(ROOT, "impl", "target", "release", "cobfmt")
SCRATCH = tempfile.mkdtemp(prefix="cobfmt-tests-")

FAILS = []


class Out:
    def __init__(self, p):
        self.status = p.returncode
        self.stdout = p.stdout.decode("utf-8", "replace")
        self.stderr = p.stderr.decode("utf-8", "replace")


def cobfmt(args, stdin=None, cwd=None):
    p = subprocess.run([COBFMT] + list(args), input=(stdin.encode("utf-8") if stdin is not None else None),
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=cwd, timeout=600)
    return Out(p)


def f(src):
    o = cobfmt(["-"], stdin=src)
    if o.status != 0:
        raise AssertionError("cobfmt - failed (%d): %s" % (o.status, o.stderr))
    return o.stdout


def eq(got, want, what=""):
    if got != want:
        raise AssertionError("%s\nexpected: %r\n     got: %r" % (what, want, got))


def ok(cond, what):
    if not cond:
        raise AssertionError(what)


def fixed(src):
    eq(f(src), src, "expected no change")


def scratch(name):
    d = os.path.join(SCRATCH, "cobfmt-" + name)
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    return d


def read(p):
    with open(p, encoding="utf-8", newline="") as fh:
        return fh.read()


def write(p, text):
    with open(p, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)


TESTS = []


def test(fn):
    TESTS.append(fn)
    return fn


# ---- the layout ----

@test
def allman_braces_and_else():
    eq(f("fn main() {\n    if (c) {\n        a();\n    } else {\n        b();\n    }\n}\n"),
       "fn main()\n{\n    if (c)\n    {\n        a();\n    }\n    else\n    {\n        b();\n    }\n}\n")


@test
def indentation_is_four_spaces_per_block():
    eq(f("fn main()\n{\nif (c)\n{\nx = 1;\n}\n}\n"), "fn main()\n{\n    if (c)\n    {\n        x = 1;\n    }\n}\n")


@test
def function_bodies_are_always_allman():
    eq(f("fn f() : i32 { 1 }\n"), "fn f() : i32\n{\n    1\n}\n")
    eq(f("fn main() {}\n"), "fn main()\n{\n}\n")
    # Code after the `{`, the body continuing on later lines.
    eq(f("fn f() : i32 { i32 x = 1;\n    x + 1\n}\n"), "fn f() : i32\n{\n    i32 x = 1;\n    x + 1\n}\n")
    eq(f("fn f()\n{ g();\n}\n"), "fn f()\n{\n    g();\n}\n")
    # The same for a statement block whose body continues below.
    eq(f("fn main()\n{\n    if (c) { x = 1;\n        y();\n    }\n}\n"),
       "fn main()\n{\n    if (c)\n    {\n        x = 1;\n        y();\n    }\n}\n")
    eq(f("fn apply(fn(i32) : i32 g, i32 x) : i32 { g(x) }   // calls g\n"),
       "fn apply(fn(i32) : i32 g, i32 x) : i32            // calls g\n{\n    g(x)\n}\n")
    eq(f("module m\n{\n    export fn f() : i32 { 2 }\n}\n"), "module m\n{\n    export fn f() : i32\n    {\n        2\n    }\n}\n")
    # A closure, and a variable of a `fn` type set to one, keep their layout.
    fixed("fn main()\n{\n    fn(i32) : i32 g = [](i32 x) { x + 1 };\n    auto h = [](i32 x) { x };\n}\n")


@test
def one_statement_per_line():
    eq(f("fn main()\n{\n    i32 a = 1; i32 b = 2;   // two\n    g();\n}\n"),
       "fn main()\n{\n    i32 a = 1;\n    i32 b = 2;              // two\n    g();\n}\n")
    # A block's `}` after code goes on its own line.
    eq(f("fn main()\n{\n    while (c)\n    {\n        a += 1; }\n}\n"),
       "fn main()\n{\n    while (c)\n    {\n        a += 1;\n    }\n}\n")
    # Kept: a `for` header, a one-line block, a one-line declaration.
    fixed("fn main()\n{\n    for (i32 i = 0; i < 3; i += 1) { a(); b(); }\n    if (c) { x = 1; y = 2; }\n}\n")
    fixed("bitstruct B : u8 { lo : 4; hi : 4; }\nstruct P { i32 a; i32 b; }\n")


@test
def one_line_blocks_stay():
    fixed("fn main()\n{\n    u64 v = if (c) { 1 } else { 0 };\n    if (n == 0) { return; }\n}\n")
    eq(f("fn main()\n{\n    if (n == 0) {return;}\n}\n"), "fn main()\n{\n    if (n == 0) { return; }\n}\n")


@test
def spacing_rules():
    eq(f("fn main()\n{\n    x=a+b*c;\n}\n"), "fn main()\n{\n    x = a + b * c;\n}\n")
    eq(f("fn main()\n{\n    f( a ,b );\n}\n"), "fn main()\n{\n    f(a, b);\n}\n")
    eq(f("fn main()\n{\n    if(c) { x = - y; }\n}\n"), "fn main()\n{\n    if (c) { x = -y; }\n}\n")
    eq(f("fn main()\n{\n    foreach (x in & v) { g(* x); }\n}\n"), "fn main()\n{\n    foreach (x in &v) { g(*x); }\n}\n")
    eq(f("fn f(): u8\n{\n    0\n}\n"), "fn f() : u8\n{\n    0\n}\n")
    fixed("fn main()\n{\n    Vec::push(&mut v, x);\n    u8 c = b[i] & 0xC0;\n    i32 d = a - b;\n}\n")
    fixed("fn f<T: clone>(ref<Vec<T>, shared> v) : usize\n{\n    Vec::len(v)\n}\n")


@test
def struct_literals_keep_their_layout():
    fixed("fn f() : R\n{\n    R { .a = 1,\n        .b = 2 }\n}\n")
    fixed("fn f() : R\n{\n    R {\n        .a = 1,\n        .b = 2,\n    }\n}\n")


@test
def continuations_keep_their_offset():
    fixed("fn main()\n{\n    printf(\"%d %d\\n\",\n        a, b);\n}\n")
    fixed("fn main()\n{\n    u32 d = if (a) { 1 }\n        else if (b) { 2 }\n        else { 3 };\n}\n")


@test
def closures_and_value_blocks_are_allman_at_the_statement():
    fixed("fn main()\n{\n    Vec::sort_by(&mut v, [](ref<i64, shared> a, ref<i64, shared> b) : bool\n    {\n        *a < *b\n    });\n}\n")
    fixed("fn main()\n{\n    u64 x = if (c)\n    {\n        1\n    }\n    else\n    {\n        2\n    };\n}\n")


@test
def match_arm_on_the_next_line():
    fixed("fn f() : bool\n{\n    match (a)\n    {\n        Some(x) :\n            match (b)\n            {\n                Some(y) : x == y,\n"
          "                None    : false,\n            },\n        None    : false,\n    }\n}\n")


@test
def comments_kept_and_trailing_groups_aligned():
    fixed("// a header\nfn main()\n{\n    // a comment\n    x = 1;      // one\n    yy = 22;    // two\n}\n")
    # A group whose column the code now overruns moves together.
    eq(f("fn main()\n{\n    x  =  1;  // one\n    y = 2;    // two\n}\n"),
       "fn main()\n{\n    x = 1;    // one\n    y = 2;    // two\n}\n")
    fixed("fn main()\n{\n    /* a block\n       comment */\n    x = 1;\n}\n")


@test
def bitstruct_field_colons_align():
    eq(f("bitstruct Double : u64\n{\n    mantissa : 52;\n    exponent : 11;\n    sign : 1;\n}\n"),
       "bitstruct Double : u64\n{\n    mantissa : 52;\n    exponent : 11;\n    sign     : 1;\n}\n")
    # One line: left alone.
    fixed("bitstruct B : u8 { lo : 4; hi : 4; }\n")


@test
def match_arm_colons_align():
    eq(f("fn f(Option<i32> o) : i32\n{\n    match (o)\n    {\n        Some(v) : v,\n        None : 0,\n    }\n}\n"),
       "fn f(Option<i32> o) : i32\n{\n    match (o)\n    {\n        Some(v) : v,\n        None    : 0,\n    }\n}\n")
    # Arms whose value starts on the next line align too; a nested match
    # aligns on its own; a one-line match is left alone.
    fixed("fn f(Option<i32> o) : i32\n{\n    match (o)\n    {\n        Some(v) :\n        {\n            match (v) { 1 : 2, _ : 3, }\n"
          "        },\n        None    : 0,\n    }\n}\n")
    fixed("fn f(Option<i32> o) : i32\n{\n    match (o)\n    {\n        Some(v)      : v,           // the payload\n        None         : 0,\n    }\n}\n"
          .replace("v)      :", "v) :").replace("None         :", "None    :"))


@test
def blank_lines():
    eq(f("\n\nfn a()\n{\n\n    x();\n\n}\n\n\n\nfn b()\n{\n}\n\n\n"), "fn a()\n{\n    x();\n}\n\nfn b()\n{\n}\n")


@test
def crlf_and_bom_kept():
    src = "﻿fn main()\r\n{\r\n  x=1;\r\n}\r\n"
    eq(f(src), "﻿fn main()\r\n{\r\n    x = 1;\r\n}\r\n")


@test
def trailing_whitespace_removed_and_tabs_become_spaces():
    eq(f("fn main()   \n{\n\tx = 1;  \n}\n"), "fn main()\n{\n    x = 1;\n}\n")


@test
def idempotent_on_messy_input():
    messy = "import std;\nfn main(){ if(a){x=1;}else{ y=2; }\nforeach(i in 0..n){ printf(\"%v\",i ) ;}\n  }\n"
    once = f(messy)
    eq(f(once), once)


# ---- the command line ----

MESSY = "import std;\nfn main() {\n  i32 x=1+2;\n  if (x>2) { printf(\"%v\\n\",x); }\n}\n"
TIDY = "import std;\nfn main()\n{\n    i32 x = 1 + 2;\n    if (x > 2) { printf(\"%v\\n\", x); }\n}\n"


@test
def no_arguments_prints_the_usage_and_exits_2():
    o = cobfmt([])
    eq(o.status, 2)
    ok("usage: cobfmt" in o.stderr and "--backup" in o.stderr, o.stderr)


@test
def help_and_version():
    o = cobfmt(["--help"])
    eq(o.status, 0)
    for flag in ["--help", "--backup", "--check", "--diff", "--stdout", "--width", "--no-wrap", "--quiet", "--version", "exit status"]:
        ok(flag in o.stdout, "--help lacks " + flag)
    v = cobfmt(["--version"])
    eq(v.status, 0)
    ok(v.stdout.startswith("cobfmt "), v.stdout)


@test
def unknown_option_is_a_usage_error():
    o = cobfmt(["--frobnicate", "x.cb"])
    eq(o.status, 2)
    ok("unknown option" in o.stderr, o.stderr)


@test
def formats_in_place():
    d = scratch("inplace")
    p = os.path.join(d, "a.cb")
    write(p, MESSY)
    o = cobfmt([p])
    eq(o.status, 0, o.stderr)
    eq(read(p), TIDY)
    # Already formatted: nothing changes, still 0.
    o = cobfmt([p])
    eq(o.status, 0)
    eq(read(p), TIDY)


@test
def backup_keeps_the_previous_contents_and_never_overwrites():
    d = scratch("backup")
    p = os.path.join(d, "a.cb")
    # The author's own backup must not be touched.
    write(os.path.join(d, "a.cb.bak"), "mine\n")
    write(p, MESSY)
    eq(cobfmt(["--backup", p]).status, 0)
    eq(read(os.path.join(d, "a.cb.format.bak")), MESSY)
    eq(read(os.path.join(d, "a.cb.bak")), "mine\n")
    eq(read(p), TIDY)
    # A second change: the next free name.
    write(p, "fn main() {\n}\n")
    eq(cobfmt(["--backup", p]).status, 0)
    eq(read(os.path.join(d, "a.cb.format.bak.1")), "fn main() {\n}\n")
    eq(read(os.path.join(d, "a.cb.format.bak")), MESSY)
    # Unchanged files get no backup.
    eq(cobfmt(["--backup", p]).status, 0)
    ok(not os.path.exists(os.path.join(d, "a.cb.format.bak.2")), "a backup of an unchanged file")
    # No temporary file is left behind.
    ok(not [e for e in os.listdir(d) if ".tmp" in e], "a temporary file was left")


@test
def check_and_diff_write_nothing():
    d = scratch("check")
    p = os.path.join(d, "a.cb")
    write(p, MESSY)
    o = cobfmt(["--check", p])
    eq(o.status, 1)
    ok("a.cb" in o.stdout, o.stdout)
    o = cobfmt(["--diff", p])
    eq(o.status, 1)
    ok("+    i32 x = 1 + 2;" in o.stdout and "-  i32 x=1+2;" in o.stdout, o.stdout)
    eq(read(p), MESSY)
    write(p, TIDY)
    eq(cobfmt(["--check", p]).status, 0)
    eq(cobfmt(["--diff", p]).status, 0)


@test
def standard_input_with_a_dash():
    o = cobfmt(["-"], stdin=MESSY)
    eq(o.status, 0)
    eq(o.stdout, TIDY)
    o = cobfmt(["--check", "-"], stdin=MESSY)
    eq(o.status, 1)


@test
def stdout_leaves_the_file():
    d = scratch("stdout")
    p = os.path.join(d, "a.cb")
    write(p, MESSY)
    o = cobfmt(["--stdout", p])
    eq(o.status, 0)
    eq(o.stdout, TIDY)
    eq(read(p), MESSY)


@test
def a_file_that_does_not_parse_is_reported_and_left_alone():
    d = scratch("noparse")
    p = os.path.join(d, "bad.cb")
    src = "fn main() {\n    i32 x = ;\n}\n"
    write(p, src)
    o = cobfmt([p])
    eq(o.status, 2)
    ok("does not parse" in o.stderr and "bad.cb:2" in o.stderr, o.stderr)
    eq(read(p), src)


@test
def directories_skip_hidden_and_target():
    d = scratch("tree")
    for sub in ["src", "src/inner", ".git", "target"]:
        os.makedirs(os.path.join(d, sub), exist_ok=True)
    for name in ["src/a.cb", "src/inner/b.cb", ".git/c.cb", "target/d.cb"]:
        write(os.path.join(d, name), MESSY)
    write(os.path.join(d, "src/notes.txt"), "x")
    o = cobfmt(["--quiet", d])
    eq(o.status, 0)
    eq(read(os.path.join(d, "src/a.cb")), TIDY)
    eq(read(os.path.join(d, "src/inner/b.cb")), TIDY)
    eq(read(os.path.join(d, ".git/c.cb")), MESSY)
    eq(read(os.path.join(d, "target/d.cb")), MESSY)


@test
def width_breaks_a_long_call_one_argument_per_line():
    src = "import std;\nfn main()\n{\n    printf(\"%v %v %v\\n\", 111111111, 222222222, 333333333);\n}\n"
    o = cobfmt(["--width", "40", "-"], stdin=src)
    eq(o.status, 0, o.stderr)
    out = o.stdout
    eq(out, "import std;\nfn main()\n{\n    printf(\n        \"%v %v %v\\n\",\n        111111111,\n        222222222,\n        333333333,\n    );\n}\n")
    # Formatting it again, with the same width, changes nothing.
    eq(cobfmt(["--width", "40", "-"], stdin=out).stdout, out)
    # No line is broken by default.
    eq(cobfmt(["-"], stdin=src).stdout, src)


# The owner, 2026-10-01 and again 2026-11-01: no line is broken unless
# `--width N` asks for it; `--no-wrap` is that default, spelled out.
@test
def no_line_is_broken_by_default_and_width_breaks():
    args = ["%d" % (1000000000 + i) for i in range(12)]
    src = "import std;\nfn main()\n{\n    printf(\"%%v\\n\", %s);\n}\n" % ", ".join(args)
    ok(any(len(l) > 120 for l in src.split("\n")), "the test's line is not long")
    eq(cobfmt(["-"], stdin=src).stdout, src)
    eq(cobfmt(["--no-wrap", "-"], stdin=src).stdout, src)
    out = cobfmt(["--width", "120", "-"], stdin=src).stdout
    ok(all(len(l) <= 120 for l in out.split("\n")), out)
    ok("    printf(\n" in out, out)
    # The last of `--width` and `--no-wrap` wins.
    eq(cobfmt(["--width", "120", "--no-wrap", "-"], stdin=src).stdout, src)


# A type's arguments stay together when a parameter list is broken.
@test
def breaking_keeps_type_arguments_together():
    src = "fn check(str name, slice<u8, shared> salt, slice<u8, shared> ikm, ref<HashMap<String, Vec<u8>>, shared> m, usize len, str w)\n{\n}\n"
    eq(cobfmt(["--width", "60", "-"], stdin=src).stdout,
       "fn check(\n    str name,\n    slice<u8, shared> salt,\n    slice<u8, shared> ikm,\n    ref<HashMap<String, Vec<u8>>, shared> m,\n"
       "    usize len,\n    str w,\n)\n{\n}\n")


@test
def crlf_round_trips():
    d = scratch("crlf")
    p = os.path.join(d, "w.cb")
    write(p, MESSY.replace("\n", "\r\n"))
    eq(cobfmt([p]).status, 0)
    eq(read(p), TIDY.replace("\n", "\r\n"))


# The style is the corpus's: these are already formatted.
@test
def the_showcases_examples_and_benchmarks_are_already_formatted():
    for d in ["showcase", "impl/cobaltc_examples", "bench"]:
        o = cobfmt(["--check", os.path.join(ROOT, d)])
        eq(o.status, 0, "%s has files cobfmt would change:\n%s%s" % (d, o.stdout, o.stderr))


# The conformance cases are formatted too; syntax-error cases are not
# formatted (exit 2) by design.
@test
def the_conformance_cases_are_already_formatted():
    o = cobfmt(["--check", os.path.join(ROOT, "impl", "conformance")])
    ok(not o.stdout.strip(), "files cobfmt would change:\n" + o.stdout)
    for line in o.stderr.split("\n"):
        ok(not line or "does not parse" in line, "unexpected error: " + line)


# D-0229: a `///` block stays directly before the item it documents.
@test
def doc_comments_stay_attached_to_their_item():
    messy = "import std;\n\n/// Adds one.\n///\n/// Overflow faults, as `+` does.\nfn inc(i32 x) : i32 { x+1 }\n\n/// The program.\nfn main() {\n  printf(\"%v\\n\",inc(1));\n}\n"
    o = cobfmt(["-"], stdin=messy)
    eq(o.status, 0, o.stderr)
    ok("/// Adds one.\n///\n/// Overflow faults, as `+` does.\nfn inc(i32 x) : i32\n" in o.stdout, o.stdout)
    ok("/// The program.\nfn main()\n{\n" in o.stdout, o.stdout)


# D-0303: an attribute that begins a line goes on a line of its own.
@test
def attributes_stand_on_their_own_line():
    messy = ("import std;\n\n/// Adds.\n[[deprecated(\"use g\")]] fn f() : i32 { 1 }\n\nstruct P\n{\n    [[deprecated(\"x\")]] i32 a;\n}\n\n"
             "[[removed(\"gone\")]]   export   fn old ;\n\nfn main()\n{\n}\n")
    o = cobfmt(["-"], stdin=messy)
    eq(o.status, 0, o.stderr)
    ok("/// Adds.\n[[deprecated(\"use g\")]]\nfn f() : i32\n{\n    1\n}\n" in o.stdout, o.stdout)
    ok("{\n    [[deprecated(\"x\")]]\n    i32 a;\n}\n" in o.stdout, o.stdout)
    ok("\n[[removed(\"gone\")]]\nexport fn old;\n" in o.stdout, o.stdout)


@test
def unary_operators_spaced_after_binary_ones():
    # Report 094: `x = -1`, `= &x`, `&& *r`, a comparison beside a unary
    # operator, `>>` before a literal, and a statement after a one-line
    # block's `}` on a line of its own.
    eq(f("fn main()\n{\n    i32 x =-1;\n    auto r =&x;\n    u32 s = 1 << 3>>1;\n    bool c = x<*r &&*r>x;\n"
         "    foreach (i in 0..2) { g(i); }h();\n}\n"),
       "fn main()\n{\n    i32 x = -1;\n    auto r = &x;\n    u32 s = 1 << 3 >> 1;\n    bool c = x < *r && *r > x;\n"
       "    foreach (i in 0..2) { g(i); }\n    h();\n}\n")
    # Types keep their angle brackets.
    fixed("fn main()\n{\n    Vec<Vec<i32>> v = Vec::new();\n    array<u8, 4> a = [1, 2, 3, 4];\n}\n")


@test
def width_leaves_foreach_bindings_and_captures():
    # Report 143: neither takes a trailing comma, so neither is broken.
    src = ("fn main()\n{\n    foreach (position, measurement in &measurements)\n    {\n        g(position, measurement);\n    }\n"
           "    auto k = [first_value, second_value](i32 k) : i32 { first_value + k };\n}\n")
    o = cobfmt(["--width", "40", "-"], stdin=src)
    eq(o.status, 0, o.stderr)
    ok("foreach (position, measurement in &measurements)\n" in o.stdout, o.stdout)
    ok("[first_value, second_value](" in o.stdout, o.stdout)


@test
def one_line_body_keeps_comment_order():
    # Report 160: a block comment in the body and a line comment after it.
    eq(f("fn twice(i32 a) : i32 { a * /* doubled */ 2 } // one-liner\n"),
       "fn twice(i32 a) : i32\n{\n    a * /* doubled */ 2\n}                                             // one-liner\n")


@test
def closure_in_a_struct_literal_field():
    # Report 179: the closure's braces at its field's indentation.
    o = cobfmt(["-"], stdin="fn main()\n{\n    Vec::push(&mut table, Route {\n        .name = \"double\",\n"
               "        .handler = [](i32 x) : i32 {\n            x * 2\n        },\n    });\n}\n")
    eq(o.status, 0, o.stderr)
    ok("        .handler = [](i32 x) : i32\n        {\n            x * 2\n        },\n" in o.stdout, o.stdout)


def main():
    if not os.path.isfile(COBFMT):
        print("tests.py: no formatter at %s (impl/build.sh installs it)" % COBFMT)
        return 1
    passed = 0
    for t in TESTS:
        try:
            t()
            passed += 1
        except Exception as e:  # noqa: BLE001 -- every failure is reported
            FAILS.append(t.__name__)
            print("FAILED %s: %s" % (t.__name__, e))
    shutil.rmtree(SCRATCH, ignore_errors=True)
    print("cobfmt tests: %d passed, %d failed" % (passed, len(FAILS)))
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
