#!/usr/bin/env python3
"""Writes cases/tools/cli_check_no_follow_on_error.case (D-0323: `cobc
--check` reports no error that follows from another). Each program is a
`main` around one body of the table `RUNS` (the eleventh's body made by a
loop).

    python3 impl/tests/gen/cli_follow_on.py [--check]

Python 3 standard library only.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CASE = os.path.join(HERE, "..", "cases", "tools", "cli_check_no_follow_on_error.case")

NOTE = "note: statements that depend on a failed statement were not checked\\n"

# (file, body, number of `error[` lines, checks of standard error)
RUNS = [
    # A typed declaration that fails declares its local at its type.
    ("typed.cb", '    usize x = nope;\n    printf("%v\\n", x + 1);', 1, ["stderr-lacks note: statements"]),
    # `auto x = …` that fails: the uses of `x` are not checked.
    ("auto.cb", '    auto x = 3 + true;\n    printf("%v\\n", x + 1);\n    u8 z = later_nope;', 2, ["stderr-ends \\n2 errors\\n" + NOTE]),
    # A failed statement that would have given `w` a value.
    ("init.cb", '    i32 w;\n    if (arg_count() > 0) { w = bad; } else { w = 1; }\n    printf("%v\\n", w);', 1, ["stderr-ends \\n" + NOTE]),
    # After a failed `return`, nothing is checked.
    ("ret.cb", "    return nope;\n    u8 z = also_nope;", 1, []),
    # Eleven independent errors: ten shown, then the count.
    ("eleven.cb", "\n".join("    u8 v%d = nope%d;" % (i, i) for i in range(11)), 10, ["stderr-ends \\n11 errors; the first 10 are shown\\n"]),
]

HEAD = """# D-0323: what a failed statement declares or leaves unknown, and what
# follows a failed `return`, gives no second error; the report says when a
# statement was not checked; past ten errors, ten are shown and the count
# (`cobc --check`). Written by gen/cli_follow_on.py.
"""


def text():
    out = [HEAD.rstrip("\n")]
    for name, body, n, checks in RUNS:
        src = "import std;\n\nfn main()\n{\n%s\n}\n" % body
        out.append("file %s" % name)
        out.append(src[:-1])
        out.append("end")
        out.append("run $COBC --check %s" % name)
        out.append("stderr-lines %d error[" % n)
        out.extend(checks)
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
            sys.exit("%s is not current: run gen/cli_follow_on.py" % os.path.relpath(CASE))
        return
    with open(CASE, "w", encoding="utf-8") as f:
        f.write(want)


if __name__ == "__main__":
    main()
