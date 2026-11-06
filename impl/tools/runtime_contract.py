#!/usr/bin/env python3
"""The runtime's contract (D-0263): every `cb_*` function and variable the
generated C may use, as `impl/cbrt/include/cbrt.h` declares it, with the
contract its comment states — the rule steps it performs (`[Rule]` labels)
and what it assumes — written as a table into spec/IMPLEMENTATION-NOTES.md
between `<!-- runtime-contract:begin -->` and `<!-- runtime-contract:end -->`.
    python3 impl/tools/runtime_contract.py            # writes the table
    python3 impl/tools/runtime_contract.py --check    # exits 1 if the table is stale, or a
                                                      # declaration of a checking group has no rule label
A declaration's contract is the comment on its line, or the block comment
directly above it; a `/* group */` line opens a group. The groups that
perform rule steps (frames and statement scopes, objects, access paths,
reference data, raw storage, values in flight, threads and mutexes,
program) must name a rule label or say `internal`; the std externs name
the std rule they implement. Standard library only; runs on Windows.
"""
import os
import re
import sys
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
HEADER = os.path.join(ROOT, "impl", "cbrt", "include", "cbrt.h")
NOTES = os.path.join(ROOT, "spec", "IMPLEMENTATION-NOTES.md")
BEGIN = "<!-- runtime-contract:begin -->"
END = "<!-- runtime-contract:end -->"
GROUPS = ("program", "frames and statement scopes", "objects", "access paths", "reference data", "raw storage", "values in flight", "threads and mutexes", "text and hashing helpers", "bit operations", "standard library externs")
CHECKED_GROUPS = ("frames and statement scopes", "objects", "access paths", "reference data", "raw storage", "values in flight", "threads and mutexes", "program")
DECL = re.compile(r"^(?:extern |static inline |_Noreturn |__thread )*(?:[A-Za-z_][A-Za-z0-9_ ]*?[ *]+)(cb_[a-z_0-9]+)\s*(\(|\[|;|=)")
LABEL = re.compile(r"\[([A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*)\]")


def declarations():
    lines = open(HEADER, encoding="utf-8").read().split("\n")
    out = []
    group = "types and helpers"
    pending = []  # block comment lines above
    i = 0
    while i < len(lines):
        line = lines[i]
        s = line.strip()
        g = None
        if s.startswith("/*"):
            for name in GROUPS:
                if s[2:].strip().startswith(name):
                    g = name
        if g:
            group = "values in flight" if g.startswith("values in flight") else g
            while "*/" not in lines[i] and i + 1 < len(lines):
                i += 1
            pending = []
            i += 1
            continue
        if s.startswith("/*") and not re.search(r"\bcb_[a-z_0-9]+\s*\(", s.split("*/")[0] if "*/" in s else s) and not DECL.match(s):
            block = [s]
            while "*/" not in lines[i] and i + 1 < len(lines):
                i += 1
                block.append(lines[i].strip())
            pending = block
            i += 1
            continue
        d = DECL.match(s)
        if d and not s.startswith("#define") and not s.startswith("typedef"):
            name = d.group(1)
            trailing = re.findall(r"/\*(.*?)\*/", line)
            text = " ".join(t.strip() for t in trailing)
            if not text and pending:
                text = " ".join(pending).replace("/*", "").replace("*/", "").strip()
            text = re.sub(r"\s+", " ", text)
            kind = "variable" if ("extern __thread" in s or (s.startswith("extern") and "(" not in s.split(";")[0])) else "function"
            if "static inline" in s:
                kind = "inline"
            out.append((name, kind, group, text, i + 1))
            pending = [] if trailing else pending
            if "{" in s and "}" not in s:
                while i + 1 < len(lines) and not lines[i].startswith("}"):
                    i += 1
        elif s and not s.startswith("#") and not s.startswith("}") and not s.startswith("CB_ROT") and not s.startswith("typedef") and not s.startswith("}"):
            pending = []
        i += 1
    return out


def render(decls):
    L = [BEGIN, "", "| Function | Group | Contract (from `cbrt.h`) | Rules |", "|---|---|---|---|"]
    for name, kind, group, text, line in decls:
        labels = ", ".join("`[%s]`" % l for l in dict.fromkeys(LABEL.findall(text)))
        t = text.replace("|", "\\|")
        L.append("| `%s`%s | %s | %s | %s |" % (name, " (inline)" if kind == "inline" else (" (variable)" if kind == "variable" else ""), group, t if t else "—", labels if labels else "—"))
    L.append("")
    L.append(END)
    return "\n".join(L)


def defined_labels():
    """Every rule label the spec's artifacts define (as speccheck counts them)."""
    labels = set()
    spec = os.path.join(ROOT, "spec")
    for f in sorted(os.listdir(spec)):
        if not re.match(r"^[0-2]\d-.*\.md$", f):
            continue
        for line in open(os.path.join(spec, f), encoding="utf-8").read().split("\n"):
            if line.startswith("    ["):
                for lab in LABEL.findall(line.split("disposition:")[0].split("outcome:")[0]):
                    labels.add(lab)
            for m in re.finditer(r"`\[([A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*)\]`\)?\s*(?:—|:|is\b)", line):
                labels.add(m.group(1))
    return labels


def main():
    decls = declarations()
    known = defined_labels()
    unknown = [(n, lab, ln) for n, k, g, t, ln in decls for lab in LABEL.findall(t) if lab not in known]
    table = render(decls)
    notes = open(NOTES, encoding="utf-8").read()
    a, b = notes.find(BEGIN), notes.find(END)
    if a < 0 or b < 0:
        print("runtime_contract: no markers in spec/IMPLEMENTATION-NOTES.md", file=sys.stderr)
        return 1
    current = notes[a:b + len(END)]
    missing = [(n, g, ln) for n, k, g, t, ln in decls if g in CHECKED_GROUPS and not LABEL.search(t) and "internal" not in t.lower()]
    if "--check" in sys.argv:
        bad = 0
        if current != table:
            print("runtime_contract: the table in spec/IMPLEMENTATION-NOTES.md is stale: run impl/tools/runtime_contract.py")
            bad = 1
        for n, g, ln in missing:
            print("cbrt.h:%d: `%s` (%s) names no rule label and is not marked internal" % (ln, n, g))
            bad = 1
        for n, lab, ln in unknown:
            print("cbrt.h:%d: `%s` names `[%s]`, which no spec artifact defines" % (ln, n, lab))
            bad = 1
        print("runtime_contract: %d declarations, %d in checked groups, %d without a rule label" % (len(decls), sum(1 for d in decls if d[2] in CHECKED_GROUPS), len(missing)))
        return bad
    new = notes[:a] + table + notes[b + len(END):]
    if new != notes:
        open(NOTES, "w", encoding="utf-8").write(new)
    for n, lab, ln in unknown:
        print("cbrt.h:%d: `%s` names `[%s]`, which no spec artifact defines" % (ln, n, lab))
    print("runtime_contract: %d declarations written (%d in checked groups, %d without a rule label, %d unknown labels)" % (len(decls), sum(1 for d in decls if d[2] in CHECKED_GROUPS), len(missing), len(unknown)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
