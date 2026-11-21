#!/usr/bin/env python3
"""Writes cases/cobc/extern_code_errors.case: each error of an
`extern "…";` declaration is reported at the declaration. Each program is
made from a row of the table `ERRORS` (declaration, call, exit status,
fragments of standard error) and written into a directory of its own.

    python3 impl/tests/gen/extern_errors.py [--check]

Python 3 standard library only.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CASE = os.path.join(HERE, "..", "cases", "cobc", "extern_code_errors.case")

ERRORS = [
    ('extern "./absent.c";', "", 2, ['cobc: extern "./absent.c": no such file', "at "]),
    ('extern "./main.cb";', "", 2, ["not a .c, .S, .s, .o, .a or .so file"]),
    ('extern "-lfoo";', "", 2, ["neither a library name"]),
    ('extern "lib/x.a";', "", 2, ["neither a library name"]),
    ('extern "surely_no_such_library_xyz";', "", 2, ["the C compiler found no library `libsurely_no_such_library_xyz`"]),
    ('extern "./broken.c";', "", 2, ["failed on"]),
    ('extern "./ok.c";\nunsafe extern fn missing(i64 x) : i64;', "    unsafe { missing(1); }", 3, ["unsupported: no C function `missing` in libc, libm or the program's C code"]),
]

HEAD = """# `extern "…";` (spec/20 `rule.trust.extern-code`, D-0030) as `cobc` reads it:
# each error reported at its declaration (`main.cb:3`, but for a C file that
# fails to compile), with its exit status, under both C compilers. Each
# program is run from the case's directory, not its own. Written by
# gen/extern_errors.py from its table.
"""


def esc(s):
    s = s.replace("\\", "\\\\").replace("\n", "\\n")
    # A space that ends the text: `\s`, which the case's trimming keeps.
    return s[:-1] + "\\s" if s.endswith(" ") else s


def text():
    out = [HEAD.rstrip("\n")]
    for i, (decl, call, code, frags) in enumerate(ERRORS):
        d = "e%d" % i
        out.append("file %s/broken.c" % d)
        out.append("int f(void) { return 1 }")
        out.append("end")
        out.append("file %s/ok.c" % d)
        out.append("int ok(void) { return 1; }")
        out.append("end")
        src = "import std;\n\n%s\n\nfn main()\n{\n%s\n}\n" % (decl, call)
        out.append("file %s/main.cb" % d)
        out.append(src[:-1])
        out.append("end")
        out.append("run $COBC --cc $CC --run $DIR/%s/main.cb" % d)
        out.append("status %d" % code)
        for f in frags:
            out.append("stderr-has %s" % esc(f))
        if code == 2 and "broken" not in decl:
            out.append("stderr-has main.cb:3")
    return "\n".join(out) + "\n"


def main():
    want = text()
    if "--check" in sys.argv[1:]:
        try:
            with open(CASE, encoding="utf-8") as f:
                have = f.read()
        except OSError:
            have = None
        if have != want:
            sys.exit("%s is not current: run gen/extern_errors.py" % os.path.relpath(CASE))
        return
    with open(CASE, "w", encoding="utf-8") as f:
        f.write(want)


if __name__ == "__main__":
    main()
