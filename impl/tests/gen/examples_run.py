#!/usr/bin/env python3
"""Writes cases/tools/examples_run.case: every program in
impl/cobaltc_examples/ runs to completion (`cobc --run`, under each C
compiler), and one with tests passes them. A file without a root `fn main`
(a module body such as 13_geometry.cb) is not a program and is skipped; the
file that declares it exercises it, so every `.cb` file of the directory is
copied. The case lists the files this script found, so run it again when an
example is added (`--check` fails when the case file is not what it would
write).

    python3 impl/tests/gen/examples_run.py [--check]

Python 3 standard library only.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
CASE = os.path.join(HERE, "..", "cases", "tools", "examples_run.case")
EXAMPLES = "impl/cobaltc_examples"


def text():
    names = sorted(f for f in os.listdir(os.path.join(REPO, EXAMPLES)) if f.endswith(".cb"))
    if not names:
        sys.exit("no examples under %s" % EXAMPLES)
    out = [
        "# Every program in impl/cobaltc_examples/ runs to completion, and one with",
        "# tests (a line that begins `unittest`) passes them (D-0230: the report's",
        "# last line counts no failure); under each C compiler. A file without a",
        "# root `fn main` (a module body such as 13_geometry.cb) is not a program",
        "# and is skipped; the file that declares it exercises it. Written by",
        "# gen/examples_run.py, which lists the directory's files; run it again when",
        "# one is added (`--check` says whether this file is current).",
    ]
    for n in names:
        out.append("copy %s/%s %s" % (EXAMPLES, n, n))
    ran = 0
    for n in names:
        with open(os.path.join(REPO, EXAMPLES, n), encoding="utf-8") as f:
            lines = f.read().split("\n")
        if not any(l.startswith("fn main") for l in lines):
            continue
        ran += 1
        out.append("run $COBC --cc $CC --run %s" % n)
        out.append("status 0")
        if any(l.startswith("unittest") for l in lines):
            out.append("run $COBC --unittest --cc $CC --run %s" % n)
            out.append("status 0")
            out.append("stdout-ends \\spassed, 0 failed\\n")
    if ran == 0:
        sys.exit("no example program under %s" % EXAMPLES)
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
            sys.exit("%s is not current: run gen/examples_run.py" % os.path.relpath(CASE))
        return
    with open(CASE, "w", encoding="utf-8") as f:
        f.write(want)


if __name__ == "__main__":
    main()
