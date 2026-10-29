#!/usr/bin/env python3
"""Which conformance cases exercise each exported `std` item (WI-6, CHG-0265).

Reads every exported item of impl/std/**/*.cb -- functions, associated
functions, structs, enums and their variants -- and every program the
conformance suite holds (each case file under impl/conformance/, and each
inline fragment of a spec/conformance.md row), and reports for each item
the cases that name it:

  - a free function by `name(`, an associated one by `Type::name(` (or
    `Type::name` passed as a value);
  - a struct or enum by its name;
  - a variant by its bare name or `Enum::Variant`;
  - a destructor, `Type::drop`, by its type: it runs wherever a value of
    the type ends, and is never written.

An item no case names is *uncovered*. An item may be exempted where it is
declared, by a comment line directly above it in the std source:

    // coverage: exempt <reason>

It also reports case files that no row of spec/conformance.md names: each
must either have a row or be a fixture, a file another case loads, whose
first line is `// CONFORMANCE-FIXTURE`.

    python3 impl/tools/std_coverage.py            # summary; exit 1 if anything is uncovered or unnamed
    python3 impl/tools/std_coverage.py --list     # every uncovered item and unnamed file
    python3 impl/tools/std_coverage.py --item X   # the cases naming item X

Standard library only; runs on Windows.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
STD = os.path.join(ROOT, "impl", "std")
CONF_DIR = os.path.join(ROOT, "impl", "conformance")
CONF_MD = os.path.join(ROOT, "spec", "conformance.md")


def posix(p):
    return p.replace(os.sep, "/")


def read(p):
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


def std_items():
    """[(kind, name, file, line, exempt_reason)] for every exported item."""
    items = []
    for dirpath, dirnames, filenames in os.walk(STD):
        dirnames.sort()
        for n in sorted(filenames):
            if not n.endswith(".cb"):
                continue
            p = os.path.join(dirpath, n)
            rel = posix(os.path.relpath(p, ROOT))
            lines = read(p).split("\n")
            for i, line in enumerate(lines):
                exempt = None
                j = i - 1
                while j >= 0 and lines[j].startswith("//"):
                    m = re.match(r"//\s*coverage:\s*exempt\s+(.*)", lines[j])
                    if m:
                        exempt = m.group(1).strip()
                    j -= 1
                m = re.match(r"export fn ([A-Za-z_][\w:]*)", line)
                if m:
                    items.append(("fn", m.group(1), rel, i + 1, exempt))
                    continue
                m = re.match(r"export (?:resource )?struct ([A-Za-z_]\w*)", line)
                if m:
                    items.append(("struct", m.group(1), rel, i + 1, exempt))
                    continue
                m = re.match(r"export bitstruct ([A-Za-z_]\w*)", line)
                if m:
                    items.append(("struct", m.group(1), rel, i + 1, exempt))
                    continue
                m = re.match(r"export enum ([A-Za-z_]\w*)", line)
                if m:
                    ename = m.group(1)
                    items.append(("enum", ename, rel, i + 1, exempt))
                    k = i + 1
                    while k < len(lines) and not lines[k].startswith("{"):
                        k += 1
                    k += 1
                    while k < len(lines) and not lines[k].startswith("}"):
                        vm = re.match(r"\s+([A-Z]\w*)", lines[k])
                        if vm and not lines[k].strip().startswith("//"):
                            items.append(("variant", ename + "::" + vm.group(1), rel, k + 1, exempt))
                        k += 1
    return items


def programs():
    """{label: source} for every case file and inline row fragment."""
    progs = {}
    for dirpath, dirnames, filenames in os.walk(CONF_DIR):
        dirnames.sort()
        for n in sorted(filenames):
            if n.endswith(".cb"):
                p = os.path.join(dirpath, n)
                progs[posix(os.path.relpath(p, ROOT))] = read(p)
    for i, line in enumerate(read(CONF_MD).split("\n"), 1):
        if not line.startswith("| `conf."):
            continue
        cols = line.split(" | ")
        if len(cols) < 3:
            continue
        frag = cols[1].strip()
        if "impl/conformance/" in frag or re.match(r"^`?ex\.", frag):
            continue
        rid = cols[0].strip("| `")
        progs["spec/conformance.md:%d (%s)" % (i, rid)] = frag
    return progs


def strip_comments(src):
    return re.sub(r"//[^\n]*", "", src)


def pattern(kind, name):
    if kind == "fn" and name.endswith("::drop"):
        return pattern("struct", name[: -len("::drop")])
    if kind == "fn":
        if "::" in name:
            return re.compile(r"(?<![\w:])" + re.escape(name) + r"\b")
        return re.compile(r"(?<![\w:.])" + re.escape(name) + r"\s*[(<]")
    if kind == "variant":
        enum, var = name.split("::")
        return re.compile(r"(?<![\w])(?:" + re.escape(enum) + r"::)?" + re.escape(var) + r"\b")
    return re.compile(r"(?<![\w:])" + re.escape(name) + r"\b")


def unnamed_files(conf_text):
    out = []
    for dirpath, dirnames, filenames in os.walk(CONF_DIR):
        dirnames.sort()
        for n in sorted(filenames):
            if not n.endswith(".cb"):
                continue
            p = os.path.join(dirpath, n)
            rel = posix(os.path.relpath(p, ROOT))
            if rel in conf_text:
                continue
            if read(p).startswith("// CONFORMANCE-FIXTURE"):
                continue
            out.append(rel)
    return out


def main():
    items = std_items()
    progs = {k: strip_comments(v) for k, v in programs().items()}
    if "--item" in sys.argv:
        want = sys.argv[sys.argv.index("--item") + 1]
        for kind, name, rel, line, exempt in items:
            if name == want:
                pat = pattern(kind, name)
                hits = [k for k, src in progs.items() if pat.search(src)]
                print("%s %s (%s:%d): %d cases" % (kind, name, rel, line, len(hits)))
                for h in hits:
                    print("  " + h)
        return 0
    uncovered, exempted, covered = [], [], 0
    for kind, name, rel, line, exempt in items:
        pat = pattern(kind, name)
        if any(pat.search(src) for src in progs.values()):
            covered += 1
        elif exempt:
            exempted.append((kind, name, rel, line, exempt))
        else:
            uncovered.append((kind, name, rel, line))
    unnamed = unnamed_files(read(CONF_MD))
    if "--list" in sys.argv:
        for kind, name, rel, line in uncovered:
            print("uncovered: %s %s (%s:%d)" % (kind, name, rel, line))
        for kind, name, rel, line, why in exempted:
            print("exempt: %s %s (%s:%d): %s" % (kind, name, rel, line, why))
        for rel in unnamed:
            print("unnamed: %s" % rel)
    print("std_coverage: %d exported items, %d covered, %d exempt, %d uncovered; %d case files named by no row and not a fixture"
          % (len(items), covered, len(exempted), len(uncovered), len(unnamed)))
    return 1 if uncovered or unnamed else 0


if __name__ == "__main__":
    sys.exit(main())
