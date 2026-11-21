#!/usr/bin/env python3
"""Every spec conformance case is referenced by a test: every `conf.*` case
of spec/conformance.md (a table row beginning "| `conf.") must be referenced,
by its id, somewhere in the suites: as a `conf.<id>` word in a program case
under impl/tests/programs/ (a `facility:` or `// covers:` line; COVERS.txt
there lists the ids no case's own line names) or in a file-based case under
impl/conformance/ (its `facility:` or description lines), or as a word
`conf_<id with underscores>` / `ex_e2e_<...>` in one of those files. A new
spec case without a test fails here, by name. More than 100 ids must be read
from the table.

    python3 impl/tests/checks/conf_coverage.py [--repo DIR]

Exit status 0 when every case is referenced, 1 when one is not. Python 3
standard library only.
"""
import os
import re
import sys


def main():
    repo = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
    args = sys.argv[1:]
    if "--repo" in args:
        repo = args[args.index("--repo") + 1]
    with open(os.path.join(repo, "spec", "conformance.md"), encoding="utf-8") as f:
        spec = f.read()
    ids = set()
    for line in spec.split("\n"):
        if line.startswith("| `conf."):
            rest = line[len("| `conf."):]
            end = rest.find("`")
            if end >= 0:
                ids.add("conf." + rest[:end])
    if len(ids) <= 100:
        print("conf_coverage: parsed only %d case ids from spec/conformance.md" % len(ids))
        return 1
    corpus = []
    # The program cases (D-0326): each names the rows it covers, and COVERS.txt the ids that no case's own
    # `// covers:` line names.
    for d, _, files in os.walk(os.path.join(repo, "impl", "tests", "programs")):
        for name in sorted(files):
            if name.endswith((".cb", ".txt")):
                with open(os.path.join(d, name), encoding="utf-8", errors="replace") as f:
                    corpus.append(f.read())
                corpus.append("\n")
    for d, _, files in os.walk(os.path.join(repo, "impl", "conformance")):
        for name in sorted(files):
            if name.endswith(".cb"):
                with open(os.path.join(d, name), encoding="utf-8", errors="replace") as f:
                    corpus.append(f.read())
                corpus.append("\n")
    mentioned = set()
    for m in re.split(r"[^A-Za-z0-9._-]", "".join(corpus)):
        if m.startswith("conf."):
            mentioned.add(m)
        elif m.startswith("conf_"):
            mentioned.add("conf." + m[len("conf_"):].replace("_", "-"))
        elif m.startswith("ex_e2e_"):
            mentioned.add("conf.e2e-" + m[len("ex_e2e_"):].replace("_", "-"))
    missing = sorted(i for i in ids if i not in mentioned)
    if missing:
        print("conf_coverage: spec/conformance.md cases with no test referencing them: %s" % ", ".join(missing))
        return 1
    print("conf_coverage: all %d cases of spec/conformance.md are referenced" % len(ids))
    return 0


if __name__ == "__main__":
    sys.exit(main())
