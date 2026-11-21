#!/usr/bin/env python3
"""Whether the documentation keeps up with the specification (CHG-0475): run by impl/tools/pregate.py.

  - Every diagnostic of the registry (spec/registry/diagnostics.md) has a row in the language guide's table of
    diagnostics (htmlguide/generator/content_c.py, the table after "Every diagnostic has a stable name"), which
    says it lists them all.
  - Every rule label a numbered chapter of the specification defines (`[Name]` at the start of a rule's line)
    that the last push did not define is cited, as `[Name]`, in the guide's sources or the book's
    (htmlguide/generator/*.py, book/src/*.md), or is listed in impl/tools/docs_exempt.txt with the reason it
    needs no mention (a rule no program can observe, an expansion's step).
  - Every diagnostic the last push did not have is in the book's appendix B too (book/src/appendix-b-
    diagnostics.md), or in docs_exempt.txt.

"The last push" is the tree of `origin/main` (BASE=REF overrides it); without it, the new-item checks are
skipped and said so.

    python3 impl/tools/docs_coverage.py           # exit 1 and a list when something is not documented

Standard library only; runs on Windows.
"""
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
REGISTRY = "spec/registry/diagnostics.md"
GUIDE_TABLE = "htmlguide/generator/content_c.py"
BOOK_DIAGS = "book/src/appendix-b-diagnostics.md"
EXEMPT = os.path.join(HERE, "docs_exempt.txt")
LABEL_DEF = re.compile(r"^\s+\[([A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)+)\]", re.M)
DIAG_ENTRY = re.compile(r"^- ((?:`diag\.[a-z0-9-]+`(?:, )?)+) — ", re.M)


def diag_ids(text):
    out = set()
    for names in DIAG_ENTRY.findall(text):
        out.update(re.findall(r"`(diag\.[a-z0-9-]+)`", names))
    return out


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as f:
        return f.read()


def at_base(base, rel):
    r = subprocess.run(["git", "show", "%s:%s" % (base, rel)], cwd=ROOT, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else ""


def chapters():
    return sorted(f for f in os.listdir(os.path.join(ROOT, "spec")) if re.match(r"\d\d-.*\.md$", f))


def labels(texts):
    out = set()
    for t in texts:
        out.update(LABEL_DEF.findall(t))
    return out


def doc_text():
    parts = []
    for d, ext in (("htmlguide/generator", ".py"), ("book/src", ".md")):
        for f in sorted(os.listdir(os.path.join(ROOT, d))):
            if f.endswith(ext):
                parts.append(read(os.path.join(d, f)))
    return "\n".join(parts)


def exempt():
    out = {}
    if os.path.exists(EXEMPT):
        for line in open(EXEMPT, encoding="utf-8"):
            line = line.strip()
            if line and not line.startswith("#"):
                key, _, why = line.partition(" ")
                if not why.strip():
                    raise SystemExit("docs_exempt.txt: %s has no reason" % key)
                out[key.strip("[]")] = why.strip()
    return out


def main():
    problems = []
    registry = diag_ids(read(REGISTRY))
    guide = read(GUIDE_TABLE)
    start = guide.find("Every diagnostic has a stable name")
    table = guide[start:guide.find("])", start)] if start >= 0 else ""
    in_table = set(re.findall(r"`(diag\.[a-z0-9-]+)`", table))
    for d in sorted(registry - in_table):
        problems.append("%s: no row in the guide's table of diagnostics (%s)" % (d, GUIDE_TABLE))

    ex = exempt()
    base = os.environ.get("BASE", "origin/main")
    have = subprocess.run(["git", "rev-parse", "--verify", "-q", base + "^{tree}"], cwd=ROOT,
                          capture_output=True, text=True).returncode == 0
    new_labels = new_diags = set()
    if have:
        now = labels(read("spec/" + c) for c in chapters())
        then = labels(at_base(base, "spec/" + c) for c in chapters())
        new_labels = now - then
        docs = doc_text()
        for l in sorted(new_labels):
            if "[%s]" % l not in docs and l not in ex:
                problems.append("[%s]: a new rule cited nowhere in the guide or the book (or say why not in %s)"
                                % (l, os.path.relpath(EXEMPT, ROOT)))
        new_diags = registry - diag_ids(at_base(base, REGISTRY))
        book = read(BOOK_DIAGS)
        for d in sorted(new_diags):
            if "`%s`" % d not in book and d not in ex:
                problems.append("%s: a new diagnostic missing from the book's appendix B (%s)" % (d, BOOK_DIAGS))
    for p in problems:
        print("docs_coverage: " + p)
    print("docs_coverage: %d diagnostics, %d in the guide's table; %s; %d problem(s)" % (
        len(registry), len(registry & in_table),
        "%d rule(s) and %d diagnostic(s) new since %s" % (len(new_labels), len(new_diags), base) if have
        else "no %s here: new rules and diagnostics not checked" % base, len(problems)))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
