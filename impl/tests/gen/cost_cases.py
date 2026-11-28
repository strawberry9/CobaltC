#!/usr/bin/env python3
"""Writes cases/cobc/driver_cost_cases_decided.case (D-0281 §6's cost
cases): every conformance case whose header says `decided: yes` is decided,
so `cobc` lowers it keeping no record of references (no C is compiled:
`--cc /bin/true`). The case lists the files this script found under
impl/conformance, so run it again when a cost case is added (`--check` fails
when the case file is not what it would write).

    python3 impl/tests/gen/cost_cases.py [--check]

Python 3 standard library only.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
CASE = os.path.join(HERE, "..", "cases", "cobc", "driver_cost_cases_decided.case")


def cost_cases():
    root = os.path.join(REPO, "impl", "conformance")
    found = []
    for d, _, files in os.walk(root):
        for f in files:
            if not f.endswith(".cb"):
                continue
            p = os.path.join(d, f)
            with open(p, encoding="utf-8", errors="replace") as fh:
                src = fh.read()
            head = []
            for line in src.split("\n"):
                if not line.startswith("//"):
                    break
                head.append(line)
            if any(l.lstrip("/").strip() == "decided: yes" for l in head):
                found.append(os.path.relpath(p, REPO).replace(os.sep, "/"))
    return sorted(found)


def text():
    cases = cost_cases()
    if len(cases) < 10:
        sys.exit("only %d cost cases" % len(cases))
    out = [
        "# D-0281 §6's cost cases: every conformance case whose header says",
        "# `decided: yes` is decided, so `cobc` lowers it keeping no record of",
        "# references (no C is compiled). Written by gen/cost_cases.py, which lists",
        "# the cases it finds under impl/conformance (at least 10); run it again",
        "# when one is added (`--check` says whether this file is current).",
        "platform unix",
    ]
    names = set()
    for k, rel in enumerate(cases):
        name = os.path.basename(rel)
        if name in names:
            name = "%d_%s" % (k, name)
        names.add(name)
        out.append("copy %s %s" % (rel, name))
        out.append("run $COBC -v --cc /bin/true -o out %s" % name)
        out.append("stderr-has lowered pathless and decided (D-0284, D-0285)")
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
            sys.exit("%s is not current: run gen/cost_cases.py" % os.path.relpath(CASE))
        return
    with open(CASE, "w", encoding="utf-8") as f:
        f.write(want)


if __name__ == "__main__":
    main()
